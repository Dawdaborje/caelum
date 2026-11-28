// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar: its own Top layer window on `bar.position`, not drawn inside the
//! Border window. Border repaints its whole full-screen buffer on every change
//! (ADR 0001, NewBuffer), so bar updates like the clock tick would cost a
//! full-screen repaint each time. Border still draws the bar's background (its
//! frame is bar-thick on that edge), so the two join seamlessly.
use crate::{
    app::{component, spec},
    clock, theme_bridge,
};
use caelum_config::{BarWorkspaces, Config, Edge};
use caelum_services::kwin::{KwinState, Workspace, activate_workspace};
use caelum_theme::Scheme;
use caelum_ui::{Bar, ClockOptions, WorkspaceSlot};
use caelum_wayland::{Anchors, InputRegion, Layer, LayerWindow};
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

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
    bar.on_activate_workspace(|id| activate_workspace(&id));
    bar.on_tray_activate(move |s, p| crate::tray::activate(output_id, s.into(), p));
    bar.on_tray_secondary(move |s, p| crate::tray::secondary(output_id, s.into(), p));
    bar.on_tray_menu(move |s, p| crate::tray::open_menu(output_id, s.into(), p));
    Ok((window, bar))
}

/// Pushes config switches, the clock and KWin state into one output's bar.
pub fn update(bar: &Bar, cfg: &Config, kwin: Option<&KwinState>, output: &str) {
    let c = &cfg.bar.clock;
    bar.set_show_clock(c.enabled);
    bar.set_clock_options(ClockOptions {
        background: c.background,
        show_date: c.show_date,
        show_icon: c.show_icon,
        show_seconds: c.show_seconds,
    });
    bar.set_clock_time(clock::now(c.twelve_hour));
    let ws = &cfg.bar.workspaces;
    bar.set_show_workspaces(ws.enabled && kwin.is_some());
    bar.set_active_indicator(ws.active_indicator);
    bar.set_occupied_bg(ws.occupied_bg);
    bar.set_show_window(cfg.bar.appmenu);
    let Some(kwin) = kwin else { return };
    let slots = visible_workspaces(kwin, output, ws);
    let active = slots.iter().position(|s| s.active).map_or(-1, |i| i as i32);
    bar.set_workspaces(ModelRc::new(VecModel::from(slots)));
    bar.set_active_workspace(active);
    let title = kwin.active_window.as_ref().map(|w| w.title.as_str());
    bar.set_window_title(title.unwrap_or_default().into());
}

/// The distro logo, as Caelestia's SysInfo: `LOGO` from /etc/os-release, looked
/// up as an icon. Empty (the icon hides) when there is none.
/// shortcut: PNGs in hicolor and pixmaps only, no icon-theme lookup or SVG (this
/// Slint build has no SVG support), and no Caelestia-logo fallback.
pub fn os_logo() -> Image {
    let Ok(release) = std::fs::read_to_string("/etc/os-release") else {
        return Image::default();
    };
    let Some(name) = logo_name(&release) else {
        return Image::default();
    };
    let sizes = ["512x512", "256x256", "128x128", "96x96", "64x64", "48x48"];
    let candidates = sizes
        .iter()
        .map(|s| format!("/usr/share/icons/hicolor/{s}/apps/{name}.png"))
        .chain([format!("/usr/share/pixmaps/{name}.png")]);
    for path in candidates {
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.into_rgba8();
                let (w, h) = rgba.dimensions();
                let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), w, h);
                return Image::from_rgba8(buf);
            }
            Err(image::ImageError::IoError(_)) => {}
            Err(e) => tracing::warn!("os logo {path}: {e}"),
        }
    }
    Image::default()
}

/// The `LOGO=` value of an os-release file, unquoted.
fn logo_name(release: &str) -> Option<&str> {
    let value = release.lines().find_map(|l| l.strip_prefix("LOGO="))?;
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    (!value.is_empty()).then_some(value)
}

/// The workspace slots one output's bar shows, as Caelestia's `Workspaces.wsIds`:
/// with `show_unoccupied`, the page of `shown` slots holding the active desktop;
/// otherwise up to `shown` occupied desktops ending at (or after) the active one.
fn visible_workspaces(kwin: &KwinState, output: &str, cfg: &BarWorkspaces) -> Vec<WorkspaceSlot> {
    let active = kwin.active_by_output.get(output).or(kwin.active.as_ref());
    let shown = cfg.shown.max(1) as usize;
    let is_active = |w: &&Workspace| Some(&w.id) == active;
    let list: Vec<&Workspace> = if cfg.show_unoccupied {
        let index = kwin
            .workspaces
            .iter()
            .position(|w| is_active(&w))
            .unwrap_or(0);
        let start = index / shown * shown;
        kwin.workspaces.iter().skip(start).take(shown).collect()
    } else {
        let list: Vec<_> = kwin
            .workspaces
            .iter()
            .filter(|w| w.occupied || is_active(w))
            .collect();
        let Some(index) = list.iter().position(is_active) else {
            return Vec::new();
        };
        let end = (index + 1).max(shown.min(list.len()));
        list[end.saturating_sub(shown)..end].to_vec()
    };
    list.into_iter()
        .map(|w| WorkspaceSlot {
            id: w.id.as_str().into(),
            occupied: w.occupied,
            active: is_active(&w),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kwin(occupied: &[bool], active: usize) -> KwinState {
        KwinState {
            workspaces: occupied
                .iter()
                .enumerate()
                .map(|(i, &occupied)| Workspace {
                    id: i.to_string(),
                    name: String::new(),
                    position: i as u32,
                    occupied,
                })
                .collect(),
            active: Some(active.to_string()),
            ..Default::default()
        }
    }

    fn ids(slots: &[WorkspaceSlot]) -> Vec<String> {
        slots.iter().map(|s| s.id.to_string()).collect()
    }

    #[test]
    fn logo_name_unquotes() {
        assert_eq!(
            logo_name("ID=arch\nLOGO=archlinux-logo\n"),
            Some("archlinux-logo")
        );
        assert_eq!(
            logo_name("LOGO=\"fedora-logo-icon\""),
            Some("fedora-logo-icon")
        );
        assert_eq!(logo_name("ID=x\nLOGO=\n"), None);
    }

    #[test]
    fn pages_of_shown() {
        let mut cfg = BarWorkspaces {
            shown: 2,
            ..Default::default()
        };
        let k = kwin(&[false; 5], 3);
        assert_eq!(ids(&visible_workspaces(&k, "X", &cfg)), ["2", "3"]);
        cfg.show_unoccupied = false;
        let k = kwin(&[true, false, true, true, false], 4);
        // Occupied 0, 2, 3 plus active 4: the last two, ending at the active one.
        assert_eq!(ids(&visible_workspaces(&k, "X", &cfg)), ["3", "4"]);
    }

    #[test]
    fn output_overrides_global_active() {
        let mut k = kwin(&[false; 3], 0);
        k.active_by_output.insert("DP-1".into(), "2".into());
        let slots = visible_workspaces(&k, "DP-1", &BarWorkspaces::default());
        assert!(slots[2].active && !slots[0].active);
    }
}
