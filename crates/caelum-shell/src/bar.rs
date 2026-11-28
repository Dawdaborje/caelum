// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar: its own Top layer window on `bar.position`, not drawn inside the
//! Border window. Border repaints its whole full-screen buffer on every change
//! (ADR 0001, NewBuffer), so bar updates like the clock tick would cost a
//! full-screen repaint each time. Border still draws the bar's background (its
//! frame is bar-thick on that edge), so the two join seamlessly.
use crate::{
    app::{component, spec},
    theme_bridge,
};
use caelum_config::{Config, Edge};
use caelum_theme::Scheme;
use caelum_ui::Bar;
use caelum_wayland::{Anchors, InputRegion, Layer, LayerWindow};

/// Anchors for a strip along `edge`, stretched over its full length.
pub fn edge_anchors(edge: Edge) -> Anchors {
    match edge {
        Edge::Top => Anchors::TOP | Anchors::LEFT | Anchors::RIGHT,
        Edge::Bottom => Anchors::BOTTOM | Anchors::LEFT | Anchors::RIGHT,
        Edge::Left => Anchors::LEFT | Anchors::TOP | Anchors::BOTTOM,
        Edge::Right => Anchors::RIGHT | Anchors::TOP | Anchors::BOTTOM,
    }
}

pub fn create_bar(
    output_id: u32,
    scheme: &Scheme,
    cfg: &Config,
) -> Result<(LayerWindow, Bar), Box<dyn std::error::Error>> {
    let edge = cfg.bar.position;
    let t = theme_bridge::bar_thickness(cfg);
    let size = match edge {
        Edge::Top | Edge::Bottom => (0, t),
        Edge::Left | Edge::Right => (t, 0),
    };
    let mut s = spec("caelum-bar", Layer::Top, edge_anchors(edge), size, t as i32);
    s.input_region = InputRegion::Full;
    let (window, bar) = component(output_id, s, Bar::new)?;
    theme_bridge::apply(&bar, scheme, cfg);
    Ok((window, bar))
}
