// SPDX-License-Identifier: GPL-3.0-or-later
//! Tray icons on the bar, their clicks, and the context-menu window.
use std::cell::RefCell;

use caelum_services::tray::{self, MenuItem, TrayIcon, TrayState};
use caelum_ui::{MenuGroup, MenuRow, TrayEntry, TrayMenu};
use caelum_wayland::{
    Anchors, InputRegion, KeyboardMode, Layer, LayerSpec, LayerWindow, create_layer_window,
};
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::{app, services};

pub fn entries(state: &TrayState) -> Vec<TrayEntry> {
    state
        .items
        .iter()
        .map(|item| TrayEntry {
            service: item.service.as_str().into(),
            title: (if item.title.is_empty() {
                &item.id
            } else {
                &item.title
            })
            .as_str()
            .into(),
            icon: match &item.icon {
                TrayIcon::Pixmap { w, h, rgba } => Image::from_rgba8(
                    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba, *w, *h),
                ),
                TrayIcon::Name(_) => Image::default(),
            },
        })
        .collect()
}

/// Left click: activate, or open the menu of an item that is only a menu.
pub fn activate(output: u32, service: String, along: f32) {
    if app::tray_menu_only(&service) {
        return open_menu(output, service, along);
    }
    let (x, y) = app::bar_point(output, along);
    services::run(async move {
        if let Err(e) = tray::activate(&service, x, y).await {
            tracing::debug!("tray activate {service}: {e}");
        }
    });
}

pub fn secondary(output: u32, service: String, along: f32) {
    let (x, y) = app::bar_point(output, along);
    services::run(async move {
        if let Err(e) = tray::secondary_activate(&service, x, y).await {
            tracing::debug!("tray secondary {service}: {e}");
        }
    });
}

/// Right click: the item's menu, or (as Caelestia) its secondary action if it has none.
pub fn open_menu(output: u32, service: String, along: f32) {
    let (x, y) = app::bar_point(output, along);
    services::run(async move {
        match tray::menu(&service).await {
            Ok(Some(items)) if !items.is_empty() => {
                let _ = slint::invoke_from_event_loop(move || show(output, service, items, along));
            }
            Ok(_) => {
                if let Err(e) = tray::secondary_activate(&service, x, y).await {
                    tracing::debug!("tray secondary {service}: {e}");
                }
            }
            Err(e) => tracing::warn!("tray menu {service}: {e}"),
        }
    });
}

struct Open {
    _window: LayerWindow,
    ui: TrayMenu,
    service: String,
    tree: Vec<MenuItem>,
    /// Indices into `tree` down to the submenu being shown.
    path: Vec<usize>,
}

thread_local! {
    static MENU: RefCell<Option<Open>> = const { RefCell::new(None) };
}

fn show(output: u32, service: String, tree: Vec<MenuItem>, along: f32) {
    close_now();
    let all = Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT | Anchors::RIGHT;
    let spec = LayerSpec {
        namespace: "caelum-tray-menu",
        layer: Layer::Overlay,
        anchors: all,
        size: (0, 0),
        exclusive_zone: -1,
        keyboard: KeyboardMode::Exclusive,
        input_region: InputRegion::Full,
    };
    let window = match create_layer_window(output, spec) {
        Ok(w) => w,
        Err(e) => return tracing::error!("tray menu window: {e}"),
    };
    let ui = match TrayMenu::new() {
        Ok(ui) => ui,
        Err(e) => return tracing::error!("tray menu: {e}"),
    };
    app::apply_theme(&ui);
    ui.set_anchor(along);
    ui.on_clicked(|row| clicked(row.id, row.submenu));
    ui.on_back(|| {
        navigate(|_, path| {
            path.pop();
        })
    });
    ui.on_dismiss(close);
    if let Err(e) = ui.show() {
        return tracing::error!("tray menu: {e}");
    }
    MENU.with(|m| {
        *m.borrow_mut() = Some(Open {
            _window: window,
            ui,
            service,
            tree,
            path: Vec::new(),
        })
    });
    navigate(|_, _| {});
}

/// Changes the shown level, then pushes its groups into the UI.
fn navigate(f: impl FnOnce(&[MenuItem], &mut Vec<usize>)) {
    MENU.with(|m| {
        let mut m = m.borrow_mut();
        let Some(open) = m.as_mut() else { return };
        f(&open.tree, &mut open.path);
        let level = level(&open.tree, &open.path);
        open.ui.set_submenu(!open.path.is_empty());
        open.ui
            .set_groups(ModelRc::new(VecModel::from(groups(level))));
    });
}

fn level<'a>(tree: &'a [MenuItem], path: &[usize]) -> &'a [MenuItem] {
    path.iter()
        .try_fold(tree, |items, &i| {
            items.get(i).map(|it| it.children.as_slice())
        })
        .unwrap_or(tree)
}

/// Rows split into Caelestia's cards at each separator.
fn groups(items: &[MenuItem]) -> Vec<MenuGroup> {
    items
        .split(|i| i.separator)
        .filter(|g| !g.is_empty())
        .map(|g| MenuGroup {
            rows: ModelRc::new(VecModel::from(
                g.iter()
                    .map(|i| MenuRow {
                        id: i.id,
                        label: i.label.as_str().into(),
                        enabled: i.enabled,
                        submenu: !i.children.is_empty(),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect()
}

fn clicked(id: i32, submenu: bool) {
    if submenu {
        return navigate(|tree, path| {
            if let Some(i) = level(tree, path).iter().position(|it| it.id == id) {
                path.push(i);
            }
        });
    }
    let service = MENU.with(|m| m.borrow().as_ref().map(|o| o.service.clone()));
    if let Some(service) = service {
        services::run(async move {
            if let Err(e) = tray::menu_clicked(&service, id).await {
                tracing::debug!("tray menu click {service}: {e}");
            }
        });
    }
    close();
}

/// Closes after the current Slint callback returns: the menu's own callbacks
/// call this, and dropping a component inside its callback is not allowed.
fn close() {
    let _ = slint::invoke_from_event_loop(close_now);
}

fn close_now() {
    // Take first so the drop happens with the RefCell released.
    let open = MENU.with(|m| m.borrow_mut().take());
    drop(open);
}
