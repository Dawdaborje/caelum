// SPDX-License-Identifier: GPL-3.0-or-later
// Spike route 2: custom Slint platform on SCTK layer-shell, FemtoVG on EGL (glutin).
// One 40px bar per output. Throwaway code: unwraps are fine here.
use std::{
    cell::{Cell, RefCell},
    ffi::CStr,
    num::NonZeroU32,
    ptr::NonNull,
    rc::Rc,
    time::Duration,
};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version},
    display::{Display, DisplayApiPreference, GlDisplay},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, WindowSurface},
};
use material_colors::{
    image::{FilterType, ImageReader},
    theme::ThemeBuilder,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use slint::{
    Color, ComponentHandle, PhysicalSize, Timer, TimerMode,
    platform::{
        Platform, PlatformError, Renderer, WindowAdapter, WindowEvent,
        femtovg_renderer::{FemtoVGRenderer, OpenGLInterface},
    },
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Proxy, QueueHandle, globals::registry_queue_init,
            protocol::{wl_output, wl_surface},
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
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

// ---- OpenGL glue: one EGL context per surface -------------------------------

struct Gl {
    display: Display,
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
}

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

unsafe impl OpenGLInterface for Gl {
    fn ensure_current(&self) -> Result<(), BoxErr> {
        Ok(self.context.make_current(&self.surface)?)
    }
    fn swap_buffers(&self) -> Result<(), BoxErr> {
        Ok(self.surface.swap_buffers(&self.context)?)
    }
    fn resize(&self, w: NonZeroU32, h: NonZeroU32) -> Result<(), BoxErr> {
        self.surface.resize(&self.context, w, h);
        Ok(())
    }
    fn get_proc_address(&self, name: &CStr) -> *const std::ffi::c_void {
        self.display.get_proc_address(name)
    }
}

fn make_gl(conn: &Connection, surface: &wl_surface::WlSurface) -> Gl {
    let raw_display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
        NonNull::new(conn.backend().display_ptr().cast()).unwrap(),
    ));
    let raw_window = RawWindowHandle::Wayland(WaylandWindowHandle::new(
        NonNull::new(surface.id().as_ptr().cast()).unwrap(),
    ));
    let display = unsafe { Display::new(raw_display, DisplayApiPreference::Egl) }.unwrap();
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .compatible_with_native_window(raw_window)
        .build();
    let config = unsafe { display.find_configs(template) }
        .unwrap()
        .reduce(|a, b| if b.num_samples() < a.num_samples() { b } else { a })
        .unwrap();
    let ctx_attrs = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version::new(2, 0))))
        .build(Some(raw_window));
    let one = NonZeroU32::new(1).unwrap();
    let surf_attrs =
        SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window, one, one);
    let surface = unsafe { display.create_window_surface(&config, &surf_attrs) }.unwrap();
    let context = unsafe { display.create_context(&config, &ctx_attrs) }
        .unwrap()
        .make_current(&surface)
        .unwrap();
    Gl { display, context, surface }
}

// ---- Slint window adapter + platform ---------------------------------------

struct Adapter {
    window: slint::Window,
    renderer: FemtoVGRenderer,
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
    bars: Vec<BarSurface>,
}

impl State {
    fn render(&mut self) {
        for b in self.bars.iter().filter(|b| b.configured && b.adapter.dirty.get()) {
            b.adapter.dirty.set(false);
            b.adapter.renderer.render().unwrap();
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

    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        bars: Vec::new(),
    };
    queue.roundtrip(&mut state).unwrap(); // learn the outputs

    let outputs: Vec<_> = state.outputs.outputs().collect();
    for output in outputs {
        let surface = compositor.create_surface(&qh);
        let layer = layer_shell.create_layer_surface(
            &qh,
            surface.clone(),
            Layer::Top,
            Some("caelum-spike"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, HEIGHT);
        layer.set_exclusive_zone(HEIGHT as i32);
        layer.commit();

        let renderer = FemtoVGRenderer::new(make_gl(&conn, &surface)).unwrap();
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
