// SPDX-License-Identifier: GPL-3.0-or-later
use caelum_wayland::{OutputEvent, OutputInfo, OutputTracker, SurfaceAction};

fn out(id: u32, width: i32) -> OutputInfo {
    OutputInfo {
        id,
        name: format!("OUT-{id}"),
        width,
        height: 1080,
        scale: 1.0,
    }
}

#[test]
fn added_creates_once() {
    let mut t = OutputTracker::default();
    assert_eq!(
        t.apply(OutputEvent::Added(out(1, 1920))),
        [SurfaceAction::Create(1)]
    );
    assert_eq!(t.apply(OutputEvent::Added(out(1, 1920))), []);
}

#[test]
fn outputs_removed_drops_surfaces() {
    let mut t = OutputTracker::default();
    t.apply(OutputEvent::Added(out(1, 1920)));
    t.apply(OutputEvent::Added(out(2, 1920)));
    assert_eq!(
        t.apply(OutputEvent::Removed(1)),
        [SurfaceAction::Destroy(1)]
    );
    assert!(t.info(1).is_none());
    assert!(t.info(2).is_some());
}

#[test]
fn changed_resizes() {
    let mut t = OutputTracker::default();
    t.apply(OutputEvent::Added(out(1, 1920)));
    assert_eq!(
        t.apply(OutputEvent::Changed(out(1, 2560))),
        [SurfaceAction::Resize(1)]
    );
    assert_eq!(t.info(1).map(|i| i.width), Some(2560));
}
