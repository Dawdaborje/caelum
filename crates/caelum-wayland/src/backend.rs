// SPDX-License-Identifier: GPL-3.0-or-later
//! Wayland state, SCTK handlers and the Slint platform that drives the loop.
//!
//! Slint callbacks never run while `STATE` is borrowed: window events, output
//! events and `invoke_from_event_loop` calls are queued during the Wayland
//! dispatch and run after it, so user code may freely create or change windows.
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

use slint::{
    LogicalPosition, LogicalSize, SharedString,
    platform::{
        EventLoopProxy, Key, Platform, PlatformError, PointerEventButton, WindowAdapter,
        WindowEvent,
    },
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            EventLoop, LoopHandle,
            channel::{self, Sender},
        },
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
        },
        protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
            Shape, WpCursorShapeDeviceV1,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{
            PointerEvent, PointerEventKind, PointerHandler, cursor_shape::CursorShapeManager,
        },
    },
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};

use crate::{
    OutputEvent, OutputInfo, WaylandError,
    surface::{Adapter, Bgra, NEXT, Surface},
};

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs `f` on the Wayland state. Panics if called before `init` or re-entrantly,
/// which the queue-then-run design rules out.
pub(crate) fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        f(s.borrow_mut()
            .as_mut()
            .expect("caelum_wayland::init not called"))
    })
}

type Deferred = Box<dyn FnOnce()>;

pub(crate) struct State {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    pub compositor: CompositorState,
    pub layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    cursor_shape: Option<CursorShapeManager>,
    pub qh: QueueHandle<State>,
    loop_handle: LoopHandle<'static, State>,
    pub surfaces: Vec<Weak<Surface>>,
    output_ids: Vec<(wl_output::WlOutput, u32)>,
    output_events: Vec<OutputEvent>,
    deferred: Vec<Deferred>,
    quit: bool,
    // shortcut: one pointer and one keyboard (the first seat that has them).
    pointer: Option<(wl_pointer::WlPointer, Option<WpCursorShapeDeviceV1>)>,
    pointer_focus: Weak<Surface>,
    pointer_pos: LogicalPosition,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: Weak<Surface>,
}

impl State {
    pub fn output(&self, id: u32) -> Option<(wl_output::WlOutput, i32)> {
        let (output, _) = self.output_ids.iter().find(|(_, i)| *i == id)?;
        let scale = self.outputs.info(output).map_or(1, |i| i.scale_factor);
        Some((output.clone(), scale))
    }

    fn surface(&mut self, wl: &wl_surface::WlSurface) -> Option<Rc<Surface>> {
        self.surfaces.retain(|w| w.strong_count() > 0);
        self.surfaces
            .iter()
            .filter_map(Weak::upgrade)
            .find(|s| s.layer.wl_surface() == wl)
    }

    fn send(&mut self, adapter: &Rc<Adapter>, ev: WindowEvent) {
        let adapter = adapter.clone();
        self.deferred
            .push(Box::new(move || adapter.window.dispatch_event(ev)));
    }

    fn output_info(&self, output: &wl_output::WlOutput) -> Option<OutputInfo> {
        let i = self.outputs.info(output)?;
        let scale = i.scale_factor.max(1);
        let (width, height) = i.logical_size.unwrap_or_else(|| {
            i.modes
                .iter()
                .find(|m| m.current)
                .map_or((0, 0), |m| (m.dimensions.0 / scale, m.dimensions.1 / scale))
        });
        Some(OutputInfo {
            id: i.id,
            name: i.name.unwrap_or_default(),
            width,
            height,
            scale: scale as f64,
        })
    }

