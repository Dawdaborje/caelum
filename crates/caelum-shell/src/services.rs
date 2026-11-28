// SPDX-License-Identifier: GPL-3.0-or-later
//! The one background thread: a current-thread tokio runtime running every
//! service, forwarding each state change to the UI thread.
use std::{future::Future, sync::OnceLock};

use caelum_services::{
    Service, ServiceError, audio::AudioService, battery::BatteryService,
    bluetooth::BluetoothService, kwin::KwinService, network::NetworkService, tray::TrayService,
};
use caelum_shell::ipc;
use tokio::{
    runtime::Handle,
    sync::{mpsc, watch},
};

use crate::app;

static HANDLE: OnceLock<Handle> = OnceLock::new();

pub fn start() -> std::io::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let _ = HANDLE.set(rt.handle().clone());
    std::thread::Builder::new()
        .name("caelum-services".into())
        .spawn(move || {
            rt.block_on(async {
                spawn(KwinService, app::on_kwin);
                spawn(TrayService::default(), app::on_tray);
                spawn(AudioService, |s| app::on_status(|st| st.audio = s));
                spawn(NetworkService::default(), |s| {
                    app::on_status(|st| st.network = s)
                });
                spawn(BluetoothService::default(), |s| {
                    app::on_status(|st| st.bluetooth = s)
                });
                spawn(BatteryService::default(), |s| {
                    app::on_status(|st| st.battery = s)
                });
                commands();
                std::future::pending::<()>().await
            })
        })?;
    Ok(())
}

/// The shell's D-Bus interface and global shortcuts, both feeding commands to
/// the UI thread.
fn commands() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let ipc_tx = tx.clone();
    tokio::spawn(async move {
        let served = match zbus::Connection::session().await {
            Ok(conn) => ipc::serve(conn, ipc_tx).await,
            Err(e) => Err(e),
        };
        match served {
            // The connection answers calls for as long as it is alive.
            Ok(_conn) => std::future::pending::<()>().await,
            Err(e) => tracing::warn!("D-Bus interface {}: {e}", ipc::NAME),
        }
    });
    tokio::spawn(async move {
        if let Err(e) = crate::shortcuts::run(tx).await {
            tracing::warn!("global shortcuts unavailable: {e}");
        }
    });
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let Err(e) = slint::invoke_from_event_loop(move || app::on_command(cmd)) {
                tracing::warn!("forward command: {e}");
            }
        }
    });
}

/// Runs a one-off request (a click's D-Bus call) on the services thread.
pub fn run(task: impl Future<Output = ()> + Send + 'static) {
    if let Some(h) = HANDLE.get() {
        h.spawn(task);
    }
}

/// Runs `service` and calls `on_change` on the UI thread with every new state.
fn spawn<S: Service>(service: S, on_change: fn(Option<S::State>)) {
    let (tx, mut rx) = watch::channel(None);
    tokio::spawn(async move {
        match service.run(tx).await {
            Ok(()) => {}
            Err(ServiceError::Unavailable(e)) => tracing::warn!("service unavailable: {e}"),
            Err(e) => tracing::error!("service {e}"),
        }
    });
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let state = rx.borrow_and_update().clone();
            if let Err(e) = slint::invoke_from_event_loop(move || on_change(state)) {
                tracing::warn!("forward service state: {e}");
            }
        }
    });
}
