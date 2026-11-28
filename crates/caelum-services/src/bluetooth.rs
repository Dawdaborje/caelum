// SPDX-License-Identifier: GPL-3.0-or-later
//! Bluetooth from BlueZ. Unavailable without bluetoothd or an adapter.
use std::collections::HashMap;

use tokio::sync::watch;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::{Service, ServiceError, next_change, publish, signals_from, system_bus};

const DEST: &str = "org.bluez";

#[derive(Clone, Debug, PartialEq)]
pub struct BluetoothState {
    pub powered: bool,
    pub connected: Vec<BtDevice>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BtDevice {
    pub name: String,
    /// BlueZ's freedesktop icon name, e.g. "audio-headset".
    pub icon: String,
}

/// Caelestia's `Icons.getBluetoothIcon`.
pub fn device_icon(icon: &str) -> &'static str {
    match icon {
        i if i.contains("headset") || i.contains("headphones") => "headphones",
        i if i.contains("audio") => "speaker",
        // Caelestia's "smartphone"; current Material Symbols name the glyph "mobile".
        i if i.contains("phone") => "mobile",
        i if i.contains("mouse") => "mouse",
        i if i.contains("keyboard") => "keyboard",
        _ => "bluetooth",
    }
}

#[derive(Default)]
pub struct BluetoothService {
    address: Option<String>,
}

impl BluetoothService {
    pub fn at(address: &str) -> Self {
        Self {
            address: Some(address.to_owned()),
        }
    }
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

async fn read(conn: &zbus::Connection) -> zbus::Result<Option<BluetoothState>> {
    let reply = conn
        .call_method(
            Some(DEST),
            "/",
            Some("org.freedesktop.DBus.ObjectManager"),
            "GetManagedObjects",
            &(),
        )
        .await?;
    let objects: Objects = reply.body().deserialize()?;
    let flag = |props: &HashMap<String, OwnedValue>, k: &str| {
        props
            .get(k)
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    };
    let text = |props: &HashMap<String, OwnedValue>, k: &str| {
        props
            .get(k)
            .and_then(|v| v.downcast_ref::<String>().ok())
            .unwrap_or_default()
    };
    let adapters: Vec<_> = objects
        .values()
        .filter_map(|i| i.get("org.bluez.Adapter1"))
        .collect();
    if adapters.is_empty() {
        return Ok(None);
    }
    let mut connected: Vec<BtDevice> = objects
        .values()
        .filter_map(|i| i.get("org.bluez.Device1"))
        .filter(|d| flag(d, "Connected"))
        .map(|d| BtDevice {
            name: text(d, "Alias"),
            icon: text(d, "Icon"),
        })
        .collect();
    connected.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(BluetoothState {
        powered: adapters.iter().any(|a| flag(a, "Powered")),
        connected,
    }))
}

impl Service for BluetoothService {
    type State = BluetoothState;

    async fn run(self, tx: watch::Sender<Option<BluetoothState>>) -> Result<(), ServiceError> {
        let unavailable = |e: zbus::Error| ServiceError::Unavailable(e.to_string());
        let conn = system_bus(self.address.as_deref())
            .await
            .map_err(unavailable)?;
        let mut changes = signals_from(&conn, DEST).await.map_err(unavailable)?;
        match read(&conn).await.map_err(unavailable)? {
            Some(state) => publish(&tx, state),
            None => return Err(ServiceError::Unavailable("no Bluetooth adapter".into())),
        }
        while next_change(&mut changes).await {
            match read(&conn).await {
                Ok(Some(state)) => publish(&tx, state),
                Ok(None) => tx.send_replace(None).map_or((), drop),
                Err(e) => tracing::debug!("bluetooth: {e}"),
            }
        }
        Err(ServiceError::Failed("BlueZ signals ended".into()))
    }
}
