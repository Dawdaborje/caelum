// SPDX-License-Identifier: GPL-3.0-or-later
// Spike route 3: custom Slint platform on SCTK layer-shell, Slint SoftwareRenderer into wl_shm.
// One 40px bar per output. Throwaway code: unwraps are fine here.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use material_colors::{
    image::{FilterType, ImageReader},
    theme::ThemeBuilder,
};
use slint::{
    Color, ComponentHandle, PhysicalSize, Timer, TimerMode,
    platform::{
        Platform, PlatformError, Renderer, WindowAdapter, WindowEvent,
        software_renderer::{
            PremultipliedRgbaColor, RepaintBufferType, SoftwareRenderer, TargetPixel,
        },
    },
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, QueueHandle, globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    shm::{Shm, ShmHandler, slot::SlotPool},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
        },
    },
};

slint::include_modules!();

const WALLPAPER: &str = "Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg";
const HEIGHT: u32 = 40;

fn primary() -> Color {
    let path = std::path::Path::new(&std::env::var("HOME").unwrap()).join(WALLPAPER);
    let mut img = ImageReader::read(std::fs::read(path).unwrap()).unwrap();
    img.resize(128, 128, FilterType::Lanczos3);
    let p = ThemeBuilder::with_source(ImageReader::extract_color(&img))
        .build()
        .schemes
        .dark
        .primary;
    Color::from_rgb_u8(p.red, p.green, p.blue)
}

// ---- Pixel format: wl_shm Argb8888 is B,G,R,A in memory, premultiplied ----

#[derive(Clone, Copy)]
#[repr(C)]
struct Bgra {
    b: u8,
    g: u8,
    r: u8,
    a: u8,
}

impl TargetPixel for Bgra {
    fn blend(&mut self, c: PremultipliedRgbaColor) {
        let inv = (255 - c.alpha) as u16;
        let f = |d: u8, s: u8| (d as u16 * inv / 255) as u8 + s;
        *self = Bgra { b: f(self.b, c.blue), g: f(self.g, c.green), r: f(self.r, c.red), a: f(self.a, c.alpha) };
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Bgra { b, g, r, a: 255 }
    }
}

// ---- Slint window adapter + platform ---------------------------------------

struct Adapter {
    window: slint::Window,
    renderer: SoftwareRenderer,
    size: Cell<PhysicalSize>,
    dirty: Cell<bool>,
}

impl WindowAdapter for Adapter {
    fn window(&self) -> &slint::Window {
        &self.window
    }
    fn size(&self) -> PhysicalSize {
        self.size.get()
    }
    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
    fn request_redraw(&self) {
        self.dirty.set(true);
    }
}

thread_local! {
    // The next adapter Platform::create_window_adapter hands out (set just before Bar::new()).
    static NEXT: RefCell<Option<Rc<Adapter>>> = const { RefCell::new(None) };
}

struct LayerPlatform;

impl Platform for LayerPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        NEXT.with(|n| n.borrow_mut().take())
            .map(|a| a as Rc<dyn WindowAdapter>)
            .ok_or_else(|| "no pending layer surface".into())
    }
}

// ---- Wayland state ----------------------------------------------------------

struct BarSurface {
    layer: LayerSurface,
    adapter: Rc<Adapter>,
    ui: Bar,
    configured: bool,
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    shm: Shm,
    pool: SlotPool,
    bars: Vec<BarSurface>,
}

impl State {
    fn render(&mut self) {
        for b in self.bars.iter().filter(|b| b.configured && b.adapter.dirty.get()) {
            b.adapter.dirty.set(false);
            let size = b.adapter.size.get();
            let (w, h) = (size.width as i32, size.height as i32);
            let (buffer, canvas) = self
                .pool
                .create_buffer(w, h, w * 4, wl_shm::Format::Argb8888)
                .unwrap();
            // Bgra is 4 u8s with align 1, so reinterpreting the byte canvas is sound.
            let pixels = unsafe {
                std::slice::from_raw_parts_mut(canvas.as_mut_ptr().cast::<Bgra>(), canvas.len() / 4)
            };
            b.adapter.renderer.render(pixels, w as usize);
            let surface = b.layer.wl_surface();
            surface.damage_buffer(0, 0, w, h);
            buffer.attach_to(surface).unwrap();
            b.layer.commit();
        }
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.bars.retain(|b| &b.layer != layer);
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(b) = self.bars.iter_mut().find(|b| &b.layer == layer) else { return };
        let size = PhysicalSize::new(cfg.new_size.0.max(1), cfg.new_size.1.max(1));
        b.adapter.size.set(size);
        b.adapter.window.dispatch_event(WindowEvent::Resized {
            size: size.to_logical(1.0),
        });
        b.configured = true;
        b.adapter.dirty.set(true);
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
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
    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_registry!(State);
smithay_client_toolkit::delegate_dispatch2!(State);

// ---- main -------------------------------------------------------------------

fn main() {
    slint::platform::set_platform(Box::new(LayerPlatform)).unwrap();
    let bg = primary();

    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut queue) = registry_queue_init(&conn).unwrap();
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).unwrap();
    let layer_shell = LayerShell::bind(&globals, &qh).unwrap();
    let shm = Shm::bind(&globals, &qh).unwrap();
    let pool = SlotPool::new(1600 * HEIGHT as usize * 4, &shm).unwrap();

    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        shm,
        pool,
        bars: Vec::new(),
    };
    queue.roundtrip(&mut state).unwrap(); // learn the outputs

    let outputs: Vec<_> = state.outputs.outputs().collect();
    for output in outputs {
        let surface = compositor.create_surface(&qh);
        let layer = layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Top,
            Some("caelum-spike"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, HEIGHT);
        layer.set_exclusive_zone(HEIGHT as i32);
        layer.commit();

        // NewBuffer: every frame gets a fresh pool slot, so repaint it fully.
        let renderer = SoftwareRenderer::new();
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        let adapter = Rc::new_cyclic(|weak: &std::rc::Weak<Adapter>| Adapter {
            window: slint::Window::new(weak.clone()),
            renderer,
            size: Cell::new(PhysicalSize::new(1, HEIGHT)),
            dirty: Cell::new(true),
        });
        NEXT.with(|n| *n.borrow_mut() = Some(adapter.clone()));
        let ui = Bar::new().unwrap();
        ui.set_bg(bg);
        ui.show().unwrap();
        state.bars.push(BarSurface { layer, adapter, ui, configured: false });
    }

    let weak: Vec<_> = state.bars.iter().map(|b| b.ui.as_weak()).collect();
    let tick = move || {
        let t: slint::SharedString = chrono::Local::now().format("%H:%M:%S").to_string().into();
        for w in &weak {
            if let Some(ui) = w.upgrade() {
                ui.set_time(t.clone());
            }
        }
    };
    tick();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), tick);

    let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .unwrap();

    loop {
        slint::platform::update_timers_and_animations();
        state.render();
        let timeout = slint::platform::duration_until_next_timer_update();
        event_loop.dispatch(timeout, &mut state).unwrap();
    }
}
