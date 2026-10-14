//! The transport: the audio engine handle, the latest status and visualizer frame, volume and
//! balance. A gpui entity that animates only while playing: the frame task runs at
//! [`FRAME_INTERVAL`] while the engine plays and does not exist otherwise, and engine events
//! arrive through an async channel, so a paused or stopped player costs nothing.
//!
//! Each frame notifies only the small [`Frame`] entity, which the display observes; the
//! transport itself notifies when something the rest of the player shows changes (play state,
//! track, volume), so the buttons, menus and sliders are not rebuilt 30 times per second.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{AppContext as _, AsyncApp, Context, Entity, Task, WeakEntity};
use nastro_audio::{AnalysisFrame, Player, PlayerEvent, PlayerStatus};
use nastro_core::{EqSettings, PlayState};

use crate::app::commands;

/// Time between two frames of the display while playing, about 30 frames per second.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// How far the seek shortcuts jump.
pub const SEEK_STEP: Duration = Duration::from_secs(5);

/// How much the volume shortcuts change the volume.
pub const VOLUME_STEP: f32 = 0.05;

/// What changes on every frame while playing: the visualizer. The position lives in
/// [`Transport::status`], updated on the same frame; observers of this entity read both.
#[derive(Default)]
pub struct Frame {
    /// The visualizer frame read on the last frame.
    pub analysis: AnalysisFrame,
}

/// The transport entity.
pub struct Transport {
    /// The engine.
    pub player: Player,
    /// The status read on the last frame or event.
    pub status: PlayerStatus,
    /// The per-frame data, notified on every frame while playing.
    pub frame: Entity<Frame>,
    /// Volume, 0..=1.
    pub volume: f32,
    /// Balance, -1..=1.
    pub balance: f32,
    /// Where the seek bar is being dragged, as a fraction, until the button is released.
    pub scrub: Option<f32>,
    /// The last error reported by the engine, shown in the display until the next track starts.
    pub error: Option<String>,
    /// How many tracks failed in a row, so a playlist of unreadable files does not loop forever.
    pub failures_in_a_row: usize,
    /// The frame task, alive only while playing.
    frames: Option<Task<()>>,
    /// Whether the frame task is still looping. It stops by itself when it sees the engine is
    /// no longer playing, and dropping a task from inside itself would cancel it mid-poll.
    frames_running: bool,
    /// The task that receives engine events.
    _events: Task<()>,
}

