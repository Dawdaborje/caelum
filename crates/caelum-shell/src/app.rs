// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-output surfaces: wallpaper, border frame and edge exclusion zones.
use std::{cell::RefCell, collections::HashMap, path::Path};

use caelum_config::Config;
use caelum_config::Edge;
use caelum_services::{kwin::KwinState, tray::TrayState};
use caelum_theme::{Scheme, scheme_or_fallback};
use caelum_ui::{Background, Bar, Border, Exclusion};
use caelum_wayland::{
    Anchors, InputRegion, KeyboardMode, Layer, LayerSpec, LayerWindow, OutputEvent, OutputInfo,
    OutputTracker, SurfaceAction, WaylandError, create_layer_window,
};
use slint::{ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel};

use crate::{bar, theme_bridge};

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

struct OutputSurfaces {
    background: (LayerWindow, Background),
    border: (LayerWindow, Border),
    edges: EdgeSurfaces,
}

/// Surfaces that depend on the bar position and thickness, rebuilt when they change.
struct EdgeSurfaces {
    bar: (LayerWindow, Bar),
    /// Held only to keep the strips' exclusive zones alive.
    _exclusions: Vec<(LayerWindow, Exclusion)>,
}

pub struct App {
    cfg: Config,
    scheme: Scheme,
    tracker: OutputTracker,
    outputs: HashMap<u32, OutputSurfaces>,
    /// Decoded wallpapers by pixel size, so equal outputs share one image.
    wallpapers: HashMap<(u32, u32), Image>,
    /// Latest workspaces and active window; None until (or unless) KWin reports.
    kwin: Option<KwinState>,
    /// Latest tray items; None while the tray service is unavailable.
    tray: Option<TrayState>,
    status: crate::status::Status,
    os_logo: Image,
}

pub fn init(cfg: Config) {
    let scheme = scheme_or_fallback(&cfg.wallpaper.path, cfg.wallpaper.dark);
    let app = App {
        cfg,
        scheme,
        tracker: OutputTracker::default(),
        outputs: HashMap::new(),
        wallpapers: HashMap::new(),
        kwin: None,
        tray: None,
        status: Default::default(),
        os_logo: bar::os_logo(),
    };
    APP.with(|a| *a.borrow_mut() = Some(app));
}

fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|a| {
        if let Some(app) = a.borrow_mut().as_mut() {
            f(app)
        }
    });
}

pub fn on_output(ev: OutputEvent) {
    with_app(|app| {
        for action in app.tracker.apply(ev) {
            let result = match action {
                SurfaceAction::Create(id) => app.create(id),
                SurfaceAction::Destroy(id) => {
                    app.outputs.remove(&id);
                    Ok(())
                }
                SurfaceAction::Resize(id) => {
                    app.set_wallpaper(id);
                    Ok(())
                }
            };
            if let Err(e) = result {
                tracing::error!("output surfaces for {action:?}: {e}");
            }
        }
    });
}

pub fn on_config(cfg: Config) {
    with_app(|app| {
        let wall = (&cfg.wallpaper.path, cfg.wallpaper.dark);
        if wall != (&app.cfg.wallpaper.path, app.cfg.wallpaper.dark) {
            app.scheme = scheme_or_fallback(&cfg.wallpaper.path, cfg.wallpaper.dark);
        }
        let new_wallpaper = cfg.wallpaper.path != app.cfg.wallpaper.path;
        let new_edges = (cfg.bar.position, cfg.border.thickness)
            != (app.cfg.bar.position, app.cfg.border.thickness);
        app.cfg = cfg;
        if new_wallpaper {
            app.wallpapers.clear();
        }
        let ids: Vec<u32> = app.outputs.keys().copied().collect();
        for id in ids {
            if new_wallpaper {
                app.set_wallpaper(id);
            }
            if new_edges {
                match app.edges(id) {
                    Ok(edges) => app.outputs.get_mut(&id).unwrap().edges = edges,
                    Err(e) => tracing::error!("bar on output {id}: {e}"),
                }
            }
            let o = &app.outputs[&id];
            theme_bridge::apply(&o.background.1, &app.scheme, &app.cfg);
            theme_bridge::apply(&o.border.1, &app.scheme, &app.cfg);
            theme_bridge::apply(&o.edges.bar.1, &app.scheme, &app.cfg);
            app.update_bar(id);
        }
        tracing::info!("config reloaded");
    });
    // The seconds switch changes how often the clock ticks.
    crate::clock::tick();
}

