// SPDX-License-Identifier: GPL-3.0-or-later
//! Workspaces and the active window, from KDE's virtual-desktop and
//! window-management Wayland protocols, on a connection of its own so it can
//! run on the tokio thread.
mod state;

use std::sync::{Mutex, OnceLock};

use caelum_wayland::protocols::{
    plasma_virtual_desktop::{
        org_kde_plasma_virtual_desktop::{self as vd, OrgKdePlasmaVirtualDesktop},
        org_kde_plasma_virtual_desktop_management::{
            self as vdm, OrgKdePlasmaVirtualDesktopManagement,
        },
    },
    plasma_window_management::{
        org_kde_plasma_window::{self as pw, OrgKdePlasmaWindow},
        org_kde_plasma_window_management::{self as pwm, OrgKdePlasmaWindowManagement},
    },
};
pub use state::*;
use tokio::{
    io::{Interest, unix::AsyncFd},
    sync::watch,
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};

use crate::{Service, ServiceError};

/// `org_kde_plasma_window` state flag for the focused window.
const STATE_ACTIVE: u32 = 0x1;

static CONN: OnceLock<Connection> = OnceLock::new();
static DESKTOPS: Mutex<Vec<(String, OrgKdePlasmaVirtualDesktop)>> = Mutex::new(Vec::new());

/// Switches to a desktop by its protocol id. Does nothing before the service has
/// seen that desktop.
pub fn activate_workspace(id: &str) {
    let desktops = DESKTOPS.lock().unwrap();
    if let Some((_, d)) = desktops.iter().find(|(d, _)| d == id) {
        d.request_activate();
        if let Some(Err(e)) = CONN.get().map(Connection::flush) {
            tracing::warn!("activate workspace: {e}");
        }
    }
}

pub struct KwinService;

struct Pump {
    tracker: KwinTracker,
    state: KwinState,
}

impl Pump {
    fn apply(&mut self, ev: KwinEvent) {
        self.state = self.tracker.apply(ev);
    }
}

fn failed(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(e.to_string())
}

impl Service for KwinService {
    type State = KwinState;

    async fn run(self, tx: watch::Sender<Option<KwinState>>) -> Result<(), ServiceError> {
        let unavailable = |e: &dyn std::fmt::Display| ServiceError::Unavailable(e.to_string());
        let conn = Connection::connect_to_env().map_err(|e| unavailable(&e))?;
        let (globals, mut queue) =
            registry_queue_init::<Pump>(&conn).map_err(|e| unavailable(&e))?;
        let qh = queue.handle();
        // v4: output_entered; v13: window_with_uuid (v8 brings desktop enter/leave).
        let _: OrgKdePlasmaVirtualDesktopManagement =
            globals.bind(&qh, 4..=4, ()).map_err(|e| unavailable(&e))?;
        let _: OrgKdePlasmaWindowManagement = globals
            .bind(&qh, 13..=16, ())
            .map_err(|e| unavailable(&e))?;
        let _ = CONN.set(conn.clone());

        let fd = conn
            .backend()
            .poll_fd()
            .try_clone_to_owned()
            .map_err(failed)?;
        // SAFETY: the AsyncFd owns this dup of the socket, so it stays open while registered.
        let fd = unsafe { AsyncFd::register_with_interest(fd, Interest::READABLE) }
            .map_err(|e| failed(e.into_parts().1))?;
        let mut pump = Pump {
            tracker: KwinTracker::default(),
            state: KwinState::default(),
        };
        loop {
            queue.dispatch_pending(&mut pump).map_err(failed)?;
            tx.send_if_modified(|s| {
                let changed = s.as_ref() != Some(&pump.state);
                if changed {
                    *s = Some(pump.state.clone());
                }
                changed
            });
            queue.flush().map_err(failed)?;
            let Some(guard) = queue.prepare_read() else {
                continue;
            };
            let mut ready = fd.readable().await.map_err(failed)?;
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    ready.clear_ready()
                }
                Err(e) => return Err(failed(e)),
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Pump {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<OrgKdePlasmaVirtualDesktopManagement, ()> for Pump {
    fn event(
        pump: &mut Self,
        mgr: &OrgKdePlasmaVirtualDesktopManagement,
        ev: vdm::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match ev {
            vdm::Event::DesktopCreated {
                desktop_id: id,
                position,
            } => {
                let desktop = mgr.get_virtual_desktop(id.clone(), qh, id.clone());
                DESKTOPS.lock().unwrap().push((id.clone(), desktop));
                pump.apply(KwinEvent::DesktopCreated { id, position });
            }
            vdm::Event::DesktopRemoved { desktop_id: id } => {
                DESKTOPS.lock().unwrap().retain(|(d, _)| *d != id);
                pump.apply(KwinEvent::DesktopRemoved { id });
            }
            _ => {}
        }
    }
}

impl Dispatch<OrgKdePlasmaVirtualDesktop, String> for Pump {
    fn event(
        pump: &mut Self,
        _: &OrgKdePlasmaVirtualDesktop,
        ev: vd::Event,
        id: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = id.clone();
        pump.apply(match ev {
            vd::Event::Name { name } => KwinEvent::DesktopName { id, name },
            vd::Event::Position { index } => KwinEvent::DesktopPosition {
                id,
                position: index,
            },
            vd::Event::Activated => KwinEvent::DesktopActivated { id },
            vd::Event::OutputEntered { output_name } => KwinEvent::DesktopOutputEntered {
                id,
                output: output_name,
            },
            _ => return,
        });
    }
}

impl Dispatch<OrgKdePlasmaWindowManagement, ()> for Pump {
    fn event(
        pump: &mut Self,
        mgr: &OrgKdePlasmaWindowManagement,
        ev: pwm::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let pwm::Event::WindowWithUuid { uuid, .. } = ev {
            mgr.get_window_by_uuid(uuid.clone(), qh, uuid.clone());
            pump.apply(KwinEvent::WindowAdded { id: uuid });
        }
    }
}

impl Dispatch<OrgKdePlasmaWindow, String> for Pump {
    fn event(
        pump: &mut Self,
        window: &OrgKdePlasmaWindow,
        ev: pw::Event,
        id: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = id.clone();
        pump.apply(match ev {
            pw::Event::TitleChanged { title } => KwinEvent::WindowTitle { id, title },
            pw::Event::AppIdChanged { app_id } => KwinEvent::WindowAppId { id, app_id },
            pw::Event::StateChanged { flags } => KwinEvent::WindowActive {
                id,
                active: flags & STATE_ACTIVE != 0,
            },
            pw::Event::VirtualDesktopEntered { id: desktop } => {
                KwinEvent::WindowDesktopEntered { id, desktop }
            }
            pw::Event::VirtualDesktopLeft { is: desktop } => {
                KwinEvent::WindowDesktopLeft { id, desktop }
            }
            pw::Event::Unmapped => {
                if window.version() >= 4 {
                    window.destroy();
                }
                KwinEvent::WindowRemoved { id }
            }
            _ => return,
        });
    }
}
