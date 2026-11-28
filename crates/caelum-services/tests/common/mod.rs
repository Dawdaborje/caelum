// SPDX-License-Identifier: GPL-3.0-or-later
//! A private dbus-daemon per test, standing in for the session or system bus.
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

pub struct Bus(Child, pub String);

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

pub fn private_bus() -> Bus {
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