impl Transport {
    /// Starts the engine with the session's volume, balance and equalizer.
    pub fn new(volume: f32, balance: f32, eq: EqSettings, cx: &mut Context<Self>) -> Self {
        let player = Player::new();
        player.set_volume(volume);
        player.set_balance(balance);
        player.set_eq(eq);
        let events = player.events();
        let task = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while let Ok(event) = events.recv().await {
                let handled = this.update(cx, |transport, cx| transport.on_event(event, cx));
                if handled.is_err() {
                    break;
                }
            }
        });
        Transport {
            status: player.status(),
            player,
            frame: cx.new(|_| Frame::default()),
            volume,
            balance,
            scrub: None,
            error: None,
            failures_in_a_row: 0,
            frames: None,
            frames_running: false,
            _events: task,
        }
    }

    /// Whether audio is playing.
    pub fn is_playing(&self) -> bool {
        self.status.state.is_playing()
    }

    /// The position the display shows: the scrub position while the seek bar is dragged.
    pub fn shown_position(&self) -> Duration {
        match (self.scrub, self.status.duration) {
            (Some(fraction), Some(length)) => length.mul_f32(fraction.clamp(0.0, 1.0)),
            _ => self.status.position,
        }
    }

    /// Loads `path` and plays it (or leaves it paused at the start).
    pub fn load(&mut self, path: PathBuf, autoplay: bool, cx: &mut Context<Self>) {
        log::info!("playing {}", path.display());
        self.error = None;
        self.scrub = None;
        self.player.load(path, autoplay);
        cx.notify();
    }

    /// Sets the volume.
    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let volume = volume.clamp(0.0, 1.0);
        if volume != self.volume {
            self.volume = volume;
            self.player.set_volume(volume);
            crate::app::persist::schedule(cx);
            cx.notify();
        }
    }

    /// Sets the balance.
    pub fn set_balance(&mut self, balance: f32, cx: &mut Context<Self>) {
        let balance = balance.clamp(-1.0, 1.0);
        if balance != self.balance {
            self.balance = balance;
            self.player.set_balance(balance);
            crate::app::persist::schedule(cx);
            cx.notify();
        }
    }

    /// Moves the scrub position while the seek bar is dragged.
    pub fn set_scrub(&mut self, fraction: Option<f32>, cx: &mut Context<Self>) {
        self.scrub = fraction;
        cx.notify();
    }

    /// Seeks to the scrub position and ends the drag.
    pub fn commit_scrub(&mut self, cx: &mut Context<Self>) {
        if let (Some(fraction), Some(length)) = (self.scrub.take(), self.status.duration) {
            let target = length.mul_f32(fraction.clamp(0.0, 1.0));
            self.player.seek(target);
            self.status.position = target;
        }
        cx.notify();
    }

    /// Jumps `forward` or back by [`SEEK_STEP`].
    pub fn seek_by(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.status.path.is_none() {
            return;
        }
        let position = self.status.position;
        let target = if forward {
            position + SEEK_STEP
        } else {
            position.saturating_sub(SEEK_STEP)
        };
        let target = match self.status.duration {
            Some(length) => target.min(length),
            None => target,
        };
        self.player.seek(target);
        self.status.position = target;
        cx.notify();
    }

    /// Handles an engine event.
    fn on_event(&mut self, event: PlayerEvent, cx: &mut Context<Self>) {
        log::debug!("engine event: {event:?}");
        self.status = self.player.status();
        match event {
            PlayerEvent::StateChanged(state) => {
                self.status.state = state;
                self.update_frames(cx);
            }
            PlayerEvent::Started { path, .. } => {
                self.failures_in_a_row = 0;
                self.error = None;
                self.update_frames(cx);
                cx.defer(move |cx| commands::on_track_started(path, cx));
            }
            PlayerEvent::Advanced { from: _, to } => {
                cx.defer(move |cx| commands::on_track_advanced(to, cx));
            }
            PlayerEvent::Finished { .. } => {
                self.update_frames(cx);
                cx.defer(commands::on_playlist_track_finished);
            }
            PlayerEvent::Error { path, message } => {
                log::warn!(
                    "playback error{}: {message}",
                    path.as_ref()
                        .map(|path| format!(" in {}", path.display()))
                        .unwrap_or_default()
                );
                self.error = Some(message);
                self.failures_in_a_row += 1;
                self.update_frames(cx);
                if path.is_some() {
                    cx.defer(commands::on_track_failed);
                }
            }
        }
        cx.notify();
    }

    /// Starts the frame task when playing and drops it otherwise.
    fn update_frames(&mut self, cx: &mut Context<Self>) {
        if self.status.state != PlayState::Playing {
            self.frames = None;
            self.frames_running = false;
            return;
        }
        if self.frames_running {
            return;
        }
        self.frames_running = true;
        self.frames = Some(
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    cx.background_executor().timer(FRAME_INTERVAL).await;
                    let alive = this.update(cx, |transport, cx| {
                        let status = transport.player.status();
                        let changed = status.state != transport.status.state
                            || status.path != transport.status.path
                            || status.duration != transport.status.duration
                            || status.info != transport.status.info;
                        transport.status = status;
                        let player = transport.player.clone();
                        transport.frame.update(cx, |frame, cx| {
                            player.analysis(&mut frame.analysis);
                            cx.notify();
                        });
                        if changed {
                            cx.notify();
                        }
                        transport.status.state == PlayState::Playing
                    });
                    if !matches!(alive, Ok(true)) {
                        break;
                    }
                }
                let _ = this.update(cx, |transport, _| transport.frames_running = false);
            }),
        );
    }
}
