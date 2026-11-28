// SPDX-License-Identifier: GPL-3.0-or-later
//! The one background thread: a current-thread tokio runtime running every
//! service, forwarding each state change to the UI thread.
use std::{future::Future, sync::OnceLock};

use caelum_services::{Service, ServiceError, kwin::KwinService, tray::TrayService};
use tokio::{runtime::Handle, sync::watch};

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
                std::future::pending::<()>().await
            })
        })?;
    Ok(())
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
