// SPDX-License-Identifier: GPL-3.0-or-later
//! Animation curves and durations copied from Caelestia's
//! `shell/plugin/src/Caelestia/Config/tokens.hpp` (`AnimCurves`, `AnimDurationTokens`).

/// A `BezierSpline`: each segment is `[c1x, c1y, c2x, c2y, endx, endy]`, starting at (0,0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve(pub &'static [[f32; 6]]);

pub const EMPHASIZED: Curve = Curve(&[
    [0.05, 0.0, 2.0 / 15.0, 0.06, 1.0 / 6.0, 0.4],
    [5.0 / 24.0, 0.82, 0.25, 1.0, 1.0, 1.0],
]);
pub const EMPHASIZED_ACCEL: Curve = Curve(&[[0.3, 0.0, 0.8, 0.15, 1.0, 1.0]]);
pub const EMPHASIZED_DECEL: Curve = Curve(&[[0.05, 0.7, 0.1, 1.0, 1.0, 1.0]]);
pub const STANDARD: Curve = Curve(&[[0.2, 0.0, 0.0, 1.0, 1.0, 1.0]]);
pub const STANDARD_ACCEL: Curve = Curve(&[[0.3, 0.0, 1.0, 1.0, 1.0, 1.0]]);
pub const STANDARD_DECEL: Curve = Curve(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
pub const EXPRESSIVE_FAST_SPATIAL: Curve = Curve(&[[0.42, 1.67, 0.21, 0.9, 1.0, 1.0]]);
pub const EXPRESSIVE_DEFAULT_SPATIAL: Curve = Curve(&[[0.38, 1.21, 0.22, 1.0, 1.0, 1.0]]);
pub const EXPRESSIVE_SLOW_SPATIAL: Curve = Curve(&[[0.39, 1.29, 0.35, 0.98, 1.0, 1.0]]);
pub const EXPRESSIVE_FAST_EFFECTS: Curve = Curve(&[[0.31, 0.94, 0.34, 1.0, 1.0, 1.0]]);
pub const EXPRESSIVE_DEFAULT_EFFECTS: Curve = Curve(&[[0.34, 0.8, 0.34, 1.0, 1.0, 1.0]]);
pub const EXPRESSIVE_SLOW_EFFECTS: Curve = Curve(&[[0.34, 0.88, 0.34, 1.0, 1.0, 1.0]]);

/// Durations in milliseconds, at Caelestia's default `scale` of 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Durations {
    pub small: u32,
    pub normal: u32,
    pub large: u32,
    pub extra_large: u32,
    pub expressive_fast_spatial: u32,
    pub expressive_default_spatial: u32,
    pub expressive_slow_spatial: u32,
    pub expressive_fast_effects: u32,
    pub expressive_default_effects: u32,
    pub expressive_slow_effects: u32,
}

pub const DURATIONS: Durations = Durations {
    small: 200,
    normal: 400,
    large: 600,
    extra_large: 1000,
    expressive_fast_spatial: 350,
    expressive_default_spatial: 500,
    expressive_slow_spatial: 650,
    expressive_fast_effects: 150,
    expressive_default_effects: 200,
    expressive_slow_effects: 300,
};
