// SPDX-License-Identifier: GPL-3.0-or-later
//! Manual check: one 1×1 transparent Top surface per output; logs output events.
//! `just nested2 target/debug/examples/outputs`
use std::{cell::RefCell, collections::HashMap};

use caelum_wayland::{
    Anchors, InputRegion, KeyboardMode, Layer, LayerSpec, LayerWindow, OutputTracker,
    SurfaceAction, create_layer_window,
};
use slint::ComponentHandle;

slint::slint! {
    #[edition = "1.19"]
    export component Dot inherits Window {
        background: transparent;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tracker = RefCell::new(OutputTracker::default());
    let windows: RefCell<HashMap<u32, (LayerWindow, Dot)>> = RefCell::default();
    caelum_wayland::init(move |ev| {
        eprintln!("{ev:?}");
        for action in tracker.borrow_mut().apply(ev) {
            eprintln!("  -> {action:?}");
            match action {
                SurfaceAction::Create(id) => {
                    let spec = LayerSpec {
                        namespace: "caelum-outputs-example",
                        layer: Layer::Top,
                        anchors: Anchors::TOP | Anchors::LEFT,
                        size: (1, 1),
                        exclusive_zone: 0,
                        keyboard: KeyboardMode::None,
                        input_region: InputRegion::Empty,
                    };
                    let window = create_layer_window(id, spec).expect("layer window");
                    let dot = Dot::new().expect("component");
                    dot.show().expect("show");
                    windows.borrow_mut().insert(id, (window, dot));
                }
                SurfaceAction::Destroy(id) => {
                    windows.borrow_mut().remove(&id);
                }
                SurfaceAction::Resize(_) => {}
            }
        }
    })?;
    slint::run_event_loop()?;
    Ok(())
}
