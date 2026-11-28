// SPDX-License-Identifier: GPL-3.0-or-later
use caelum_services::kwin::{KwinEvent, KwinTracker};

fn desktop(t: &mut KwinTracker, id: &str, position: u32) {
    t.apply(KwinEvent::DesktopCreated {
        id: id.into(),
        position,
    });
    t.apply(KwinEvent::DesktopName {
        id: id.into(),
        name: id.to_uppercase(),
    });
}

#[test]
fn desktops_sorted_by_position() {
    let mut t = KwinTracker::default();
    desktop(&mut t, "c", 2);
    desktop(&mut t, "a", 0);
    desktop(&mut t, "b", 1);
    let s = t.apply(KwinEvent::DesktopActivated { id: "a".into() });
    let ids: Vec<_> = s.workspaces.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(s.workspaces[1].name, "B");
    assert_eq!(s.active.as_deref(), Some("a"));
}

#[test]
fn occupied_follows_windows() {
    let mut t = KwinTracker::default();
    desktop(&mut t, "a", 0);
    desktop(&mut t, "b", 1);
    t.apply(KwinEvent::WindowAdded { id: "w".into() });
    let s = t.apply(KwinEvent::WindowDesktopEntered {
        id: "w".into(),
        desktop: "b".into(),
    });
    assert!(!s.workspaces[0].occupied);
    assert!(s.workspaces[1].occupied);
    let s = t.apply(KwinEvent::WindowRemoved { id: "w".into() });
    assert!(!s.workspaces[1].occupied);
}

#[test]
fn active_window_tracks_focus() {
    let mut t = KwinTracker::default();
    for (id, title) in [("w1", "first"), ("w2", "second")] {
        t.apply(KwinEvent::WindowAdded { id: id.into() });
        t.apply(KwinEvent::WindowTitle {
            id: id.into(),
            title: title.into(),
        });
    }
    t.apply(KwinEvent::WindowActive {
        id: "w1".into(),
        active: true,
    });
    t.apply(KwinEvent::WindowActive {
        id: "w1".into(),
        active: false,
    });
    let s = t.apply(KwinEvent::WindowActive {
        id: "w2".into(),
        active: true,
    });
    assert_eq!(s.active_window.unwrap().title, "second");
    let s = t.apply(KwinEvent::WindowRemoved { id: "w2".into() });
    assert!(s.active_window.is_none());
}

#[test]
fn active_by_output_follows_output_entered() {
    let mut t = KwinTracker::default();
    desktop(&mut t, "a", 0);
    desktop(&mut t, "b", 1);
    t.apply(KwinEvent::DesktopOutputEntered {
        id: "a".into(),
        output: "DP-1".into(),
    });
    let s = t.apply(KwinEvent::DesktopOutputEntered {
        id: "b".into(),
        output: "DP-1".into(),
    });
    assert_eq!(s.active_by_output["DP-1"], "b");
}
