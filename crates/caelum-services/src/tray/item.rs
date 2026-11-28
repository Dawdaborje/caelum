// SPDX-License-Identifier: GPL-3.0-or-later
//! One StatusNotifierItem: where it lives, its properties, and its icon as an
//! `ICON_PX` RGBA pixmap.
use std::{path::Path, sync::Arc};

use image::{RgbaImage, imageops::FilterType};
use zbus::{Connection, zvariant::OwnedObjectPath};

use super::{ICON_PX, Status, TrayIcon, TrayItem};

pub const INTERFACE: &str = "org.kde.StatusNotifierItem";

/// A registered item string, "bus/path" or just "bus", split into bus and path.
pub fn address(service: &str) -> (String, String) {
    match service.find('/') {
        Some(i) => (service[..i].to_owned(), service[i..].to_owned()),
        None => (service.to_owned(), "/StatusNotifierItem".to_owned()),
    }
}

/// What the host keeps per item besides the published [`TrayItem`].
pub struct Fetched {
    pub item: TrayItem,
    pub menu: Option<OwnedObjectPath>,
}

pub async fn fetch(
    conn: &Connection,
    service: &str,
    bus: &str,
    path: &str,
    style: &IconStyle,
) -> zbus::Result<Fetched> {
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(bus.to_owned())?
        .path(path.to_owned())?
        .build()
        .await?
        .get_all(zbus::names::InterfaceName::from_static_str_unchecked(
            INTERFACE,
        ))
        .await?;
    let string = |k: &str| -> String {
        props
            .get(k)
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default()
    };
    let status = match string("Status").as_str() {
        "Passive" => Status::Passive,
        "NeedsAttention" => Status::NeedsAttention,
        _ => Status::Active,
    };
    let pixmaps = props
        .get("IconPixmap")
        .and_then(|v| Vec::<(i32, i32, Vec<u8>)>::try_from(v.clone()).ok())
        .unwrap_or_default();
    let name = string("IconName");
    let icon = match pixmap_icon(&pixmaps) {
        Some(icon) => icon,
        None => named_icon(&name, &string("IconThemePath"), style).unwrap_or(TrayIcon::Name(name)),
    };
    let menu = props
        .get("Menu")
        .and_then(|v| OwnedObjectPath::try_from(v.clone()).ok())
        .filter(|p| p.as_str() != "/");
    let menu_only = props
        .get("ItemIsMenu")
        .and_then(|v| bool::try_from(v.clone()).ok())
        .unwrap_or(false);
    Ok(Fetched {
        item: TrayItem {
            service: service.to_owned(),
            id: string("Id"),
            title: string("Title"),
            icon,
            status,
            menu_only,
        },
        menu,
    })
}

/// The smallest pixmap at least `ICON_PX` (else the largest), ARGB32 in network
/// byte order, converted and scaled once.
fn pixmap_icon(pixmaps: &[(i32, i32, Vec<u8>)]) -> Option<TrayIcon> {
    let valid = pixmaps
        .iter()
        .filter(|(w, h, d)| *w > 0 && *h > 0 && d.len() == (*w * *h * 4) as usize);
    let best = valid
        .clone()
        .filter(|(w, _, _)| *w >= ICON_PX as i32)
        .min_by_key(|(w, _, _)| *w)
        .or_else(|| valid.max_by_key(|(w, _, _)| *w))?;
    let (w, h, argb) = best;
    let rgba: Vec<u8> = argb
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[a, r, g, b]| [*r, *g, *b, *a])
        .collect();
    RgbaImage::from_raw(*w as u32, *h as u32, rgba).map(sized)
}

fn sized(img: RgbaImage) -> TrayIcon {
    let img = if img.dimensions() == (ICON_PX, ICON_PX) {
        img
    } else {
        image::imageops::resize(&img, ICON_PX, ICON_PX, FilterType::Triangle)
    };
    TrayIcon::Pixmap {
        w: ICON_PX,
        h: ICON_PX,
        rgba: Arc::from(img.into_raw()),
    }
}

/// An icon name looked up in the item's own theme path, then the icon theme.
fn named_icon(name: &str, theme_path: &str, style: &IconStyle) -> Option<TrayIcon> {
    if name.is_empty() {
        return None;
    }
    if name.starts_with('/') {
        return load(Path::new(name), &style.stylesheet);
    }
    let own = (!theme_path.is_empty()).then(|| {
        ["svg", "png"]
            .iter()
            .map(|ext| Path::new(theme_path).join(format!("{name}.{ext}")))
            .find(|p| p.exists())
    });
    let path = own.flatten().or_else(|| {
        freedesktop_icons::lookup(name)
            .with_theme(&style.theme)
            .with_size(ICON_PX as u16)
            .find()
    })?;
    load(&path, &style.stylesheet)
}

/// Loads a PNG or SVG icon file at `ICON_PX`; SVGs get `stylesheet` applied.
pub fn load(path: &Path, stylesheet: &str) -> Option<TrayIcon> {
    if path.extension().is_some_and(|e| e == "svg") {
        let data = recolour(&std::fs::read_to_string(path).ok()?, stylesheet);
        let options = resvg::usvg::Options {
            style_sheet: (!stylesheet.is_empty()).then(|| stylesheet.to_owned()),
            ..Default::default()
        };
        let tree = resvg::usvg::Tree::from_str(&data, &options).ok()?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(ICON_PX, ICON_PX)?;
        let size = tree.size();
        let scale = ICON_PX as f32 / size.width().max(size.height());
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        // tiny-skia is premultiplied; Slint's from_rgba8 wants straight alpha.
        let rgba = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        return RgbaImage::from_raw(ICON_PX, ICON_PX, rgba).map(sized);
    }
    match image::open(path) {
        Ok(img) => Some(sized(img.into_rgba8())),
        Err(e) => {
            tracing::debug!("tray icon {}: {e}", path.display());
            None
        }
    }
}

