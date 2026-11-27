// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;

/// An output (screen). `width`/`height` are logical pixels; physical = logical × `scale`.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    /// The `wl_output` global name; stable while the output exists.
    pub id: u32,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    Added(OutputInfo),
    Removed(u32),
    Changed(OutputInfo),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceAction {
    Create(u32),
    Destroy(u32),
    Resize(u32),
}

/// Turns output events into per-output surface actions. Pure, so it is testable
/// without a compositor; duplicate or no-op events produce no actions.
#[derive(Default, Debug)]
pub struct OutputTracker {
    outputs: HashMap<u32, OutputInfo>,
}

impl OutputTracker {
    pub fn apply(&mut self, ev: OutputEvent) -> Vec<SurfaceAction> {
        match ev {
            OutputEvent::Added(info) | OutputEvent::Changed(info) => {
                let id = info.id;
                match self.outputs.insert(id, info.clone()) {
                    None => vec![SurfaceAction::Create(id)],
                    Some(old) if old != info => vec![SurfaceAction::Resize(id)],
                    Some(_) => vec![],
                }
            }
            OutputEvent::Removed(id) => match self.outputs.remove(&id) {
                Some(_) => vec![SurfaceAction::Destroy(id)],
                None => vec![],
            },
        }
    }

    pub fn info(&self, id: u32) -> Option<&OutputInfo> {
        self.outputs.get(&id)
    }
}