pub fn on_clock() {
    with_app(|app| {
        let time = crate::clock::now(app.cfg.bar.clock.twelve_hour);
        for o in app.outputs.values() {
            o.edges.bar.1.set_clock_time(time.clone());
        }
    });
}

/// Whether the clock shows seconds, so it must tick every second.
pub fn clock_seconds() -> bool {
    let mut seconds = false;
    with_app(|app| seconds = app.cfg.bar.clock.enabled && app.cfg.bar.clock.show_seconds);
    seconds
}

pub fn on_tray(state: Option<TrayState>) {
    with_app(|app| {
        app.tray = state;
        let entries = app
            .tray
            .as_ref()
            .map(crate::tray::entries)
            .unwrap_or_default();
        for o in app.outputs.values() {
            o.edges
                .bar
                .1
                .set_tray_items(ModelRc::new(VecModel::from(entries.clone())));
        }
    });
}

/// Updates one status service's state and redraws every bar's status icons.
pub fn on_status(update: impl FnOnce(&mut crate::status::Status)) {
    with_app(|app| {
        update(&mut app.status);
        let glyphs = app.status_glyphs();
        for o in app.outputs.values() {
            o.edges
                .bar
                .1
                .set_status_icons(ModelRc::new(VecModel::from(glyphs.clone())));
        }
    });
}

/// Scrolling on the audio icon: one volume step up or down.
pub fn volume_scroll(up: bool) {
    let mut target = None;
    with_app(|app| target = app.status.scrolled_volume(up));
    if let Some(v) = target {
        crate::services::run(async move {
            if let Err(e) = caelum_services::audio::set_volume(v).await {
                tracing::warn!("set volume: {e}");
            }
        });
    }
}

pub fn tray_menu_only(service: &str) -> bool {
    let mut only = false;
    with_app(|app| {
        only = app
            .tray
            .as_ref()
            .is_some_and(|t| t.items.iter().any(|i| i.service == service && i.menu_only))
    });
    only
}

/// A point on the bar in output coordinates, `along` it from its start: what
/// tray items get as the click position.
/// shortcut: output-local; KDE apps ignore it, and global layout is not tracked.
pub fn bar_point(output: u32, along: f32) -> (i32, i32) {
    let mut point = (0, 0);
    with_app(|app| {
        let Some(info) = app.tracker.info(output) else {
            return;
        };
        let half = theme_bridge::bar_thickness(&app.cfg) as i32 / 2;
        let along = along as i32;
        point = match app.cfg.bar.position {
            Edge::Top => (along, half),
            Edge::Bottom => (along, info.height - half),
            Edge::Left => (half, along),
            Edge::Right => (info.width - half, along),
        };
    });
    point
}

/// Applies the current colour scheme to a window created outside `App`.
pub fn apply_theme<C>(ui: &C)
where
    C: ComponentHandle,
    for<'a> caelum_ui::Theme<'a>: slint::Global<'a, C>,
{
    with_app(|app| theme_bridge::apply(ui, &app.scheme, &app.cfg));
}

/// A command from the D-Bus interface or a global shortcut.
pub fn on_command(cmd: caelum_shell::ipc::Command) {
    use caelum_shell::ipc::Command;
    match cmd {
        // shortcut: drawers arrive in 04-2; until then the command is only logged.
        Command::Drawer(c) => tracing::info!("drawer command: {c:?}"),
        Command::Reload => {
            tracing::info!("reload requested");
            match caelum_config::load(&caelum_config::config_path()) {
                Ok(cfg) => on_config(cfg),
                Err(e) => tracing::warn!("reload: {e}"),
            }
        }
    }
}

pub fn on_kwin(state: Option<KwinState>) {
    with_app(|app| {
        app.kwin = state;
        let ids: Vec<u32> = app.outputs.keys().copied().collect();
        for id in ids {
            app.update_bar(id);
        }
    });
}

pub(crate) fn spec(
    namespace: &'static str,
    layer: Layer,
    anchors: Anchors,
    size: (u32, u32),
    zone: i32,
) -> LayerSpec {
    LayerSpec {
        namespace,
        layer,
        anchors,
        size,
        exclusive_zone: zone,
        keyboard: KeyboardMode::None,
        input_region: InputRegion::Empty,
    }
}

