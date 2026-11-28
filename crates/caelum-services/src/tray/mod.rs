// SPDX-License-Identifier: GPL-3.0-or-later
//! System tray host (StatusNotifierItem). Normally a host on Plasma's watcher;
//! when the session has no watcher, it serves one itself.
mod item;
mod menu;
mod watcher;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use futures_util::StreamExt;
pub use menu::{MenuItem, Toggle};
use tokio::sync::watch;
use zbus::{
    Connection, MatchRule, MessageStream, fdo::DBusProxy, message::Type, zvariant::OwnedObjectPath,
};

use crate::{Service, ServiceError};

/// Tray icons are stored at this size: Caelestia's icon.small (15pt) at
/// 1.33 px/pt and its bar icon scale of 1.2.
/// shortcut: logical size at scale 1; scale by the largest output scale for HiDPI.
pub const ICON_PX: u32 = 24;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TrayState {
    pub items: Vec<TrayItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayItem {
    /// The registered item string; the key for [`activate`] and [`menu`].
    pub service: String,
    pub id: String,
    pub title: String,
    pub icon: TrayIcon,
    pub status: Status,
    /// The item only has a menu: a left click should open it, not activate.
    pub menu_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrayIcon {
    /// A theme icon name that did not resolve to a loadable file.
    Name(String),
    /// Straight-alpha RGBA8, `ICON_PX` square.
    Pixmap { w: u32, h: u32, rgba: Arc<[u8]> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Passive,
    Active,
    NeedsAttention,
}

/// Where each item lives, for requests from the UI thread.
struct Address {
    bus: String,
    path: String,
    menu: Option<OwnedObjectPath>,
}

static CONN: OnceLock<Connection> = OnceLock::new();
static ADDRESSES: Mutex<Option<HashMap<String, Address>>> = Mutex::new(None);

fn address_of(service: &str) -> Option<(String, String, Option<OwnedObjectPath>)> {
    let map = ADDRESSES.lock().unwrap();
    let a = map.as_ref()?.get(service)?;
    Some((a.bus.clone(), a.path.clone(), a.menu.clone()))
}

async fn call(service: &str, method: &str, x: i32, y: i32) -> zbus::Result<()> {
    let (Some(conn), Some((bus, path, _))) = (CONN.get(), address_of(service)) else {
        return Ok(());
    };
    conn.call_method(
        Some(bus.as_str()),
        path.as_str(),
        Some(item::INTERFACE),
        method,
        &(x, y),
    )
    .await?;
    Ok(())
}

/// Left click: the item's primary action, at screen position (x, y).
pub async fn activate(service: &str, x: i32, y: i32) -> zbus::Result<()> {
    call(service, "Activate", x, y).await
}

/// Middle click: the item's secondary action.
pub async fn secondary_activate(service: &str, x: i32, y: i32) -> zbus::Result<()> {
    call(service, "SecondaryActivate", x, y).await
}

async fn menu_proxy(service: &str) -> zbus::Result<Option<menu::DBusMenuProxy<'static>>> {
    let (Some(conn), Some((bus, _, Some(path)))) = (CONN.get(), address_of(service)) else {
        return Ok(None);
    };
    let proxy = menu::DBusMenuProxy::builder(conn)
        .destination(bus)?
        .path(path)?
        .build()
        .await?;
    Ok(Some(proxy))
}

/// The item's context menu, or None if it has none.
pub async fn menu(service: &str) -> zbus::Result<Option<Vec<MenuItem>>> {
    let Some(proxy) = menu_proxy(service).await? else {
        return Ok(None);
    };
    // Apps may fill the menu lazily; the answer only says whether to re-read.
    let _ = proxy.about_to_show(0).await;
    let (_, layout) = proxy.get_layout(0, -1, &[]).await?;
    Ok(Some(layout.into_items()))
}

/// Tells the app a menu entry was clicked.
pub async fn menu_clicked(service: &str, id: i32) -> zbus::Result<()> {
    if let Some(proxy) = menu_proxy(service).await? {
        proxy
            .event(id, "clicked", &zbus::zvariant::Value::I32(0), 0)
            .await?;
    }
    Ok(())
}

pub struct TrayService {
    address: Option<String>,
}

impl Default for TrayService {
    /// On the session bus.
    fn default() -> Self {
        Self { address: None }
    }
}

impl TrayService {
    /// On the bus at `address` (tests use a private dbus-daemon).
    pub fn at(address: &str) -> Self {
        Self {
            address: Some(address.to_owned()),
        }
    }
}

struct Entry {
    owner: String,
    item: TrayItem,
}

struct Host {
    conn: Connection,
    style: item::IconStyle,
    own_watcher: bool,
    entries: Vec<Entry>,
}

impl Host {
    async fn add(&mut self, service: String) {
        if self.entries.iter().any(|e| e.item.service == service) {
            return;
        }
        let (bus, path) = item::address(&service);
        let owner = if bus.starts_with(':') {
            bus.clone()
        } else {
            match self.owner_of(&bus).await {
                Ok(o) => o,
                Err(e) => return tracing::debug!("tray item {service}: {e}"),
            }
        };
        match item::fetch(&self.conn, &service, &bus, &path, &self.style).await {
            Ok(f) => {
                ADDRESSES.lock().unwrap().get_or_insert_default().insert(
                    service,
                    Address {
                        bus,
                        path,
                        menu: f.menu,
                    },
                );
                self.entries.push(Entry {
                    owner,
                    item: f.item,
                });
            }
            Err(e) => tracing::debug!("tray item {service}: {e}"),
        }
    }

    async fn owner_of(&self, bus: &str) -> zbus::Result<String> {
        let name = zbus::names::BusName::try_from(bus)?;
        Ok(DBusProxy::new(&self.conn)
            .await?
            .get_name_owner(name)
            .await?
            .to_string())
    }

    async fn refetch(&mut self, owner: &str) {
        for e in self.entries.iter_mut().filter(|e| e.owner == owner) {
            let (bus, path) = item::address(&e.item.service);
            match item::fetch(&self.conn, &e.item.service, &bus, &path, &self.style).await {
                Ok(f) => {
                    if let Some(a) = ADDRESSES
                        .lock()
                        .unwrap()
                        .get_or_insert_default()
                        .get_mut(&e.item.service)
                    {
                        a.menu = f.menu;
                    }
                    e.item = f.item;
                }
                Err(err) => tracing::debug!("tray item {}: {err}", e.item.service),
            }
        }
    }

    async fn remove(&mut self, gone: impl Fn(&Entry) -> bool) {
        let removed: Vec<String> = self
            .entries
            .iter()
            .filter(|e| gone(e))
            .map(|e| e.item.service.clone())
            .collect();
        self.entries.retain(|e| !gone(e));
        if let Some(map) = ADDRESSES.lock().unwrap().as_mut() {
            for s in &removed {
                map.remove(s);
            }
        }
        if self.own_watcher {
            self.unregister_from_own_watcher(&removed).await;
        }
    }

    async fn unregister_from_own_watcher(&self, removed: &[String]) {
        let Ok(iface) = self
            .conn
            .object_server()
            .interface::<_, watcher::Watcher>(watcher::PATH)
            .await
        else {
            return;
        };
        iface.get_mut().await.items.retain(|s| !removed.contains(s));
        for s in removed {
            let _ = watcher::Watcher::status_notifier_item_unregistered(iface.signal_emitter(), s)
                .await;
        }
    }

    fn state(&self) -> TrayState {
        TrayState {
            items: self.entries.iter().map(|e| e.item.clone()).collect(),
        }
    }
}

impl Service for TrayService {
    type State = TrayState;

    async fn run(self, tx: watch::Sender<Option<TrayState>>) -> Result<(), ServiceError> {
        let unavailable = |e: zbus::Error| ServiceError::Unavailable(e.to_string());
        let failed = |e: zbus::Error| ServiceError::Failed(e.to_string());
        let conn = match &self.address {
            Some(a) => zbus::connection::Builder::address(a.as_str())
                .map_err(unavailable)?
                .build()
                .await
                .map_err(unavailable)?,
            None => Connection::session().await.map_err(unavailable)?,
        };
        let _ = CONN.set(conn.clone());
        let dbus = DBusProxy::new(&conn).await.map_err(failed)?;

        // Subscribe before registering, so no change slips between.
        let mut owners = dbus.receive_name_owner_changed().await.map_err(failed)?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(item::INTERFACE)
            .map_err(failed)?
            .build();
        let mut item_signals = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .map_err(failed)?;

        let has_watcher = dbus
            .name_has_owner(
                watcher::NAME
                    .try_into()
                    .map_err(|e: zbus::names::Error| failed(e.into()))?,
            )
            .await
            .map_err(|e| failed(e.into()))?;
        let mut own_watcher = false;
        if !has_watcher {
            conn.object_server()
                .at(watcher::PATH, watcher::Watcher::default())
                .await
                .map_err(failed)?;
            own_watcher = conn.request_name(watcher::NAME).await.is_ok();
        }
        let client = watcher::WatcherClientProxy::new(&conn)
            .await
            .map_err(failed)?;
        let mut registered = client
            .receive_status_notifier_item_registered()
            .await
            .map_err(failed)?;
        let mut unregistered = client
            .receive_status_notifier_item_unregistered()
            .await
            .map_err(failed)?;
        let host_name = format!("org.kde.StatusNotifierHost-{}", std::process::id());
        conn.request_name(host_name.as_str())
            .await
            .map_err(failed)?;
        client
            .register_status_notifier_host(&host_name)
            .await
            .map_err(unavailable)?;

        let mut host = Host {
            conn: conn.clone(),
            style: item::IconStyle::load(),
            own_watcher,
            entries: Vec::new(),
        };
        for service in client
            .registered_status_notifier_items()
            .await
            .unwrap_or_default()
        {
            host.add(service).await;
        }
        let publish = |host: &Host| {
            let state = host.state();
            tx.send_if_modified(|s| {
                let changed = s.as_ref() != Some(&state);
                if changed {
                    *s = Some(state);
                }
                changed
            });
        };
        publish(&host);

        loop {
            tokio::select! {
                Some(sig) = registered.next() => {
                    if let Ok(args) = sig.args() {
                        host.add(args.service).await;
                    }
                }
                Some(sig) = unregistered.next() => {
                    if let Ok(args) = sig.args() {
                        let service = args.service;
                        host.remove(|e| e.item.service == service).await;
                    }
                }
                Some(change) = owners.next() => {
                    let Ok(args) = change.args() else { continue };
                    if args.new_owner().is_none()
                        && let Some(old) = args.old_owner().as_ref()
                    {
                        let old = old.to_string();
                        host.remove(|e| e.owner == old).await;
                    }
                }
                Some(Ok(msg)) = item_signals.next() => {
                    if let Some(sender) = msg.header().sender() {
                        host.refetch(sender.as_str()).await;
                    }
                }
                else => return Err(ServiceError::Failed("tray streams ended".into())),
            }
            publish(&host);
        }
    }
}
