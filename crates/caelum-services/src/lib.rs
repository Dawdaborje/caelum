// SPDX-License-Identifier: GPL-3.0-or-later
//! Background services. Each runs on the shell's one tokio runtime and publishes
//! its state through a `watch` channel; `None` means its module is hidden.
pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod kwin;
pub mod network;
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

/// The system bus, or the bus at `address` (tests use a private dbus-daemon).
pub(crate) async fn system_bus(address: Option<&str>) -> zbus::Result<zbus::Connection> {
    match address {
        Some(a) => zbus::connection::Builder::address(a)?.build().await,
        None => zbus::Connection::system().await,
    }
}

/// Reads one D-Bus property.
pub(crate) async fn prop<T>(
    conn: &zbus::Connection,
    dest: &str,
    path: &str,
    iface: &str,
    name: &str,
) -> zbus::Result<T>
where
    T: TryFrom<zbus::zvariant::OwnedValue>,
    T::Error: Into<zbus::Error>,
{
    let reply = conn
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(iface, name),
        )
        .await?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize()?;
    T::try_from(value).map_err(Into::into)
}

/// Sends `state` if it differs from what the channel holds.
pub(crate) fn publish<T: PartialEq>(tx: &tokio::sync::watch::Sender<Option<T>>, state: T) {
    tx.send_if_modified(|s| {
        let changed = s.as_ref() != Some(&state);
        if changed {
            *s = Some(state);
        }
        changed
    });
}

/// A stream of every signal `sender` emits, so a service can re-read its state
/// on any change.
pub(crate) async fn signals_from(
    conn: &zbus::Connection,
    sender: &str,
) -> zbus::Result<zbus::MessageStream> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(sender)?
        .build();
    zbus::MessageStream::for_match_rule(rule, conn, None).await
}

/// Waits for a signal, then drains any already queued, so a burst (NetworkManager
/// sends one per access point on every scan) costs one re-read. False when the
/// stream has ended.
pub(crate) async fn next_change(stream: &mut zbus::MessageStream) -> bool {
    use futures_util::{FutureExt, StreamExt};
    if stream.next().await.is_none() {
        return false;
    }
    while let Some(Some(_)) = stream.next().now_or_never() {}
    true
}