/// Swaps the body of an SVG's `current-color-scheme` style for `stylesheet`, as
/// KIconLoader does; the embedded rule would otherwise beat an injected one.
fn recolour(svg: &str, stylesheet: &str) -> String {
    let swapped = (|| {
        let id = svg.find("id=\"current-color-scheme\"")?;
        let open = id + svg[id..].find('>')? + 1;
        let close = open + svg[open..].find("</style>")?;
        Some(format!("{}{stylesheet}{}", &svg[..open], &svg[close..]))
    })();
    match swapped {
        Some(s) if !stylesheet.is_empty() => s,
        _ => svg.to_owned(),
    }
}

/// How Plasma draws icons: its icon theme, and the stylesheet KIconLoader gives
/// SVG icons so `ColorScheme-*` classes (Breeze's symbolic icons) take the colour
/// scheme's colours instead of their dark built-in defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IconStyle {
    pub theme: String,
    pub stylesheet: String,
}

impl IconStyle {
    /// From `kdeglobals` in the config dir; breeze and no recolouring without it.
    pub fn load() -> Self {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")));
        let text = config
            .and_then(|c| std::fs::read_to_string(c.join("kdeglobals")).ok())
            .unwrap_or_default();
        Self::parse(&text)
    }

    fn parse(kdeglobals: &str) -> Self {
        let mut section = "";
        let mut values = std::collections::HashMap::new();
        for line in kdeglobals.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name;
            } else if let Some((k, v)) = line.split_once('=') {
                values.insert((section, k), v);
            }
        }
        let colour = |section: &str, key: &str| -> Option<String> {
            let v = values.get(&(section, key))?;
            if v.starts_with('#') {
                return Some(v.to_string());
            }
            let rgb: Vec<u8> = v
                .split(',')
                .map(|c| c.trim().parse().ok())
                .collect::<Option<_>>()?;
            (rgb.len() >= 3).then(|| format!("rgb({},{},{})", rgb[0], rgb[1], rgb[2]))
        };
        // KIconColors' classes and where Plasma takes them from.
        let classes = [
            ("Text", "Colors:Window", "ForegroundNormal"),
            ("Background", "Colors:Window", "BackgroundNormal"),
            ("Highlight", "Colors:Selection", "BackgroundNormal"),
            ("HighlightedText", "Colors:Selection", "ForegroundNormal"),
            ("PositiveText", "Colors:Window", "ForegroundPositive"),
            ("NeutralText", "Colors:Window", "ForegroundNeutral"),
            ("NegativeText", "Colors:Window", "ForegroundNegative"),
        ];
        let stylesheet = classes
            .iter()
            .filter_map(|(class, section, key)| {
                Some(format!(
                    ".ColorScheme-{class}{{color:{};}}",
                    colour(section, key)?
                ))
            })
            .collect();
        Self {
            theme: values
                .get(&("Icons", "Theme"))
                .map_or_else(|| "breeze".to_owned(), |t| t.to_string()),
            stylesheet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_style_from_kdeglobals() {
        let s = IconStyle::parse(
            "[Colors:Window]\nForegroundNormal=#eee0d5\nBackgroundNormal=37,30,23\n[Icons]\nTheme=Desert\n",
        );
        assert_eq!(s.theme, "Desert");
        assert!(s.stylesheet.contains(".ColorScheme-Text{color:#eee0d5;}"));
        assert!(
            s.stylesheet
                .contains(".ColorScheme-Background{color:rgb(37,30,23);}")
        );
        assert_eq!(IconStyle::parse("").theme, "breeze");
    }

    #[test]
    fn embedded_scheme_style_is_replaced() {
        let svg = r#"<svg><style id="current-color-scheme">.ColorScheme-Text{color:#232629;}</style></svg>"#;
        let out = recolour(svg, ".ColorScheme-Text{color:#eee0d5;}");
        assert!(out.contains("#eee0d5") && !out.contains("#232629"), "{out}");
        assert_eq!(recolour("<svg/>", ".x{}"), "<svg/>");
    }

    #[test]
    fn address_splits_path() {
        assert_eq!(
            address(":1.5"),
            (":1.5".into(), "/StatusNotifierItem".into())
        );
        assert_eq!(
            address(":1.5/org/ayatana/NotificationItem/x"),
            (":1.5".into(), "/org/ayatana/NotificationItem/x".into())
        );
    }

    #[test]
    fn pixmap_prefers_smallest_at_least_icon_size() {
        let px = |s: i32| (s, s, vec![255u8; (s * s * 4) as usize]);
        let Some(TrayIcon::Pixmap { w, .. }) = pixmap_icon(&[px(16), px(32), px(64)]) else {
            panic!()
        };
        assert_eq!(w, ICON_PX);
        // Malformed data is skipped rather than trusted.
        assert!(pixmap_icon(&[(4, 4, vec![0; 3])]).is_none());
    }
}
