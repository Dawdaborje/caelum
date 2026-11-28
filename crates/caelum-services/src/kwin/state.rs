// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct KwinState {
    pub workspaces: Vec<Workspace>,
    pub active: Option<String>,
    pub active_by_output: HashMap<String, String>,
    pub active_window: Option<WindowInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub position: u32,
    pub occupied: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub output: Option<String>,
}

pub enum KwinEvent {
    DesktopCreated { id: String, position: u32 },
    DesktopName { id: String, name: String },
    DesktopPosition { id: String, position: u32 },
    DesktopActivated { id: String },
    DesktopOutputEntered { id: String, output: String },
    DesktopRemoved { id: String },
    WindowAdded { id: String },
    WindowTitle { id: String, title: String },
    WindowAppId { id: String, app_id: String },
    WindowActive { id: String, active: bool },
    WindowDesktopEntered { id: String, desktop: String },
    WindowDesktopLeft { id: String, desktop: String },
    WindowRemoved { id: String },
}

#[derive(Default)]
struct Window {
    title: String,
    app_id: String,
    desktops: Vec<String>,
}

/// Folds KWin protocol events into a [`KwinState`]. Pure, so it is tested
/// without a compositor.
#[derive(Default)]
pub struct KwinTracker {
    desktops: HashMap<String, (String, u32)>,
    active: Option<String>,
    active_by_output: HashMap<String, String>,
    windows: HashMap<String, Window>,
    active_window: Option<String>,
}

impl KwinTracker {
    pub fn apply(&mut self, ev: KwinEvent) -> KwinState {
        use KwinEvent::*;
        match ev {
            DesktopCreated { id, position } => {
                self.desktops.entry(id).or_default().1 = position;
            }
            DesktopName { id, name } => self.desktops.entry(id).or_default().0 = name,
            DesktopPosition { id, position } => self.desktops.entry(id).or_default().1 = position,
            DesktopActivated { id } => self.active = Some(id),
            DesktopOutputEntered { id, output } => {
                self.active_by_output.insert(output, id);
            }
            DesktopRemoved { id } => {
                self.desktops.remove(&id);
                self.active_by_output.retain(|_, d| *d != id);
                if self.active.as_ref() == Some(&id) {
                    self.active = None;
                }
            }
            WindowAdded { id } => {
                self.windows.entry(id).or_default();
            }
            WindowTitle { id, title } => self.windows.entry(id).or_default().title = title,
            WindowAppId { id, app_id } => self.windows.entry(id).or_default().app_id = app_id,
            WindowActive { id, active } => {
                if active {
                    self.active_window = Some(id);
                } else if self.active_window.as_ref() == Some(&id) {
                    self.active_window = None;
                }
            }
            WindowDesktopEntered { id, desktop } => {
                let w = self.windows.entry(id).or_default();
                if !w.desktops.contains(&desktop) {
                    w.desktops.push(desktop);
                }
            }
            WindowDesktopLeft { id, desktop } => {
                self.windows
                    .entry(id)
                    .or_default()
                    .desktops
                    .retain(|d| *d != desktop);
            }
            WindowRemoved { id } => {
                self.windows.remove(&id);
                if self.active_window.as_ref() == Some(&id) {
                    self.active_window = None;
                }
            }
        }
        self.state()
    }

    fn state(&self) -> KwinState {
        let mut workspaces: Vec<_> = self
            .desktops
            .iter()
            .map(|(id, (name, position))| Workspace {
                id: id.clone(),
                name: name.clone(),
                position: *position,
                occupied: self.windows.values().any(|w| w.desktops.contains(id)),
            })
            .collect();
        workspaces.sort_by_key(|w| w.position);
        KwinState {
            workspaces,
            active: self.active.clone(),
            active_by_output: self.active_by_output.clone(),
            active_window: self.active_window.as_ref().and_then(|id| {
                let w = self.windows.get(id)?;
                Some(WindowInfo {
                    id: id.clone(),
                    title: w.title.clone(),
                    app_id: w.app_id.clone(),
                    // shortcut: the window protocol has no output event; derive it
                    // from window geometry if per-output occupancy is ever needed.
                    output: None,
                })
            }),
        }
    }
}
