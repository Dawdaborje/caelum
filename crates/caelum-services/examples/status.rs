// SPDX-License-Identifier: GPL-3.0-or-later
//! Prints live status states: `cargo run -p caelum-services --example status`.
use std::fmt::Debug;

use caelum_services::{
    Service, audio::AudioService, battery::BatteryService, bluetooth::BluetoothService,
    network::NetworkService,
};
use tokio::sync::watch;

fn watch_and_print<S: Service>(name: &'static str, service: S)
where
    S::State: Debug,
{
    let (tx, mut rx) = watch::channel(None);
    tokio::spawn(async move {
        if let Err(e) = service.run(tx).await {
            println!("{name}: {e}");
        }
    });
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            println!("{name}: {:?}", rx.borrow_and_update());
        }
    });
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    watch_and_print("audio", AudioService);
    watch_and_print("network", NetworkService::default());
    watch_and_print("bluetooth", BluetoothService::default());
    watch_and_print("battery", BatteryService::default());
    std::future::pending::<()>().await;
}
