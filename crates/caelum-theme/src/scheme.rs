// SPDX-License-Identifier: GPL-3.0-or-later
//! Material You scheme from a wallpaper, with the roles Caelestia's
//! `src/matugen/scheme.json.tmpl` uses (minus its base16 `term*` colours).
use std::path::Path;

use material_colors::{
    color::Argb,
    image::{Image, ImageReader},
    theme::ThemeBuilder,
};

use crate::ThemeError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl From<Argb> for Rgba {
    fn from(c: Argb) -> Self {
        Self(c.red, c.green, c.blue, c.alpha)
    }
}

macro_rules! scheme {
    ($($role:ident),* $(,)?) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct Scheme {
            $(pub $role: Rgba,)*
        }

        impl From<material_colors::scheme::Scheme> for Scheme {
            fn from(s: material_colors::scheme::Scheme) -> Self {
                Self { $($role: s.$role.into(),)* }
            }
        }

        impl Scheme {
            /// Every role as `(name, colour)`, in declaration order.
            pub fn roles(&self) -> [(&'static str, Rgba); [$(stringify!($role)),*].len()] {
                [$((stringify!($role), self.$role)),*]
            }
        }
    };
}

scheme!(
    background,
    on_background,
    surface,
    surface_dim,
    surface_bright,
    surface_container_lowest,
    surface_container_low,
    surface_container,
    surface_container_high,
    surface_container_highest,
    on_surface,
    surface_variant,
    on_surface_variant,
    inverse_surface,
    inverse_on_surface,
    outline,
    outline_variant,
    shadow,
    scrim,
    surface_tint,
    primary,
    on_primary,
    primary_container,
    on_primary_container,
    inverse_primary,
    secondary,
    on_secondary,
    secondary_container,
    on_secondary_container,
    tertiary,
    on_tertiary,
    tertiary_container,
    on_tertiary_container,
    error,
    on_error,
    error_container,
    on_error_container,
    primary_fixed,
    primary_fixed_dim,
    on_primary_fixed,
    on_primary_fixed_variant,
    secondary_fixed,
    secondary_fixed_dim,
    on_secondary_fixed,
    on_secondary_fixed_variant,
    tertiary_fixed,
    tertiary_fixed_dim,
    on_tertiary_fixed,
    on_tertiary_fixed_variant,
);

impl Scheme {
    /// Scheme seeded from Material's baseline purple, for when the wallpaper fails.
    pub fn fallback(dark: bool) -> Self {
        from_source(Argb::from_u32(0xff6750a4), dark)
    }
}

fn from_source(source: Argb, dark: bool) -> Scheme {
    let schemes = ThemeBuilder::with_source(source).build().schemes;
    if dark { schemes.dark } else { schemes.light }.into()
}

pub fn scheme_from_image(path: &Path, dark: bool) -> Result<Scheme, ThemeError> {
    // Shrink right after decoding so the full-size image is freed before quantising.
    let small = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .thumbnail_exact(128, 128)
        .into_rgba8();
    Ok(from_source(
        ImageReader::extract_color(&Image::new(small)),
        dark,
    ))
}

pub fn scheme_or_fallback(path: &Path, dark: bool) -> Scheme {
    scheme_from_image(path, dark).unwrap_or_else(|e| {
        tracing::warn!(
            "wallpaper scheme from {}: {e}; using fallback",
            path.display()
        );
        Scheme::fallback(dark)
    })
}
