// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer-shell windows that Slint draws into with its software renderer (ADR 0001).
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use slint::{
    PhysicalSize,
    platform::{
        Renderer, WindowAdapter, WindowEvent,
        software_renderer::{
            PremultipliedRgbaColor, RepaintBufferType, SoftwareRenderer, TargetPixel,
        },
    },
};
use smithay_client_toolkit::{
    compositor::Region,
    shell::{WaylandSurface, wlr_layer::LayerSurface},
};

use crate::{WaylandError, backend::with_state};

/// Which parts of a window take pointer input; the rest clicks through.
#[derive(Clone, Debug, PartialEq)]
pub enum InputRegion {
    Full,
    Empty,
    /// `(x, y, width, height)` in logical pixels.
    Rects(Vec<(i32, i32, i32, i32)>),
}

pub struct LayerSpec {
    pub namespace: &'static str,
    pub layer: crate::Layer,
    pub anchors: crate::Anchors,
    /// Logical size; 0 on an axis stretches between opposite anchors.
    pub size: (u32, u32),
    pub exclusive_zone: i32,
    pub keyboard: crate::KeyboardMode,
    pub input_region: InputRegion,
}

/// wl_shm `Argb8888` is B,G,R,A in memory, premultiplied.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct Bgra {
    b: u8,
    g: u8,
    r: u8,
    a: u8,
}

impl TargetPixel for Bgra {
    fn blend(&mut self, c: PremultipliedRgbaColor) {
        let inv = (255 - c.alpha) as u16;
        let f = |d: u8, s: u8| (d as u16 * inv / 255) as u8 + s;
        *self = Bgra {
            b: f(self.b, c.blue),
            g: f(self.g, c.green),
            r: f(self.r, c.red),
            a: f(self.a, c.alpha),
        };
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Bgra { b, g, r, a: 255 }
    }
}

pub(crate) struct Adapter {
    pub window: slint::Window,
    pub renderer: SoftwareRenderer,
    /// Physical (buffer) size.
    pub size: Cell<PhysicalSize>,
    pub dirty: Cell<bool>,
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
    /// The adapter the next Slint component created on this thread attaches to.
    pub(crate) static NEXT: RefCell<Option<Rc<Adapter>>> = const { RefCell::new(None) };
}

pub(crate) struct Surface {
    pub layer: LayerSurface,
    pub adapter: Rc<Adapter>,
    pub output_id: u32,
    pub configured: Cell<bool>,
    pub frame_pending: Cell<bool>,
    pub scale: Cell<i32>,
    /// Logical size from the last configure.
    pub logical: Cell<(u32, u32)>,
}

impl Surface {
    /// Applies an integer output scale; returns the event Slint needs, if it changed.
    pub fn set_scale(&self, scale: i32) -> Option<WindowEvent> {
        let scale = scale.max(1);
        if self.scale.replace(scale) == scale {
            return None;
        }
        self.layer.wl_surface().set_buffer_scale(scale);
        self.resize_buffer();
        Some(WindowEvent::ScaleFactorChanged {
            scale_factor: scale as f32,
        })
    }

    pub fn resize_buffer(&self) {
        let (w, h) = self.logical.get();
        let s = self.scale.get() as u32;
        self.adapter
            .size
            .set(PhysicalSize::new(w.max(1) * s, h.max(1) * s));
        self.adapter.dirty.set(true);
    }
}

/// A layer-shell window. Dropping it destroys the surface.
pub struct LayerWindow {
    pub(crate) inner: Rc<Surface>,
}

impl LayerWindow {
    pub fn slint_window(&self) -> &slint::Window {
        &self.inner.adapter.window
    }

    pub fn output_id(&self) -> u32 {
        self.inner.output_id
    }

    pub fn set_exclusive_zone(&self, zone: i32) {
        self.inner.layer.set_exclusive_zone(zone);
        self.inner.layer.commit();
    }

    pub fn set_size(&self, width: u32, height: u32) {
        self.inner.layer.set_size(width, height);
        self.inner.layer.commit();
    }

    pub fn set_input_region(&self, region: InputRegion) {
        with_state(|s| apply_input_region(&s.compositor, &self.inner.layer, &region));
        self.inner.layer.commit();
    }
}

fn apply_input_region(
    compositor: &smithay_client_toolkit::compositor::CompositorState,
    layer: &LayerSurface,
    region: &InputRegion,
) {
    let rects = match region {
        // A null region means the whole surface.
        InputRegion::Full => return layer.wl_surface().set_input_region(None),
        InputRegion::Empty => &[][..],
        InputRegion::Rects(r) => r,
    };
    match Region::new(compositor) {
        Ok(wl_region) => {
            for &(x, y, w, h) in rects {
                wl_region.add(x, y, w, h);
            }
            layer
                .wl_surface()
                .set_input_region(Some(wl_region.wl_region()));
        }
        Err(e) => tracing::warn!("input region: {e}"),
    }
}

/// Creates a layer window on an output. The **next Slint component created on
/// this thread** (e.g. `Background::new()`) becomes this window's content.
pub fn create_layer_window(output_id: u32, spec: LayerSpec) -> Result<LayerWindow, WaylandError> {
    let (surface, scale_event) = with_state(|s| {
        let (output, scale) = s
            .output(output_id)
            .ok_or_else(|| WaylandError(format!("no output {output_id}")))?;
        let wl_surface = s.compositor.create_surface(&s.qh);
        let layer = s.layer_shell.create_layer_surface(
            &s.qh,
            wl_surface,
            spec.layer,
            Some(spec.namespace),
            Some(&output),
        );
        layer.set_anchor(spec.anchors);
        layer.set_size(spec.size.0, spec.size.1);
        layer.set_exclusive_zone(spec.exclusive_zone);
        layer.set_keyboard_interactivity(spec.keyboard);
        apply_input_region(&s.compositor, &layer, &spec.input_region);
        layer.commit();

        // shortcut: NewBuffer repaints the whole surface each frame; switch to
        // SwappedBuffers with two kept buffers once drawers animate (ADR 0001).
        let renderer = SoftwareRenderer::new();
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        let adapter = Rc::new_cyclic(|weak: &std::rc::Weak<Adapter>| Adapter {
            window: slint::Window::new(weak.clone()),
            renderer,
            size: Cell::new(PhysicalSize::new(1, 1)),
            dirty: Cell::new(true),
        });
        let surface = Rc::new(Surface {
            layer,
            adapter,
            output_id,
            configured: Cell::new(false),
            frame_pending: Cell::new(false),
            scale: Cell::new(1),
            logical: Cell::new(spec.size),
        });
        let scale_event = surface.set_scale(scale);
        s.surfaces.push(Rc::downgrade(&surface));
        Ok::<_, WaylandError>((surface, scale_event))
    })?;
    if let Some(ev) = scale_event {
        surface.adapter.window.dispatch_event(ev);
    }
    NEXT.with(|n| *n.borrow_mut() = Some(surface.adapter.clone()));
    Ok(LayerWindow { inner: surface })
}
