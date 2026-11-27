// SPDX-License-Identifier: GPL-3.0-or-later
//! Wallpaper → Material You colour scheme, plus Caelestia's animation tokens.
use std::{fmt, io};

mod scheme;
pub mod tokens;

pub use scheme::{Rgba, Scheme, scheme_from_image, scheme_or_fallback};

#[derive(Debug)]
pub enum ThemeError {
    Io(io::Error),
    Image(image::ImageError),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "reading wallpaper: {e}"),
            Self::Image(e) => write!(f, "decoding wallpaper: {e}"),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<io::Error> for ThemeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<image::ImageError> for ThemeError {
    fn from(e: image::ImageError) -> Self {
        Self::Image(e)
    }
}
