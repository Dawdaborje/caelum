// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar's status icons, chosen as Caelestia's StatusIcon and BarWidgets do,
//! in its default order: network, ethernet, bluetooth, audio, battery.
use caelum_services::{
    audio::{self, AudioState},
    battery::{BatteryState, battery_icon},
    bluetooth::{BluetoothState, device_icon},
    network::{NetworkState, wifi_icon},
};
use caelum_ui::{StatusGlyph, icon};

/// Latest state of each status service; None hides its icon.
#[derive(Default)]
pub struct Status {
    pub audio: Option<AudioState>,
    pub network: Option<NetworkState>,
    pub bluetooth: Option<BluetoothState>,
    pub battery: Option<BatteryState>,
}

fn glyph(kind: &str, name: &str, filled: bool, alert: bool) -> StatusGlyph {
    StatusGlyph {
        kind: kind.into(),
        glyph: icon(name).into(),
        filled,
        alert,
    }
}

impl Status {
    pub fn glyphs(&self) -> Vec<StatusGlyph> {
        let mut out = Vec::new();
        if let Some(n) = &self.network {
            // Caelestia's `network` shows Wi-Fi (or wifi_off) unless showWifi is
            // off on a wired machine; showWifi defaults on, so always.
            let name = n
                .wifi
                .as_ref()
                .map_or("wifi_off", |w| wifi_icon(w.strength));
            out.push(glyph("network", name, false, false));
            if n.wired {
                out.push(glyph("ethernet", "cable", false, false));
            }
        }
        if let Some(b) = &self.bluetooth {
            if !b.powered {
                out.push(glyph("bluetooth", "bluetooth_disabled", false, false));
            } else if b.connected.is_empty() {
                out.push(glyph("bluetooth", "bluetooth", false, false));
            }
            for d in b.connected.iter().filter(|_| b.powered) {
                out.push(glyph("bluetooth", device_icon(&d.icon), true, false));
            }
        }
        if let Some(a) = &self.audio {
            out.push(glyph(
                "audio",
                audio::volume_icon(a.volume, a.muted),
                false,
                false,
            ));
        }
        if let Some(b) = &self.battery {
            let low = !b.charging && b.percent <= 20.0;
            out.push(glyph(
                "battery",
                &battery_icon(b.percent, b.charging),
                true,
                low,
            ));
        }
        out
    }

    /// Caelestia's audioIncrement (0.1), from the current volume.
    pub fn scrolled_volume(&self, up: bool) -> Option<f32> {
        let v = self.audio.as_ref()?.volume;
        Some(if up { v + 0.1 } else { v - 0.1 })
    }
}

#[cfg(test)]
mod tests {
    use caelum_services::bluetooth::BtDevice;

    use super::*;

    fn kinds(s: &Status) -> Vec<String> {
        s.glyphs().iter().map(|g| g.kind.to_string()).collect()
    }

    #[test]
    fn order_and_presence_follow_caelestia() {
        let mut s = Status::default();
        assert!(s.glyphs().is_empty());
        s.network = Some(NetworkState {
            wired: true,
            ..Default::default()
        });
        s.bluetooth = Some(BluetoothState {
            powered: true,
            connected: vec![BtDevice {
                name: "Buds".into(),
                icon: "audio-headset".into(),
            }],
        });
        s.audio = Some(AudioState::default());
        s.battery = Some(BatteryState {
            percent: 10.0,
            charging: false,
            time_to_empty_s: None,
        });
        assert_eq!(
            kinds(&s),
            ["network", "ethernet", "bluetooth", "audio", "battery"]
        );
        let g = s.glyphs();
        assert_eq!(g[0].glyph, icon("wifi_off"));
        assert_eq!(g[2].glyph, icon("headphones"));
        assert!(g[4].alert && g[4].filled);
    }
}
