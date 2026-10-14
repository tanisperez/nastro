//! The player: a cheap handle over an engine thread that decodes, filters, resamples and feeds
//! the output device.

mod pipeline;
mod render;
mod shared;
mod sink;
mod thread;

#[cfg(test)]
mod test_sink;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use nastro_core::{EqSettings, PlayState, StreamInfo};

pub use render::Renderer;
pub use sink::{AudioSink, CpalSink, SinkFormat};

use self::render::volume_to_amplitude;
use self::shared::{Shared, TAP_SIZE, frames_to_duration};
use self::thread::{Command, Engine};
use crate::analysis::{AnalysisFrame, Analyzer};
use crate::decoder::{Decoder, SymphoniaDecoder};

/// What the player tells the UI. Delivered through [`Player::events`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// A track became the current one: after a `load`, and after a gapless move to the queued
    /// track. It carries what the display needs. It does not mean the audio is running; for
    /// that follow [`PlayerEvent::StateChanged`].
    Started {
        /// The file.
        path: PathBuf,
        /// Technical details of its stream.
        info: StreamInfo,
        /// Its length, when known.
        duration: Option<Duration>,
    },
    /// The track reached its end with nothing queued; the player is now stopped.
    Finished {
        /// The file that ended.
        path: PathBuf,
    },
    /// Playback moved on to the queued track without a gap. A [`PlayerEvent::Started`] for
    /// `to` follows.
    Advanced {
        /// The track that ended.
        from: PathBuf,
        /// The queued track now playing.
        to: PathBuf,
    },
    /// The transport state changed.
    StateChanged(PlayState),
    /// Something failed: a file that cannot be opened or decoded, or the output device.
    Error {
        /// The file involved, when there is one.
        path: Option<PathBuf>,
        /// A message for the log, not for translation.
        message: String,
    },
}

/// A snapshot of the player, cheap to take from the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStatus {
    /// Transport state.
    pub state: PlayState,
    /// The current track, if one is loaded.
    pub path: Option<PathBuf>,
    /// Where the audio being heard is in the track.
    pub position: Duration,
    /// Length of the track, when known.
    pub duration: Option<Duration>,
    /// Technical details of the current track.
    pub info: Option<StreamInfo>,
}

/// Samples fed to the analyzer when nothing new arrived, so the bars fall.
const SILENCE_FEED: usize = 1024;

/// The visualizer side, owned by the handles.
struct AnalysisState {
    analyzer: Option<Analyzer>,
    last_total: u64,
    scratch: Vec<f32>,
}

/// Owner of the engine thread. Dropping the last handle shuts the thread down.
struct Inner {
    commands: Sender<Command>,
    shared: Arc<Shared>,
    events: async_channel::Receiver<PlayerEvent>,
    analysis: Mutex<AnalysisState>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle
            && handle.join().is_err()
        {
            log::error!("the audio engine thread panicked");
        }
    }
}

/// The audio player. Cloning gives another handle to the same engine.
///
/// All methods take `&self` and return immediately: commands go to the engine thread, and
/// the outcome arrives as [`PlayerEvent`]s.
#[derive(Clone)]
pub struct Player {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player").finish_non_exhaustive()
    }
}

impl Default for Player {
    fn default() -> Self {
        Player::new()
    }
}

impl Player {
    /// A player on the default output device. It never fails: the device is opened the first
    /// time something plays, and a missing device is reported as a [`PlayerEvent::Error`].
    pub fn new() -> Player {
        Player::with_parts(|| Box::new(CpalSink::new()), Arc::new(SymphoniaDecoder))
    }

    /// A player with its own output and decoder, for tests. `make_sink` runs on the engine
    /// thread, because some devices cannot move between threads.
    #[doc(hidden)]
    pub fn with_parts<F>(make_sink: F, decoder: Arc<dyn Decoder>) -> Player
    where
        F: FnOnce() -> Box<dyn AudioSink> + Send + 'static,
    {
        let (commands, rx) = mpsc::channel();
        let (event_tx, events) = async_channel::unbounded();
        let shared = Arc::new(Shared::new());
        let engine_shared = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("nastro-audio".into())
            .spawn(move || {
                Engine::new(rx, engine_shared, event_tx, decoder, make_sink()).run();
            });
        let thread = match spawned {
            Ok(handle) => Some(handle),
            Err(err) => {
                log::error!("cannot start the audio engine thread: {err}");
                None
            }
        };
        Player {
            inner: Arc::new(Inner {
                commands,
                shared,
                events,
                analysis: Mutex::new(AnalysisState {
                    analyzer: None,
                    last_total: 0,
                    scratch: Vec::with_capacity(TAP_SIZE),
                }),
                thread: Mutex::new(thread),
            }),
        }
    }

