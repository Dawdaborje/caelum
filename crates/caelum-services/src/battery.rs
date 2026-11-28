// SPDX-License-Identifier: GPL-3.0-or-later
//! Battery from UPower's DisplayDevice. Unavailable without UPower or without a
//! battery (a desktop PC), so the bar hides the icon.
use tokio::sync::watch;

use crate::{Service, ServiceError, next_change, prop, publish, signals_from, system_bus};

const DEST: &str = "org.freedesktop.UPower";
const PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const IFACE: &str = "org.freedesktop.UPower.Device";

#[derive(Clone, Debug, PartialEq)]
pub struct BatteryState {
    pub percent: f32,
    pub charging: bool,
    pub time_to_empty_s: Option<u64>,
}

/// Caelestia's `Icons.getBatteryIcon`: seven bars, charging icons in tens.
pub fn battery_icon(percent: f32, charging: bool) -> String {
    let p = percent / 100.0;
    if p >= 1.0 {
        return (if charging {
            "battery_charging_full"
        } else {
            "battery_full"
        })
        .into();
    }
    let mut level = (p.max(0.0) * 7.0).floor() as u32;
    if charging && (level == 4 || level == 1) {
        level -= 1;
    }
    if charging {
        format!("battery_charging_{}", (level + 3) * 10)
    } else {
        format!("battery_{level}_bar")
    }
}

#[derive(Default)]
pub struct BatteryService {
    address: Option<String>,
}

impl BatteryService {
    pub fn at(address: &str) -> Self {
        Self {
            address: Some(address.to_owned()),
        }
    }
}

async fn read(conn: &zbus::Connection) -> zbus::Result<Option<BatteryState>> {
    if !prop::<bool>(conn, DEST, PATH, IFACE, "IsPresent").await? {
        return Ok(None);
    }
    let percent = prop::<f64>(conn, DEST, PATH, IFACE, "Percentage").await? as f32;
    // UPower states: 1 charging, 4 fully charged, 5 pending charge.
    let state = prop::<u32>(conn, DEST, PATH, IFACE, "State").await?;
    let empty = prop::<i64>(conn, DEST, PATH, IFACE, "TimeToEmpty").await?;
    Ok(Some(BatteryState {
        percent,
        charging: matches!(state, 1 | 4 | 5),
        time_to_empty_s: u64::try_from(empty).ok().filter(|&s| s > 0),
    }))
}

impl Service for BatteryService {
    type State = BatteryState;

    async fn run(self, tx: watch::Sender<Option<BatteryState>>) -> Result<(), ServiceError> {
        let unavailable = |e: zbus::Error| ServiceError::Unavailable(e.to_string());
        let conn = system_bus(self.address.as_deref())
            .await
            .map_err(unavailable)?;
        let mut changes = signals_from(&conn, DEST).await.map_err(unavailable)?;
        match read(&conn).await.map_err(unavailable)? {
            Some(state) => publish(&tx, state),
            None => return Err(ServiceError::Unavailable("no battery".into())),
        }
        while next_change(&mut changes).await {
            match read(&conn).await {
                Ok(Some(state)) => publish(&tx, state),
                Ok(None) => tx.send_replace(None).map_or((), drop),
                Err(e) => tracing::debug!("battery: {e}"),
            }
        }
        Err(ServiceError::Failed("UPower signals ended".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_match_caelestia() {
        assert_eq!(battery_icon(100.0, false), "battery_full");
        assert_eq!(battery_icon(42.0, false), "battery_2_bar");
        assert_eq!(battery_icon(20.0, true), "battery_charging_30");
        assert_eq!(battery_icon(60.0, true), "battery_charging_60");
        assert_eq!(battery_icon(95.0, true), "battery_charging_90");
    }
}
