// SPDX-License-Identifier: GPL-3.0-or-later
//! Volume of the default sink and source, from PipeWire's Pulse server.
//!
//! libpulse's context is single-threaded (Rc inside), so it lives on its own
//! thread with a standard mainloop that sleeps until Pulse sends an event.
//! Changes go out through `pactl`: they are rare (a click or scroll) and do not
//! need the context.
use std::{cell::Cell, cell::RefCell, rc::Rc};

use libpulse_binding::{
    callbacks::ListResult,
    context::{Context, FlagSet, State, introspect::Introspector, subscribe::InterestMaskSet},
    mainloop::standard::{IterateResult, Mainloop},
    volume::{ChannelVolumes, Volume},
};
use tokio::sync::{oneshot, watch};

use crate::{Service, ServiceError, publish};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct AudioState {
    /// 0-1 (can exceed 1 when boosted), as pavucontrol's percentage.
    pub volume: f32,
    pub muted: bool,
    pub mic_volume: f32,
    pub mic_muted: bool,
    pub default_sink: String,
}

/// Caelestia's `Icons.getVolumeIcon`.
pub fn volume_icon(volume: f32, muted: bool) -> &'static str {
    match () {
        _ if muted => "no_sound",
        _ if volume >= 0.5 => "volume_up",
        _ if volume > 0.0 => "volume_down",
        _ => "volume_mute",
    }
}

async fn pactl(args: &[&str]) -> std::io::Result<()> {
    let status = tokio::process::Command::new("pactl")
        .args(args)
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("pactl {args:?}: {status}")))
    }
}

/// Sets the default sink's volume, clamped to 0-1 like Caelestia's maxVolume.
pub async fn set_volume(volume: f32) -> std::io::Result<()> {
    let percent = format!("{}%", (volume.clamp(0.0, 1.0) * 100.0).round());
    pactl(&["set-sink-volume", "@DEFAULT_SINK@", &percent]).await
}

pub async fn toggle_mute() -> std::io::Result<()> {
    pactl(&["set-sink-mute", "@DEFAULT_SINK@", "toggle"]).await
}

#[derive(Default)]
pub struct AudioService;

impl Service for AudioService {
    type State = AudioState;

    async fn run(self, tx: watch::Sender<Option<AudioState>>) -> Result<(), ServiceError> {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        std::thread::Builder::new()
            .name("caelum-pulse".into())
            .spawn(move || {
                pulse_loop(tx, ready_tx);
                let _ = done_tx.send(());
            })
            .map_err(|e| ServiceError::Failed(e.to_string()))?;
        match ready_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(ServiceError::Unavailable(e)),
            Err(_) => return Err(ServiceError::Failed("pulse thread died".into())),
        }
        let _ = done_rx.await;
        // shortcut: no reconnect after the Pulse server restarts.
        Err(ServiceError::Failed("lost the Pulse server".into()))
    }
}

fn level(v: &ChannelVolumes) -> f32 {
    v.avg().0 as f32 / Volume::NORMAL.0 as f32
}

fn pulse_loop(tx: watch::Sender<Option<AudioState>>, ready: oneshot::Sender<Result<(), String>>) {
    let Some(mut ml) = Mainloop::new() else {
        let _ = ready.send(Err("no mainloop".into()));
        return;
    };
    let Some(mut ctx) = Context::new(&ml, "caelum") else {
        let _ = ready.send(Err("no context".into()));
        return;
    };
    if let Err(e) = ctx.connect(None, FlagSet::NOFLAGS, None) {
        let _ = ready.send(Err(e
            .to_string()
            .unwrap_or_else(|| "cannot connect".into())));
        return;
    }
    loop {
        if let IterateResult::Quit(_) | IterateResult::Err(_) = ml.iterate(true) {
            let _ = ready.send(Err("mainloop stopped".into()));
            return;
        }
        match ctx.get_state() {
            State::Ready => break,
            State::Failed | State::Terminated => {
                let _ = ready.send(Err("cannot connect to the Pulse server".into()));
                return;
            }
            _ => {}
        }
    }
    let _ = ready.send(Ok(()));

    let dirty = Rc::new(Cell::new(true));
    let d = dirty.clone();
    ctx.set_subscribe_callback(Some(Box::new(move |_, _, _| d.set(true))));
    ctx.subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        |_| {},
    );
    let intro = Rc::new(ctx.introspect());
    let state = Rc::new(RefCell::new(AudioState::default()));
    let tx = Rc::new(tx);
    loop {
        if dirty.replace(false) {
            query(&intro, &state, &tx);
        }
        if !matches!(ml.iterate(true), IterateResult::Success(_))
            || !matches!(ctx.get_state(), State::Ready)
        {
            return;
        }
    }
}

/// Server info gives the default sink and source names; their info gives the
/// volumes. Each answer publishes what is known so far.
fn query(
    intro: &Rc<Introspector>,
    state: &Rc<RefCell<AudioState>>,
    tx: &Rc<watch::Sender<Option<AudioState>>>,
) {
    let (i, s, t) = (intro.clone(), state.clone(), tx.clone());
    intro.get_server_info(move |info| {
        let sink = info
            .default_sink_name
            .as_deref()
            .unwrap_or_default()
            .to_owned();
        let source = info
            .default_source_name
            .as_deref()
            .unwrap_or_default()
            .to_owned();
        s.borrow_mut().default_sink = sink.clone();
        let (s1, t1) = (s.clone(), t.clone());
        i.get_sink_info_by_name(&sink, move |r| {
            if let ListResult::Item(info) = r {
                let mut st = s1.borrow_mut();
                st.volume = level(&info.volume);
                st.muted = info.mute;
                publish(&t1, st.clone());
            }
        });
        let (s2, t2) = (s.clone(), t.clone());
        i.get_source_info_by_name(&source, move |r| {
            if let ListResult::Item(info) = r {
                let mut st = s2.borrow_mut();
                st.mic_volume = level(&info.volume);
                st.mic_muted = info.mute;
                publish(&t2, st.clone());
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_match_caelestia() {
        assert_eq!(volume_icon(0.7, true), "no_sound");
        assert_eq!(volume_icon(0.5, false), "volume_up");
        assert_eq!(volume_icon(0.2, false), "volume_down");
        assert_eq!(volume_icon(0.0, false), "volume_mute");
    }
}