    fn render(&mut self) {
        self.surfaces.retain(|w| w.strong_count() > 0);
        let ready: Vec<_> = self
            .surfaces
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|s| s.configured.get() && !s.frame_pending.get() && s.adapter.dirty.get())
            .collect();
        for s in ready {
            s.adapter.dirty.set(false);
            let size = s.adapter.size.get();
            let (w, h) = (size.width as i32, size.height as i32);
            let (buffer, canvas) =
                match self
                    .pool
                    .create_buffer(w, h, w * 4, wl_shm::Format::Argb8888)
                {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("shm buffer {w}x{h}: {e}");
                        continue;
                    }
                };
            // SAFETY: Bgra is four u8s (align 1), so any byte slice of len 4n is n valid Bgra.
            let pixels = unsafe {
                std::slice::from_raw_parts_mut(canvas.as_mut_ptr().cast::<Bgra>(), canvas.len() / 4)
            };
            s.adapter.renderer.render(pixels, w as usize);
            let wl = s.layer.wl_surface();
            wl.damage_buffer(0, 0, w, h);
            wl.frame(&self.qh, FrameCallbackData(wl.clone()));
            if let Err(e) = buffer.attach_to(wl) {
                tracing::warn!("attach buffer: {e}");
                continue;
            }
            s.frame_pending.set(true);
            s.layer.commit();
        }
    }

    fn key(&mut self, event: KeyEvent, kind: fn(SharedString) -> WindowEvent) {
        let Some(s) = self.keyboard_focus.upgrade() else {
            return;
        };
        if let Some(text) = key_text(event.keysym, event.utf8) {
            self.send(&s.adapter, kind(text));
        }
    }
}

fn key_text(sym: Keysym, utf8: Option<String>) -> Option<SharedString> {
    let key = match sym {
        Keysym::Return | Keysym::KP_Enter => Key::Return,
        Keysym::Escape => Key::Escape,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Tab => Key::Tab,
        Keysym::ISO_Left_Tab => Key::Backtab,
        Keysym::Delete => Key::Delete,
        Keysym::Left => Key::LeftArrow,
        Keysym::Right => Key::RightArrow,
        Keysym::Up => Key::UpArrow,
        Keysym::Down => Key::DownArrow,
        Keysym::Home => Key::Home,
        Keysym::End => Key::End,
        Keysym::Page_Up => Key::PageUp,
        Keysym::Page_Down => Key::PageDown,
        Keysym::Shift_L => Key::Shift,
        Keysym::Shift_R => Key::ShiftR,
        Keysym::Control_L => Key::Control,
        Keysym::Control_R => Key::ControlR,
        Keysym::Alt_L => Key::Alt,
        Keysym::Alt_R | Keysym::ISO_Level3_Shift => Key::AltGr,
        Keysym::Super_L => Key::Meta,
        Keysym::Super_R => Key::MetaR,
        _ => return utf8.filter(|t| !t.is_empty()).map(Into::into),
    };
    Some(key.into())
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        // The compositor dropped it (usually its output went away); stop drawing.
        if let Some(s) = self.surface(layer.wl_surface()) {
            s.configured.set(false);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(s) = self.surface(layer.wl_surface()) else {
            return;
        };
        let (w, h) = (cfg.new_size.0.max(1), cfg.new_size.1.max(1));
        s.logical.set((w, h));
        s.resize_buffer();
        s.configured.set(true);
        self.send(
            &s.adapter,
            WindowEvent::Resized {
                size: LogicalSize::new(w as f32, h as f32),
            },
        );
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: &wl_surface::WlSurface, _: u32) {
        if let Some(s) = self.surface(wl) {
            s.frame_pending.set(false);
        }
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(info) = self.output_info(&output) {
            self.output_ids.push((output, info.id));
            self.output_events.push(OutputEvent::Added(info));
        }
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let Some(info) = self.output_info(&output) else {
            return;
        };
        let scale = info.scale as i32;
        for s in self.surfaces.clone().iter().filter_map(Weak::upgrade) {
            if s.output_id == info.id
                && let Some(ev) = s.set_scale(scale)
            {
                self.send(&s.adapter, ev);
            }
        }
        self.output_events.push(OutputEvent::Changed(info));
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(i) = self.output_ids.iter().position(|(o, _)| *o == output) {
            let (_, id) = self.output_ids.swap_remove(i);
            self.output_events.push(OutputEvent::Removed(id));
        }
    }
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Pointer if self.pointer.is_none() => {
                match self.seats.get_pointer(qh, &seat) {
                    Ok(p) => {
                        let shape = self
                            .cursor_shape
                            .as_ref()
                            .map(|m| m.get_shape_device(&p, qh));
                        self.pointer = Some((p, shape));
                    }
                    Err(e) => tracing::warn!("pointer: {e}"),
                }
            }
            Capability::Keyboard if self.keyboard.is_none() => {
                let repeat =
                    Box::new(|s: &mut State, _: &wl_keyboard::WlKeyboard, ev: KeyEvent| {
                        s.key(ev, |text| WindowEvent::KeyPressRepeated { text });
                    });
                match self.seats.get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    repeat,
                ) {
                    Ok(k) => self.keyboard = Some(k),
                    Err(e) => tracing::warn!("keyboard: {e}"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Pointer => {
                if let Some((p, shape)) = self.pointer.take() {
                    if let Some(shape) = shape {
                        shape.destroy();
                    }
                    p.release();
                }
            }
            Capability::Keyboard => {
                if let Some(k) = self.keyboard.take() {
                    k.release();
                }
            }
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            let Some(s) = self.surface(&e.surface) else {
                continue;
            };
            let position = LogicalPosition::new(e.position.0 as f32, e.position.1 as f32);
            let ev = match e.kind {
                PointerEventKind::Enter { serial } => {
                    self.pointer_focus = Rc::downgrade(&s);
                    if let Some((_, Some(shape))) = &self.pointer {
                        shape.set_shape(serial, Shape::Default);
                    }
                    WindowEvent::PointerMoved { position }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_focus = Weak::new();
                    WindowEvent::PointerExited
                }
                PointerEventKind::Motion { .. } => WindowEvent::PointerMoved { position },
                PointerEventKind::Press { button, .. } => WindowEvent::PointerPressed {
                    position,
                    button: pointer_button(button),
                },
                PointerEventKind::Release { button, .. } => WindowEvent::PointerReleased {
                    position,
                    button: pointer_button(button),
                },
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => WindowEvent::PointerScrolled {
                    position: self.pointer_pos,
                    // Wayland axis grows downwards/rightwards; Slint deltas are the opposite.
                    delta_x: -horizontal.absolute as f32,
                    delta_y: -vertical.absolute as f32,
                },
            };
            if !matches!(e.kind, PointerEventKind::Axis { .. }) {
                self.pointer_pos = position;
            }
            self.send(&s.adapter, ev);
        }
    }
}

