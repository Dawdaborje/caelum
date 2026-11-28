// SPDX-License-Identifier: GPL-3.0-or-later
//! Network from NetworkManager's active connections.
use tokio::sync::watch;
use zbus::zvariant::OwnedObjectPath;

use crate::{Service, ServiceError, next_change, prop, publish, signals_from, system_bus};

const DEST: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop/NetworkManager";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
/// NM_ACTIVE_CONNECTION_STATE_ACTIVATED.
const ACTIVATED: u32 = 2;

/// Wired and Wi-Fi are separate because Caelestia shows their icons
/// independently (both while a laptop is docked on Wi-Fi).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NetworkState {
    pub wired: bool,
    pub wifi: Option<Wifi>,
    pub vpn: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wifi {
    pub ssid: String,
    /// 0-100.
    pub strength: u8,
}

/// Caelestia's `Icons.getNetworkIcon` (unsecured variant, as its bar uses).
pub fn wifi_icon(strength: u8) -> &'static str {
    match strength {
        80.. => "network_wifi",
        60.. => "network_wifi_3_bar",
        40.. => "network_wifi_2_bar",
        20.. => "network_wifi_1_bar",
        _ => "signal_wifi_0_bar",
    }
}

#[derive(Default)]
pub struct NetworkService {
    address: Option<String>,
}

impl NetworkService {
    pub fn at(address: &str) -> Self {
        Self {
            address: Some(address.to_owned()),
        }
    }
}

async fn read(conn: &zbus::Connection) -> zbus::Result<NetworkState> {
    let mut state = NetworkState::default();
    let active: Vec<OwnedObjectPath> = prop(
        conn,
        DEST,
        ROOT,
        "org.freedesktop.NetworkManager",
        "ActiveConnections",
    )
    .await?;
    for path in active {
        let path = path.as_str();
        // A connection can vanish between listing and reading; skip it.
        let (Ok(kind), Ok(st)) = (
            prop::<String>(conn, DEST, path, ACTIVE, "Type").await,
            prop::<u32>(conn, DEST, path, ACTIVE, "State").await,
        ) else {
            continue;
        };
        if st != ACTIVATED {
            continue;
        }
        match kind.as_str() {
            "802-3-ethernet" => state.wired = true,
            "vpn" | "wireguard" => state.vpn = true,
            "802-11-wireless" if state.wifi.is_none() => {
                state.wifi = Some(wifi(conn, path).await);
            }
            _ => {}
        }
    }
    Ok(state)
}

/// For a Wi-Fi connection, SpecificObject is its access point.
async fn wifi(conn: &zbus::Connection, active: &str) -> Wifi {
    let id = prop::<String>(conn, DEST, active, ACTIVE, "Id")
        .await
        .unwrap_or_default();
    let Ok(ap) = prop::<OwnedObjectPath>(conn, DEST, active, ACTIVE, "SpecificObject").await else {
        return Wifi {
            ssid: id,
            strength: 0,
        };
    };
    let strength = prop::<u8>(conn, DEST, ap.as_str(), AP, "Strength")
        .await
        .unwrap_or(0);
    let ssid = prop::<Vec<u8>>(conn, DEST, ap.as_str(), AP, "Ssid")
        .await
        .ok()
        .and_then(|s| String::from_utf8(s).ok())
        .filter(|s| !s.is_empty())
        .unwrap_or(id);
    Wifi { ssid, strength }
}

impl Service for NetworkService {
    type State = NetworkState;

    async fn run(self, tx: watch::Sender<Option<NetworkState>>) -> Result<(), ServiceError> {
        let unavailable = |e: zbus::Error| ServiceError::Unavailable(e.to_string());
        let conn = system_bus(self.address.as_deref())
            .await
            .map_err(unavailable)?;
        let mut changes = signals_from(&conn, DEST).await.map_err(unavailable)?;
        publish(&tx, read(&conn).await.map_err(unavailable)?);
        while next_change(&mut changes).await {
            match read(&conn).await {
                Ok(state) => publish(&tx, state),
                Err(e) => tracing::debug!("network: {e}"),
            }
        }
        Err(ServiceError::Failed("NetworkManager signals ended".into()))
    }
}
