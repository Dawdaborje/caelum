// SPDX-License-Identifier: GPL-3.0-or-later
//! The shell's D-Bus interface on a private bus.
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};

use caelum_shell::ipc::{self, Action, Command as ShellCommand, Drawer, DrawerCommand};
use tokio::sync::mpsc;

struct Bus(Child, String);

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn private_bus() -> Bus {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon");
    let mut address = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    Bus(child, address.trim().to_owned())
}

async fn connect(bus: &Bus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.1.as_str())
        .unwrap()
        .build()
        .await
        .unwrap()
}

async fn call(conn: &zbus::Connection, method: &str, drawer: &str) -> zbus::Result<zbus::Message> {
    conn.call_method(
        Some(ipc::NAME),
        ipc::PATH,
        Some(ipc::INTERFACE),
        method,
        &(drawer,),
    )
    .await
}

#[tokio::test]
async fn toggle_known_drawer_sends_command() {
    let bus = private_bus();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _server = ipc::serve(connect(&bus).await, tx).await.unwrap();

    call(&connect(&bus).await, "Toggle", "launcher")
        .await
        .unwrap();
    let cmd = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap();
    assert_eq!(
        cmd,
        Some(ShellCommand::Drawer(DrawerCommand {
            drawer: Drawer::Launcher,
            action: Action::Toggle
        }))
    );
    assert!(rx.try_recv().is_err(), "exactly one command");
}

#[tokio::test]
async fn unknown_drawer_is_dbus_error() {
    let bus = private_bus();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _server = ipc::serve(connect(&bus).await, tx).await.unwrap();

    let err = call(&connect(&bus).await, "Open", "nope")
        .await
        .unwrap_err();
    let zbus::Error::MethodError(name, _, _) = err else {
        panic!("not a method error: {err:?}");
    };
    assert_eq!(name.as_str(), "org.caelum.Shell1.Error.UnknownDrawer");
    assert!(rx.try_recv().is_err());
}