    fn send(&self, command: Command) {
        // Fails only when the engine thread is gone, and then there is nothing to command.
        let _ = self.inner.commands.send(command);
    }

    /// Opens `path` as the current track. With `autoplay` it starts playing, otherwise the
    /// player is stopped at 0. Any queued track is forgotten.
    pub fn load(&self, path: PathBuf, autoplay: bool) {
        self.send(Command::Load { path, autoplay });
    }

    /// Sets the track that follows the current one without a gap, or clears it.
    pub fn queue_next(&self, path: Option<PathBuf>) {
        self.send(Command::QueueNext(path));
    }

    /// Resumes when paused, or restarts the loaded track from its start when stopped.
    pub fn play(&self) {
        self.send(Command::Play);
    }

    /// Pauses at the current position. The device and the engine thread go idle.
    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    /// Stops and puts the position back at 0. The device and the engine thread go idle.
    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    /// Moves to `position` in the current track.
    pub fn seek(&self, position: Duration) {
        self.send(Command::Seek(position));
    }

    /// Sets the volume from a slider fraction in `0..=1`. The amplitude follows a cubic
    /// curve (`fraction^3`): half way is -18 dB, a tenth is -60 dB, zero is silence.
    /// Changes are ramped over 10 ms.
    pub fn set_volume(&self, volume: f32) {
        let amplitude = volume_to_amplitude(volume);
        self.inner
            .shared
            .amplitude
            .store(amplitude.to_bits(), Ordering::Relaxed);
    }

    /// Sets the balance in `-1..=1` (left to right). Linear pan: the side away from the
    /// balance is attenuated in proportion, the centre leaves both sides at full level.
    /// Changes are ramped over 10 ms.
    pub fn set_balance(&self, balance: f32) {
        let balance = if balance.is_finite() {
            balance.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.inner
            .shared
            .balance
            .store(balance.to_bits(), Ordering::Relaxed);
    }

    /// Sets the equalizer. It applies to the audio decoded from now on.
    pub fn set_eq(&self, eq: EqSettings) {
        self.send(Command::SetEq(eq));
    }

    /// The state of the player now. The position follows what comes out of the device.
    pub fn status(&self) -> PlayerStatus {
        let shared = &self.inner.shared;
        let rate = shared.device_rate.load(Ordering::Acquire);
        let timeline = shared.timeline();
        let played = shared.played();
        let state = timeline.state;
        let segment = timeline
            .segments
            .iter()
            .rev()
            .find(|segment| segment.start_abs <= played)
            .or(timeline.segments.front());
        let Some(segment) = segment else {
            return PlayerStatus {
                state,
                path: None,
                position: Duration::ZERO,
                duration: None,
                info: None,
            };
        };
        let mut position = segment.start_pos;
        if matches!(state, PlayState::Playing | PlayState::Paused) {
            position += frames_to_duration(played.saturating_sub(segment.start_abs), rate);
        }
        if let Some(duration) = segment.duration {
            position = position.min(duration);
        }
        PlayerStatus {
            state,
            path: Some(segment.path.clone()),
            position,
            duration: segment.duration,
            info: Some(segment.info.clone()),
        }
    }

    /// Fills `frame` from the samples the output consumed most recently. Meant to be called
    /// about 30 times a second while playing. Never blocks the audio callback.
    pub fn analysis(&self, frame: &mut AnalysisFrame) {
        let shared = &self.inner.shared;
        let rate = shared.device_rate.load(Ordering::Acquire);
        if rate == 0 {
            *frame = AnalysisFrame::default();
            return;
        }
        let mut guard = self
            .inner
            .analysis
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let state = &mut *guard;
        if state.analyzer.as_ref().map(Analyzer::sample_rate) != Some(rate) {
            state.analyzer = Some(Analyzer::new(rate));
        }
        let Some(analyzer) = state.analyzer.as_mut() else {
            return;
        };
        state.scratch.clear();
        {
            let tap = shared.tap.lock().unwrap_or_else(|p| p.into_inner());
            let fresh = (tap.total - state.last_total).min(TAP_SIZE as u64) as usize;
            let end = tap.total as usize;
            state
                .scratch
                .extend((end - fresh..end).map(|n| tap.data[n % TAP_SIZE]));
            state.last_total = tap.total;
        }
        if state.scratch.is_empty() {
            state.scratch.resize(SILENCE_FEED, 0.0);
        }
        analyzer.push(&state.scratch);
        analyzer.frame(frame);
    }

    /// A receiver of the player's events. Every call returns another receiver on the same
    /// queue: each event goes to one of them, so the UI should keep a single one.
    pub fn events(&self) -> async_channel::Receiver<PlayerEvent> {
        self.inner.events.clone()
    }
}
