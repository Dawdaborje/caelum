// SPDX-License-Identifier: GPL-3.0-or-later
//! Background services. Each runs on the shell's one tokio runtime and publishes
//! its state through a `watch` channel; `None` means its module is hidden.
pub mod kwin;
pub mod tray;

pub trait Service: Send + 'static {
    type State: Clone + Send + Sync + 'static;
    /// Runs until the shell exits. Sends a fresh State on every change.
    /// Err(Unavailable) once at start if the backing service or hardware is absent.
    fn run(
        self,
        tx: tokio::sync::watch::Sender<Option<Self::State>>,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + Send;
}

#[derive(Debug)]
pub enum ServiceError {
    Unavailable(String),
    Failed(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(e) => write!(f, "unavailable: {e}"),
            Self::Failed(e) => write!(f, "failed: {e}"),
        }
    }
}

impl std::error::Error for ServiceError {}
