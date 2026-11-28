// SPDX-License-Identifier: GPL-3.0-or-later
//! Status services against a private bus standing in for the system bus.
mod common;

use std::time::Duration;

use caelum_services::{
    Service, ServiceError,
    battery::{BatteryService, BatteryState},
    bluetooth::BluetoothService,
    network::wifi_icon,
};
use common::private_bus;
use tokio::sync::watch;

#[tokio::test]
async fn absent_service_hides_module() {
    let bus = private_bus();

    let (tx, rx) = watch::channel(None);
    let r = BatteryService::at(&bus.1).run(tx).await;
    assert!(matches!(r, Err(ServiceError::Unavailable(_))), "{r:?}");
    assert!(rx.borrow().is_none());

    let (tx, rx) = watch::channel(None);
    let r = BluetoothService::at(&bus.1).run(tx).await;
    assert!(matches!(r, Err(ServiceError::Unavailable(_))), "{r:?}");
    assert!(rx.borrow().is_none());
}

struct FakeDisplayDevice;

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl FakeDisplayDevice {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        42.0
    }
    /// Discharging.
    #[zbus(property)]
    fn state(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn time_to_empty(&self) -> i64 {
        3600
    }
}

#[tokio::test]
async fn battery_present_reports_percent() {
    let bus = private_bus();
    let _upower = zbus::connection::Builder::address(bus.1.as_str())
        .unwrap()
        .name("org.freedesktop.UPower")
        .unwrap()
        .serve_at(
            "/org/freedesktop/UPower/devices/DisplayDevice",
            FakeDisplayDevice,
        )
        .unwrap()
        .build()
        .await
        .unwrap();

    let (tx, mut rx) = watch::channel(None);
    let service = BatteryService::at(&bus.1);
    tokio::spawn(async move { service.run(tx).await });
    let state: BatteryState =
        tokio::time::timeout(Duration::from_secs(5), rx.wait_for(Option::is_some))
            .await
            .expect("timed out")
            .unwrap()
            .clone()
            .unwrap();
    assert_eq!(state.percent, 42.0);
    assert!(!state.charging);
    assert_eq!(state.time_to_empty_s, Some(3600));
}

#[test]
fn wifi_icon_buckets_match_caelestia() {
    assert_eq!(wifi_icon(0), "signal_wifi_0_bar");
    assert_eq!(wifi_icon(30), "network_wifi_1_bar");
    assert_eq!(wifi_icon(60), "network_wifi_3_bar");
    assert_eq!(wifi_icon(90), "network_wifi");
}
