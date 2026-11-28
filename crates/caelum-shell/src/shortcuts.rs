// SPDX-License-Identifier: GPL-3.0-or-later
//! Global shortcuts through KDE's kglobalaccel, as Caelestia's
//! globalshortcut.cpp does with KGlobalAccel::setShortcut(…, NoAutoloading):
//! register each action under the component "caelum", set its keys, then turn
//! the component's globalShortcutPressed signal into shell commands.
//!
//! shortcut: unlike Caelestia, keys another component already holds are not
//! stolen (and later restored); kglobalaccel leaves those unbound. Add the
//! steal/restore dance if collisions bite once Caelum replaces Caelestia.
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::OwnedObjectPath;

use caelum_shell::ipc::{Action, Command, Drawer, DrawerCommand};

const COMPONENT: &str = "caelum";
const FRIENDLY: &str = "Caelum";
/// kglobalaccel setter flags: IsDefault = 1, SetPresent = 2, NoAutoloading = 4.
const IS_DEFAULT: u32 = 1;
const ACTIVE: u32 = 2 | 4;

/// Caelestia's defaults from Config/keybindsdefaults.hpp, for the drawers
/// Caelum has; "; " separates alternative sequences.
const SHORTCUTS: &[(&str, &str, Drawer, &str)] = &[
    (
        "launcher",
        "Toggle launcher",
        Drawer::Launcher,
        "Meta+Space; Meta",
    ),
    (
        "session",
        "Toggle session menu",
        Drawer::Session,
        "Ctrl+Alt+Delete",
    ),
    ("sidebar", "Toggle sidebar", Drawer::Sidebar, "Meta+B"),
    ("dashboard", "Toggle dashboard", Drawer::Dashboard, ""),
];

// Qt::KeyboardModifier and Qt::Key values, as kglobalaccel stores keys.
const SHIFT: i32 = 0x0200_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const META: i32 = 0x1000_0000;

fn key_code(name: &str) -> Option<i32> {
    let special = match name {
        "Space" => 0x20,
        "Tab" => 0x0100_0001,
        "Backspace" => 0x0100_0003,
        "Return" => 0x0100_0004,
        "Enter" => 0x0100_0005,
        "Delete" | "Del" => 0x0100_0007,
        "Left" => 0x0100_0012,
        "Up" => 0x0100_0013,
        "Right" => 0x0100_0014,
        "Down" => 0x0100_0015,
        "Meta" => 0x0100_0022,
        _ => {
            let mut chars = name.chars();
            return match (chars.next(), chars.next()) {
                // Qt keys for printable characters are their uppercase code point.
                (Some(c), None) if c.is_ascii_graphic() => Some(c.to_ascii_uppercase() as i32),
                _ => None,
            };
        }
    };
    Some(special)
}

/// One key combination ("Ctrl+Alt+Delete") as a Qt combined key code. A lone
/// modifier ("Meta") is that modifier's key with no modifier bits, as
/// QKeySequence gives it.
fn chord(text: &str) -> Option<i32> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let (key, mods) = parts.split_last()?;
    let mut code = 0;
    for m in mods {
        code |= match *m {
            "Shift" => SHIFT,
            "Ctrl" | "Control" => CTRL,
            "Alt" => ALT,
            "Meta" | "Super" => META,
            _ => return None,
        };
    }
    Some(code | key_code(key)?)
}

/// "Meta+Space; Meta" as kglobalaccel's a(ai): one entry per alternative, each
/// a four-chord key sequence padded with 0.
pub fn parse_keys(keys: &str) -> Vec<(Vec<i32>,)> {
    keys.split(';')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .filter_map(|k| match chord(k) {
            Some(c) => Some((vec![c, 0, 0, 0],)),
            None => {
                tracing::warn!("shortcut: cannot parse key {k:?}");
                None
            }
        })
        .collect()
}

#[zbus::proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    #[zbus(name = "doRegister")]
    fn do_register(&self, action_id: &[&str]) -> zbus::Result<()>;
    #[zbus(name = "setShortcutKeys")]
    fn set_shortcut_keys(
        &self,
        action_id: &[&str],
        keys: &[(Vec<i32>,)],
        flags: u32,
    ) -> zbus::Result<Vec<(Vec<i32>,)>>;
    #[zbus(name = "getComponent")]
    fn get_component(&self, unique: &str) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.kde.kglobalaccel.Component",
    default_service = "org.kde.kglobalaccel"
)]
trait Component {
    #[zbus(signal, name = "globalShortcutPressed")]
    fn global_shortcut_pressed(
        &self,
        component: String,
        action: String,
        timestamp: i64,
    ) -> zbus::Result<()>;
}

/// Registers the shortcuts and sends a command for each press. Returns only on
/// error; without kglobalaccel (nested KWin) that is right away.
pub async fn run(tx: UnboundedSender<Command>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let accel = KGlobalAccelProxy::new(&conn).await?;
    for &(action, text, _, keys) in SHORTCUTS {
        let id = [COMPONENT, action, FRIENDLY, text];
        let keys = parse_keys(keys);
        accel.do_register(&id).await?;
        accel.set_shortcut_keys(&id, &keys, IS_DEFAULT).await?;
        let got = accel.set_shortcut_keys(&id, &keys, ACTIVE).await?;
        if got.len() < keys.len() {
            tracing::warn!("shortcut {action}: some keys are taken by another component");
        }
    }
    let path = accel.get_component(COMPONENT).await?;
    let component = ComponentProxy::builder(&conn).path(path)?.build().await?;
    let mut presses = component.receive_global_shortcut_pressed().await?;
    while let Some(press) = presses.next().await {
        let args = press.args()?;
        if args.component != COMPONENT {
            continue;
        }
        if let Some(&(_, _, drawer, _)) = SHORTCUTS.iter().find(|s| s.0 == args.action) {
            let _ = tx.send(Command::Drawer(DrawerCommand {
                drawer,
                action: Action::Toggle,
            }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_qt() {
        // Values from kglobalaccel for plasmashell's "Meta; Alt+F1"-style keys.
        assert_eq!(parse_keys("Meta"), [(vec![0x0100_0022, 0, 0, 0],)]);
        assert_eq!(parse_keys("Meta+Space; Meta").len(), 2);
        assert_eq!(parse_keys("Meta+Space")[0].0[0], META | 0x20);
        assert_eq!(
            parse_keys("Ctrl+Alt+Delete")[0].0[0],
            CTRL | ALT | 0x0100_0007
        );
        assert_eq!(parse_keys("Meta+B")[0].0[0], META | 'B' as i32);
        assert!(parse_keys("").is_empty());
        assert!(parse_keys("Hyper+Q").is_empty());
    }
}
