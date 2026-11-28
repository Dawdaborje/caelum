// SPDX-License-Identifier: GPL-3.0-or-later
//! Config structs. Defaults are copied from Caelestia's `Config/*.hpp`.
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Config {
    pub wallpaper: WallpaperConfig,
    pub border: BorderConfig,
    pub bar: BarConfig,
    pub notifs: NotifsConfig,
    pub launcher: LauncherConfig,
    pub osd: OsdConfig,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct WallpaperConfig {
    pub path: PathBuf,
    pub rotation: Vec<PathBuf>,
    pub dark: bool,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Pictures/wallpaper/anime");
        let path = dir.join("sukuna-hoodie-3840x2160-26053.jpg");
        Self {
            rotation: vec![
                path.clone(),
                dir.join("sukuna-action-3840x2160-25100.jpg"),
                dir.join("sukuna and raga.jpeg"),
            ],
            path,
            dark: true,
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BorderConfig {
    pub thickness: u32,
    pub rounding: u32,
    pub smoothing: u32,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            thickness: 10,
            rounding: 25,
            smoothing: 20,
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BarConfig {
    pub position: Edge,
    pub clock: bool,
    pub tray: bool,
    pub status_icons: bool,
    pub appmenu: bool,
    pub workspaces: BarWorkspaces,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            position: Edge::default(),
            clock: true,
            tray: true,
            status_icons: true,
            appmenu: true,
            workspaces: BarWorkspaces::default(),
        }
    }
}

/// Caelestia's `bar.workspaces`, plus `enabled` (its per-widget toggle).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BarWorkspaces {
    pub enabled: bool,
    pub shown: u32,
    pub active_indicator: bool,
    pub occupied_bg: bool,
    pub show_unoccupied: bool,
    pub show_windows: bool,
}

impl Default for BarWorkspaces {
    fn default() -> Self {
        Self {
            enabled: true,
            shown: 5,
            active_indicator: true,
            occupied_bg: false,
            show_unoccupied: true,
            show_windows: true,
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct NotifsConfig {}

#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct LauncherConfig {}

#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct OsdConfig {}
