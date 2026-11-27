// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use caelum_config::{BorderConfig, Config, load, watch};

fn load_str(s: &str) -> Result<Config, caelum_config::ConfigError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, s).unwrap();
    load(&path)
}

#[test]
fn missing_file_is_default() {
    assert_eq!(load(Path::new("/nonexistent")).unwrap(), Config::default());
}

#[test]
fn defaults_match_caelestia() {
    assert_eq!(
        Config::default().border,
        BorderConfig {
            thickness: 10,
            rounding: 25,
            smoothing: 20
        }
    );
}

#[test]
fn partial_file_fills_defaults() {
    let c = load_str("[border]\nthickness = 4").unwrap();
    assert_eq!(c.border.thickness, 4);
    assert_eq!(c.border.rounding, 25);
}

#[test]
fn example_file_is_default() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.example.toml");
    assert_eq!(load(&path).unwrap(), Config::default());
}

#[test]
fn unknown_edge_is_error() {
    assert!(load_str("[bar]\nposition = \"middle\"").is_err());
}

#[test]
fn invalid_reload_keeps_previous() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "").unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let _handle = watch(path.clone(), move |c| sink.lock().unwrap().push(c)).unwrap();

    std::fs::write(&path, "[border]\nthickness = 3").unwrap();
    sleep(Duration::from_millis(500));
    std::fs::write(&path, "[[[").unwrap();
    sleep(Duration::from_millis(500));

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].border.thickness, 3);
}
