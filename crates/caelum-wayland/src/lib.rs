// SPDX-License-Identifier: GPL-3.0-or-later
//! Slint on wlr-layer-shell (ADR 0001 route 3) plus output tracking.
//!
//! Usage: `init(on_output)`, then `slint::run_event_loop()`. In `on_output`, feed
//! an [`OutputTracker`] and call [`create_layer_window`] then `MyComponent::new()`
//! for each `SurfaceAction::Create`.
mod backend;
mod outputs;
pub mod protocols;
mod surface;

pub use backend::init;
pub use outputs::{OutputEvent, OutputInfo, OutputTracker, SurfaceAction};
pub use smithay_client_toolkit::shell::wlr_layer::{
    Anchor as Anchors, KeyboardInteractivity as KeyboardMode, Layer,
};
pub use surface::{InputRegion, LayerSpec, LayerWindow, create_layer_window};

#[derive(Debug)]
pub struct WaylandError(pub String);

impl std::fmt::Display for WaylandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for WaylandError {}
