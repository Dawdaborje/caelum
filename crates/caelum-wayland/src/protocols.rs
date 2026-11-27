// SPDX-License-Identifier: GPL-3.0-or-later
//! Client bindings for KDE protocols, from XML copied from Caelestia's
//! `shell/plugin/src/Caelestia/protocols/`.
//! shortcut: only the protocols Phase 1 uses are generated; add the other XMLs
//! here when a task needs them (output-management needs output-device in scope).
#![allow(
    clippy::all,
    unused_imports,
    non_upper_case_globals,
    non_camel_case_types
)]

macro_rules! protocol {
    ($name:ident, $file:literal) => {
        pub mod $name {
            use wayland_client;
            use wayland_client::backend as wayland_backend;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::backend as wayland_backend;
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!($file);
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!($file);
        }
    };
}

protocol!(
    plasma_window_management,
    "protocols/plasma-window-management.xml"
);
protocol!(
    plasma_virtual_desktop,
    "protocols/org-kde-plasma-virtual-desktop.xml"
);
