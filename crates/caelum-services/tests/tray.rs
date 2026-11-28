// SPDX-License-Identifier: GPL-3.0-or-later
//! Tray host against a private dbus-daemon: no watcher exists there, so the
//! service must act as the watcher too.
mod common;

use std::time::Duration;

use caelum_services::{
    Service,
    tray::{ICON_PX, TrayIcon, TrayService, TrayState},
};
use common::{Bus, private_bus};
use tokio::sync::watch;

struct FakeItem {
    size: i32,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl FakeItem {
    #[zbus(property)]
    fn id(&self) -> String {
        "fake".into()
    }
    #[zbus(property)]
    fn title(&self) -> String {
        "Fake".into()
    }
    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }
    #[zbus(property)]
    fn icon_name(&self) -> String {
        String::new()
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        let n = (self.size * self.size) as usize;
        // Opaque red, ARGB32 in network byte order.
        vec![(self.size, self.size, [255, 255, 0, 0].repeat(n))]
    }
}

async fn fake_item(address: &str, size: i32) -> zbus::Connection {
    let conn = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at("/StatusNotifierItem", FakeItem { size })
        .unwrap()
        .build()
        .await
        .unwrap();
    let name = conn.unique_name().unwrap().to_string();
    // The tray service becomes the watcher shortly after it starts.
    for _ in 0..50 {
        let reply = conn
            .call_method(
                Some("org.kde.StatusNotifierWatcher"),
                "/StatusNotifierWatcher",
                Some("org.kde.StatusNotifierWatcher"),
                "RegisterStatusNotifierItem",
                &name,
            )
            .await;
        if reply.is_ok() {
            return conn;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no watcher appeared");
}

async fn until(
    rx: &mut watch::Receiver<Option<TrayState>>,
    f: impl FnMut(&Option<TrayState>) -> bool,
) -> TrayState {
    tokio::time::timeout(Duration::from_secs(5), rx.wait_for(f))
        .await
        .expect("timed out")
        .unwrap()
        .clone()
        .unwrap()
}

fn start(bus: &Bus) -> watch::Receiver<Option<TrayState>> {
    let (tx, rx) = watch::channel(None);
    let service = TrayService::at(&bus.1);
    tokio::spawn(async move { service.run(tx).await.unwrap() });
    rx
}

#[tokio::test]
async fn item_appears_and_disappears() {
    let bus = private_bus();
    let mut rx = start(&bus);
    let item = fake_item(&bus.1, 16).await;
    let s = until(&mut rx, |s| s.as_ref().is_some_and(|s| s.items.len() == 1)).await;
    assert_eq!(s.items[0].title, "Fake");
    drop(item);
    until(&mut rx, |s| s.as_ref().is_some_and(|s| s.items.is_empty())).await;
}

#[tokio::test]
async fn pixmap_is_downscaled() {
    let bus = private_bus();
    let mut rx = start(&bus);
    let _item = fake_item(&bus.1, 256).await;
    let s = until(&mut rx, |s| s.as_ref().is_some_and(|s| s.items.len() == 1)).await;
    let TrayIcon::Pixmap { w, h, ref rgba } = s.items[0].icon else {
        panic!("not a pixmap: {:?}", s.items[0].icon);
    };
    assert_eq!((w, h), (ICON_PX, ICON_PX));
    assert_eq!(rgba.len(), (ICON_PX * ICON_PX * 4) as usize);
}
