// SPDX-License-Identifier: GPL-3.0-or-later
//! Copies the colour scheme and border sizes into a component's `Theme` global.
use caelum_config::Config;
use caelum_theme::{Rgba, Scheme};
use caelum_ui::{Edge, Theme};
use slint::{Color, ComponentHandle, Global};

fn color(Rgba(r, g, b, a): Rgba) -> Color {
    Color::from_argb_u8(a, r, g, b)
}

/// Caelestia's `BarWrapper.contentWidth`: inner width 40 plus padding on both sides.
pub fn bar_thickness(cfg: &Config) -> u32 {
    40 + 2 * cfg.border.thickness.max(8)
}

/// Every Slint window has its own globals, so this runs once per component.
pub fn apply<C>(ui: &C, scheme: &Scheme, cfg: &Config)
where
    C: ComponentHandle,
    for<'a> Theme<'a>: Global<'a, C>,
{
    let t = ui.global::<Theme>();
    t.set_border_thickness(cfg.border.thickness as f32);
    t.set_border_rounding(cfg.border.rounding as f32);
    t.set_bar_thickness(bar_thickness(cfg) as f32);
    t.set_bar_position(match cfg.bar.position {
        caelum_config::Edge::Top => Edge::Top,
        caelum_config::Edge::Bottom => Edge::Bottom,
        caelum_config::Edge::Left => Edge::Left,
        caelum_config::Edge::Right => Edge::Right,
    });
    t.set_background(color(scheme.background));
    t.set_on_background(color(scheme.on_background));
    t.set_surface(color(scheme.surface));
    t.set_surface_dim(color(scheme.surface_dim));
    t.set_surface_bright(color(scheme.surface_bright));
    t.set_surface_container_lowest(color(scheme.surface_container_lowest));
    t.set_surface_container_low(color(scheme.surface_container_low));
    t.set_surface_container(color(scheme.surface_container));
    t.set_surface_container_high(color(scheme.surface_container_high));
    t.set_surface_container_highest(color(scheme.surface_container_highest));
    t.set_on_surface(color(scheme.on_surface));
    t.set_surface_variant(color(scheme.surface_variant));
    t.set_on_surface_variant(color(scheme.on_surface_variant));
    t.set_inverse_surface(color(scheme.inverse_surface));
    t.set_inverse_on_surface(color(scheme.inverse_on_surface));
    t.set_outline(color(scheme.outline));
    t.set_outline_variant(color(scheme.outline_variant));
    t.set_shadow(color(scheme.shadow));
    t.set_scrim(color(scheme.scrim));
    t.set_surface_tint(color(scheme.surface_tint));
    t.set_primary(color(scheme.primary));
    t.set_on_primary(color(scheme.on_primary));
    t.set_primary_container(color(scheme.primary_container));
    t.set_on_primary_container(color(scheme.on_primary_container));
    t.set_inverse_primary(color(scheme.inverse_primary));
    t.set_secondary(color(scheme.secondary));
    t.set_on_secondary(color(scheme.on_secondary));
    t.set_secondary_container(color(scheme.secondary_container));
    t.set_on_secondary_container(color(scheme.on_secondary_container));
    t.set_tertiary(color(scheme.tertiary));
    t.set_on_tertiary(color(scheme.on_tertiary));
    t.set_tertiary_container(color(scheme.tertiary_container));
    t.set_on_tertiary_container(color(scheme.on_tertiary_container));
    t.set_error(color(scheme.error));
    t.set_on_error(color(scheme.on_error));
    t.set_error_container(color(scheme.error_container));
    t.set_on_error_container(color(scheme.on_error_container));
    t.set_primary_fixed(color(scheme.primary_fixed));
    t.set_primary_fixed_dim(color(scheme.primary_fixed_dim));
    t.set_on_primary_fixed(color(scheme.on_primary_fixed));
    t.set_on_primary_fixed_variant(color(scheme.on_primary_fixed_variant));
    t.set_secondary_fixed(color(scheme.secondary_fixed));
    t.set_secondary_fixed_dim(color(scheme.secondary_fixed_dim));
    t.set_on_secondary_fixed(color(scheme.on_secondary_fixed));
    t.set_on_secondary_fixed_variant(color(scheme.on_secondary_fixed_variant));
    t.set_tertiary_fixed(color(scheme.tertiary_fixed));
    t.set_tertiary_fixed_dim(color(scheme.tertiary_fixed_dim));
    t.set_on_tertiary_fixed(color(scheme.on_tertiary_fixed));
    t.set_on_tertiary_fixed_variant(color(scheme.on_tertiary_fixed_variant));
}
