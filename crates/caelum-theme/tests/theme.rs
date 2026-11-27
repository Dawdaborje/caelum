// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::{Path, PathBuf};

use caelum_theme::{Rgba, Scheme, scheme_from_image, scheme_or_fallback, tokens};

/// 16×16 solid #c0392b, written to a temp dir so no image is committed.
fn red_png(dir: &Path) -> PathBuf {
    let path = dir.join("red.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([0xc0, 0x39, 0x2b, 0xff]))
        .save(&path)
        .unwrap();
    path
}

fn hue(Rgba(r, g, b, _): Rgba) -> f32 {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        (g - b) / d
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0).rem_euclid(360.0)
}

fn luma(Rgba(r, g, b, _): Rgba) -> u32 {
    r as u32 + g as u32 + b as u32
}

#[test]
fn red_image_gives_reddish_primary() {
    let dir = tempfile::tempdir().unwrap();
    let h = hue(scheme_from_image(&red_png(dir.path()), true)
        .unwrap()
        .primary);
    let dist = (h - 6.0).abs().min(360.0 - (h - 6.0).abs());
    assert!(dist <= 30.0, "hue {h}");
}

#[test]
fn missing_wallpaper_uses_fallback() {
    assert_eq!(
        scheme_or_fallback(Path::new("/nope.jpg"), true),
        Scheme::fallback(true)
    );
}

#[test]
fn dark_and_light_differ() {
    let dir = tempfile::tempdir().unwrap();
    let red = red_png(dir.path());
    let dark = scheme_from_image(&red, true).unwrap();
    let light = scheme_from_image(&red, false).unwrap();
    assert!(luma(dark.surface) < luma(light.surface));
}

#[test]
fn curves_match_caelestia() {
    // ~/caelestia/shell/plugin/src/Caelestia/Config/tokens.hpp:19
    assert_eq!(
        tokens::EMPHASIZED_DECEL.0,
        &[[0.05, 0.7, 0.1, 1.0, 1.0, 1.0]]
    );
    // tokens.hpp:16-17, two segments
    assert_eq!(tokens::EMPHASIZED.0.len(), 2);
}
