// SPDX-License-Identifier: GPL-3.0-or-later
use std::{path::PathBuf, time::Duration};

use notify_debouncer_mini::{
    DebounceEventResult, DebouncedEventBatch, Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};

use crate::{Config, ConfigError, load};

/// Keeps the watcher alive; dropping it stops watching.
pub struct WatchHandle {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Calls `on_change` with the new config whenever the file changes and parses.
/// Watches the parent directory so editors that replace the file are seen too.
pub fn watch(
    path: PathBuf,
    on_change: impl Fn(Config) + Send + 'static,
) -> Result<WatchHandle, ConfigError> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let name = path.file_name().map(ToOwned::to_owned);
    let target = path.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(200),
        move |res: DebounceEventResult| {
            let changed = match res {
                Ok(DebouncedEventBatch::Events(events)) => {
                    events.iter().any(|e| e.path.file_name() == name.as_deref())
                }
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!("config watch error: {e}");
                    false
                }
            };
            if changed {
                match load(&target) {
                    Ok(c) => on_change(c),
                    Err(e) => tracing::warn!("config reload failed, keeping previous: {e}"),
                }
            }
        },
    )?;
    debouncer
        .watcher()
        .watch(&dir, RecursiveMode::NonRecursive)?;
    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}
