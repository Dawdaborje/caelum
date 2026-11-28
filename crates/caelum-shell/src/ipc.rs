// SPDX-License-Identifier: GPL-3.0-or-later
//! The shell's D-Bus interface: `org.caelum.Shell1` at `/org/caelum/Shell`,
//! called by the `caelum` CLI and by global shortcuts. Commands go to the UI
//! thread through a channel.
use std::str::FromStr;

use tokio::sync::mpsc::UnboundedSender;

pub const NAME: &str = "org.caelum.Shell";
pub const PATH: &str = "/org/caelum/Shell";
pub const INTERFACE: &str = "org.caelum.Shell1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    Launcher,
    Session,
    Dashboard,
    Sidebar,
}

impl FromStr for Drawer {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "launcher" => Ok(Self::Launcher),
            "session" => Ok(Self::Session),
            "dashboard" => Ok(Self::Dashboard),
            "sidebar" => Ok(Self::Sidebar),
            _ => Err(format!("unknown drawer {s:?}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Toggle,
    Open,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawerCommand {
    pub drawer: Drawer,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Drawer(DrawerCommand),
    Reload,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.caelum.Shell1.Error")]
pub enum ShellError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UnknownDrawer(String),
    /// The shell is shutting down.
    Gone(String),
}

struct Shell(UnboundedSender<Command>);

impl Shell {
    fn send(&self, cmd: Command) -> Result<(), ShellError> {
        self.0
            .send(cmd)
            .map_err(|e| ShellError::Gone(e.to_string()))
    }

    fn drawer(&self, name: &str, action: Action) -> Result<(), ShellError> {
        let drawer = name.parse().map_err(ShellError::UnknownDrawer)?;
        self.send(Command::Drawer(DrawerCommand { drawer, action }))
    }
}

#[zbus::interface(name = "org.caelum.Shell1")]
impl Shell {
    fn toggle(&self, drawer: &str) -> Result<(), ShellError> {
        self.drawer(drawer, Action::Toggle)
    }

    fn open(&self, drawer: &str) -> Result<(), ShellError> {
        self.drawer(drawer, Action::Open)
    }

    fn close(&self, drawer: &str) -> Result<(), ShellError> {
        self.drawer(drawer, Action::Close)
    }

    fn reload(&self) -> Result<(), ShellError> {
        self.send(Command::Reload)
    }
}

/// Serves the interface on `conn` and takes the well-known name. Keep the
/// returned connection alive for as long as the interface should answer.
pub async fn serve(
    conn: zbus::Connection,
    tx: UnboundedSender<Command>,
) -> zbus::Result<zbus::Connection> {
    conn.object_server().at(PATH, Shell(tx)).await?;
    conn.request_name(NAME).await?;
    Ok(conn)
}
