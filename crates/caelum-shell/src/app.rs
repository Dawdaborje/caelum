// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-output surfaces: wallpaper, border frame and edge exclusion zones.
use std::{cell::RefCell, collections::HashMap, path::Path};

use caelum_config::Config;
use caelum_theme::{Scheme, scheme_or_fallback};
use caelum_ui::{Background, Bar, Border, Exclusion};
use caelum_wayland::{
    Anchors, InputRegion, KeyboardMode, Layer, LayerSpec, LayerWindow, OutputEvent, OutputInfo,
    OutputTracker, SurfaceAction, WaylandError, create_layer_window,
};
use slint::{ComponentHandle, Image, Rgb8Pixel, SharedPixelBuffer};

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
}

pub fn init(cfg: Config) {
    let scheme = scheme_or_fallback(&cfg.wallpaper.path, cfg.wallpaper.dark);
    let app = App {
        cfg,
        scheme,
        tracker: OutputTracker::default(),
        outputs: HashMap::new(),
        wallpapers: HashMap::new(),
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
        }
        tracing::info!("config reloaded");
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
        Ok(())
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
