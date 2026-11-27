// SPDX-License-Identifier: GPL-3.0-or-later
//! Loads `~/.config/caelum/config.toml` and reloads it live.
use std::{
    fmt, io,
    path::{Path, PathBuf},
};

mod model;
mod watch;

pub use model::*;
pub use watch::{WatchHandle, watch};

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
    Watch(notify_debouncer_mini::notify::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "reading config: {e}"),
            Self::Parse(e) => write!(f, "parsing config: {e}"),
            Self::Watch(e) => write!(f, "watching config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

impl From<notify_debouncer_mini::notify::Error> for ConfigError {
    fn from(e: notify_debouncer_mini::notify::Error) -> Self {
        Self::Watch(e)
    }
}

/// `$XDG_CONFIG_HOME/caelum/config.toml`, falling back to `~/.config`.
pub fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| Path::new(v).is_absolute())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("caelum/config.toml")
}

/// Parses the config file. A missing file is the default config.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            let mut c: Config = toml::from_str(&s)?;
            let w = &mut c.wallpaper;
            for p in std::iter::once(&mut w.path).chain(&mut w.rotation) {
                *p = expand_home(p);
            }
            Ok(c)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e.into()),
    }
}

fn expand_home(p: &Path) -> PathBuf {
    match p.strip_prefix("~") {
        Ok(rest) => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest),
        Err(_) => p.to_path_buf(),
    }
}