fn pointer_button(code: u32) -> PointerEventButton {
    // Linux input codes BTN_LEFT, BTN_RIGHT, BTN_MIDDLE.
    match code {
        0x110 => PointerEventButton::Left,
        0x111 => PointerEventButton::Right,
        0x112 => PointerEventButton::Middle,
        _ => PointerEventButton::Other,
    }
}

impl KeyboardHandler for State {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        wl: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        if let Some(s) = self.surface(wl) {
            self.keyboard_focus = Rc::downgrade(&s);
            self.send(&s.adapter, WindowEvent::WindowActiveChanged(true));
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        if let Some(s) = std::mem::take(&mut self.keyboard_focus).upgrade() {
            self.send(&s.adapter, WindowEvent::WindowActiveChanged(false));
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        ev: KeyEvent,
    ) {
        self.key(ev, |text| WindowEvent::KeyPressed { text });
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        ev: KeyEvent,
    ) {
        self.key(ev, |text| WindowEvent::KeyPressRepeated { text });
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        ev: KeyEvent,
    ) {
        self.key(ev, |text| WindowEvent::KeyReleased { text });
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        // Slint tracks modifiers from the Shift/Control/Alt/Meta key events themselves.
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(State);
smithay_client_toolkit::delegate_dispatch2!(State);

// ---- Slint platform ---------------------------------------------------------

enum Msg {
    Invoke(Box<dyn FnOnce() + Send>),
    Quit,
}

struct Proxy(Sender<Msg>);

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.0
            .send(Msg::Quit)
            .map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
    fn invoke_from_event_loop(
        &self,
        f: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.0
            .send(Msg::Invoke(f))
            .map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
}

struct LayerPlatform {
    event_loop: RefCell<EventLoop<'static, State>>,
    on_output: RefCell<Box<dyn FnMut(OutputEvent)>>,
    proxy: Sender<Msg>,
}

impl Platform for LayerPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        NEXT.with(|n| n.borrow_mut().take())
            .map(|a| a as Rc<dyn WindowAdapter>)
            .ok_or_else(|| "create_layer_window must be called before creating a component".into())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.proxy.clone())))
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let mut event_loop = self.event_loop.borrow_mut();
        loop {
            let (outputs, deferred, quit) = with_state(|s| {
                (
                    std::mem::take(&mut s.output_events),
                    std::mem::take(&mut s.deferred),
                    std::mem::take(&mut s.quit),
                )
            });
            if quit {
                return Ok(());
            }
            for ev in outputs {
                (self.on_output.borrow_mut())(ev);
            }
            for f in deferred {
                f();
            }
            slint::platform::update_timers_and_animations();
            with_state(State::render);
            let timeout = if with_state(|s| s.deferred.is_empty() && s.output_events.is_empty()) {
                slint::platform::duration_until_next_timer_update()
            } else {
                Some(Duration::ZERO)
            };
            STATE
                .with(|s| {
                    let mut s = s.borrow_mut();
                    let s = s.as_mut().expect("caelum_wayland::init not called");
                    event_loop.dispatch(timeout, s)
                })
                .map_err(|e| PlatformError::Other(format!("wayland dispatch: {e}")))?;
        }
    }
}