impl App {
    fn create(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let all = Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT | Anchors::RIGHT;
        let background = component(
            id,
            spec("caelum-background", Layer::Background, all, (0, 0), -1),
            Background::new,
        )?;
        let border = component(
            id,
            spec("caelum-border", Layer::Top, all, (0, 0), -1),
            Border::new,
        )?;

        let edges = self.edges(id)?;
        theme_bridge::apply(&background.1, &self.scheme, &self.cfg);
        theme_bridge::apply(&border.1, &self.scheme, &self.cfg);
        self.outputs.insert(
            id,
            OutputSurfaces {
                background,
                border,
                edges,
            },
        );
        self.set_wallpaper(id);
        self.update_bar(id);
        Ok(())
    }

    fn status_glyphs(&self) -> Vec<caelum_ui::StatusGlyph> {
        if self.cfg.bar.status_icons {
            self.status.glyphs()
        } else {
            Vec::new()
        }
    }

    fn update_bar(&self, id: u32) {
        if let (Some(o), Some(info)) = (self.outputs.get(&id), self.tracker.info(id)) {
            let b = &o.edges.bar.1;
            b.set_os_logo(self.os_logo.clone());
            b.set_show_tray(self.cfg.bar.tray);
            b.set_status_icons(ModelRc::new(VecModel::from(self.status_glyphs())));
            let entries = self
                .tray
                .as_ref()
                .map(crate::tray::entries)
                .unwrap_or_default();
            b.set_tray_items(ModelRc::new(VecModel::from(entries)));
            bar::update(b, &self.cfg, self.kwin.as_ref(), &info.name);
        }
    }

    /// The bar reserves its own edge; the other edges reserve the border thickness.
    fn edges(&self, id: u32) -> Result<EdgeSurfaces, Box<dyn std::error::Error>> {
        use caelum_config::Edge;
        let bar = bar::create_bar(id, &self.scheme, &self.cfg)?;
        let zone = self.cfg.border.thickness as i32;
        let exclusions = [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right]
            .into_iter()
            .filter(|&e| e != self.cfg.bar.position)
            .map(|e| {
                let size = if matches!(e, Edge::Top | Edge::Bottom) {
                    (0, 1)
                } else {
                    (1, 0)
                };
                component(
                    id,
                    spec(
                        "caelum-exclusion",
                        Layer::Bottom,
                        bar::edge_anchors(e),
                        size,
                        zone,
                    ),
                    Exclusion::new,
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(EdgeSurfaces {
            bar,
            _exclusions: exclusions,
        })
    }

    fn set_wallpaper(&mut self, id: u32) {
        let (Some(info), Some(o)) = (self.tracker.info(id), self.outputs.get(&id)) else {
            return;
        };
        let size = pixel_size(info);
        let image = match self.wallpapers.get(&size) {
            Some(image) => image.clone(),
            None => match load_wallpaper(&self.cfg.wallpaper.path, size) {
                Ok(image) => {
                    self.wallpapers.insert(size, image.clone());
                    image
                }
                Err(e) => {
                    tracing::warn!("wallpaper {}: {e}", self.cfg.wallpaper.path.display());
                    Image::default()
                }
            },
        };
        o.background.1.set_wallpaper(image);
    }
}

pub(crate) fn component<C: ComponentHandle>(
    output: u32,
    spec: LayerSpec,
    new: impl FnOnce() -> Result<C, slint::PlatformError>,
) -> Result<(LayerWindow, C), Box<dyn std::error::Error>> {
    let window = create_layer_window(output, spec).map_err(|e: WaylandError| e.to_string())?;
    let ui = new()?;
    ui.show()?;
    Ok((window, ui))
}

fn pixel_size(info: &OutputInfo) -> (u32, u32) {
    let px = |v: i32| (v as f64 * info.scale).round().max(1.0) as u32;
    (px(info.width), px(info.height))
}

/// Decodes the wallpaper and scales it to cover `(w, h)`, so the shell keeps only
/// an output-sized copy, never the full-resolution image.
fn load_wallpaper(path: &Path, (w, h): (u32, u32)) -> Result<Image, image::ImageError> {
    let rgb = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .resize_to_fill(w, h, image::imageops::FilterType::CatmullRom)
        .into_rgb8();
    let buffer =
        SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(rgb.as_raw(), rgb.width(), rgb.height());
    Ok(Image::from_rgb8(buffer))
}