/// Connects to Wayland and makes this crate Slint's platform. Then run
/// `slint::run_event_loop()`; `on_output` is called (on the loop) for every
/// output, starting with the ones that already exist.
pub fn init(on_output: impl FnMut(OutputEvent) + 'static) -> Result<(), WaylandError> {
    let err =
        |what: &'static str| move |e: &dyn std::fmt::Display| WaylandError(format!("{what}: {e}"));
    let conn = Connection::connect_to_env().map_err(|e| err("connect")(&e))?;
    let (globals, mut queue) = registry_queue_init(&conn).map_err(|e| err("registry")(&e))?;
    let qh = queue.handle();
    let event_loop: EventLoop<'static, State> =
        EventLoop::try_new().map_err(|e| err("event loop")(&e))?;
    let shm = Shm::bind(&globals, &qh).map_err(|e| err("wl_shm")(&e))?;
    let pool = SlotPool::new(4096, &shm).map_err(|e| err("shm pool")(&e))?;

    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh).map_err(|e| err("wl_compositor")(&e))?,
        layer_shell: LayerShell::bind(&globals, &qh).map_err(|e| err("layer shell")(&e))?,
        shm,
        pool,
        cursor_shape: CursorShapeManager::bind(&globals, &qh).ok(),
        qh,
        loop_handle: event_loop.handle(),
        surfaces: Vec::new(),
        output_ids: Vec::new(),
        output_events: Vec::new(),
        deferred: Vec::new(),
        quit: false,
        pointer: None,
        pointer_focus: Weak::new(),
        pointer_pos: LogicalPosition::default(),
        keyboard: None,
        keyboard_focus: Weak::new(),
    };
    queue
        .roundtrip(&mut state)
        .map_err(|e| err("roundtrip")(&e))?;

    WaylandSource::new(conn, queue)
        .insert(event_loop.handle())
        .map_err(|e| err("wayland source")(&e.error))?;
    let (proxy, channel) = channel::channel();
    event_loop
        .handle()
        .insert_source(channel, |ev, _, s: &mut State| match ev {
            channel::Event::Msg(Msg::Invoke(f)) => s.deferred.push(f),
            channel::Event::Msg(Msg::Quit) => s.quit = true,
            channel::Event::Closed => {}
        })
        .map_err(|e| err("channel")(&e.error))?;

    STATE.with(|s| *s.borrow_mut() = Some(state));
    slint::platform::set_platform(Box::new(LayerPlatform {
        event_loop: RefCell::new(event_loop),
        on_output: RefCell::new(Box::new(on_output)),
        proxy,
    }))
    .map_err(|e| err("slint platform")(&e))
}
