//! The engine thread: owns the decoder stream, the pipeline and the device.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_channel::Sender;
use nastro_core::{EqSettings, PlayState};
use rtrb::{Producer, RingBuffer};

use super::PlayerEvent;
use super::pipeline::Pipeline;
use super::render::Renderer;
use super::shared::{Segment, Shared, frames_to_duration};
use super::sink::{AudioSink, SinkFormat};
use crate::decoder::{DecodedStream, Decoder};
use crate::error::AudioError;

/// Seconds of audio the ring holds, which is also the output latency of the engine.
const RING_SECONDS: f32 = 0.3;
/// How long to sleep when the ring is full.
const FULL_WAIT: Duration = Duration::from_millis(10);

/// What the handles ask the engine to do.
pub(crate) enum Command {
    Load { path: PathBuf, autoplay: bool },
    QueueNext(Option<PathBuf>),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    SetEq(EqSettings),
    Shutdown,
}

/// What the loop should do before looking at commands again.
enum Step {
    /// Work is left: only peek at the commands.
    Busy,
    /// Waiting for the ring to drain: sleep at most this long.
    Wait(Duration),
    /// Nothing to do until a command arrives.
    Idle,
}

/// The device side once opened.
struct Output {
    producer: Producer<f32>,
    consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
    format: SinkFormat,
}

/// The stream being decoded.
struct Track {
    stream: Box<dyn DecodedStream>,
    path: PathBuf,
    pipeline: Option<Pipeline>,
}

pub(crate) struct Engine {
    rx: Receiver<Command>,
    shared: Arc<Shared>,
    events: Sender<PlayerEvent>,
    decoder: Arc<dyn Decoder>,
    sink: Box<dyn AudioSink>,
    output: Option<Output>,
    eq: EqSettings,
    track: Option<Track>,
    queued: Option<PathBuf>,
    state: PlayState,
    /// Device frames produced by the pipeline so far, written to the ring or still pending.
    produced: u64,
    /// Device frames waiting for room in the ring.
    pending: Vec<f32>,
    pending_pos: usize,
    read_buf: Vec<f32>,
    /// The decoder reached its end and nothing follows: waiting for the output to drain.
    finishing: bool,
}

impl Engine {
    pub fn new(
        rx: Receiver<Command>,
        shared: Arc<Shared>,
        events: Sender<PlayerEvent>,
        decoder: Arc<dyn Decoder>,
        sink: Box<dyn AudioSink>,
    ) -> Self {
        Engine {
            rx,
            shared,
            events,
            decoder,
            sink,
            output: None,
            eq: EqSettings::default(),
            track: None,
            queued: None,
            state: PlayState::Stopped,
            produced: 0,
            pending: Vec::new(),
            pending_pos: 0,
            read_buf: Vec::new(),
            finishing: false,
        }
    }

    /// The loop of the thread. Returns on shutdown or when every handle is gone.
    pub fn run(mut self) {
        loop {
            self.poll_boundary();
            let command = match self.step() {
                Step::Busy => match self.rx.try_recv() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                },
                Step::Wait(wait) => match self.rx.recv_timeout(wait) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                Step::Idle => match self.rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                },
            };
            if let Some(command) = command {
                if matches!(command, Command::Shutdown) {
                    break;
                }
                self.handle(command);
            }
        }
        if let Err(err) = self.sink.pause() {
            log::debug!("pausing the output on shutdown: {err}");
        }
    }

    fn emit(&self, event: PlayerEvent) {
        // The channel is unbounded and only closes when every receiver is gone.
        let _ = self.events.try_send(event);
    }

    fn error(&self, path: Option<&Path>, err: &AudioError) {
        log::warn!(
            "{}: {err}",
            path.map(|p| p.display().to_string()).unwrap_or_default()
        );
        self.emit(PlayerEvent::Error {
            path: path.map(Path::to_path_buf),
            message: err.to_string(),
        });
    }

    fn set_state(&mut self, state: PlayState) {
        if self.state != state {
            self.state = state;
            self.shared.timeline().state = state;
            self.emit(PlayerEvent::StateChanged(state));
        }
    }

    // ---- commands ----

    fn handle(&mut self, command: Command) {
        match command {
            Command::Load { path, autoplay } => self.load(path, autoplay),
            Command::QueueNext(path) => self.queued = path,
            Command::Play => self.play(),
            Command::Pause => self.pause(),
            Command::Stop => self.stop(),
            Command::Seek(position) => self.seek(position),
            Command::SetEq(eq) => {
                self.eq = eq;
                if let Some(pipeline) = self.track.as_mut().and_then(|t| t.pipeline.as_mut()) {
                    pipeline.set_eq(&eq);
                }
            }
            Command::Shutdown => {}
        }
    }

    fn open_track(&self, path: &Path) -> Result<Track, AudioError> {
        let stream = self.decoder.open(path)?;
        Ok(Track {
            stream,
            path: path.to_path_buf(),
            pipeline: None,
        })
    }

    fn segment_for(&self, track: &Track, start_pos: Duration) -> Segment {
        Segment {
            start_abs: self.produced,
            start_pos,
            path: track.path.clone(),
            info: track.stream.info().clone(),
            duration: track.stream.duration(),
        }
    }

    fn load(&mut self, path: PathBuf, autoplay: bool) {
        match self.open_track(&path) {
            Err(err) => {
                self.error(Some(&path), &err);
                self.pause_output();
                self.flush();
                self.track = None;
                self.queued = None;
                self.shared.timeline().segments.clear();
                self.set_state(PlayState::Stopped);
            }
            Ok(track) => {
                self.pause_output();
                self.flush();
                self.queued = None;
                let segment = self.segment_for(&track, Duration::ZERO);
                self.shared.timeline().segments = [segment.clone()].into();
                self.track = Some(track);
                self.set_state(PlayState::Stopped);
                self.emit(PlayerEvent::Started {
                    path: segment.path,
                    info: segment.info,
                    duration: segment.duration,
                });
                if autoplay {
                    self.play();
                }
            }
        }
    }

    /// The segment being heard, if any.
    fn heard(&self) -> Option<Segment> {
        self.shared.timeline().segments.front().cloned()
    }

    fn play(&mut self) {
        match self.state {
            PlayState::Playing => {}
            PlayState::Paused => {
                if let Err(err) = self.sink.resume() {
                    self.error(None, &err);
                    return;
                }
                self.set_state(PlayState::Playing);
            }
            PlayState::Stopped => {
                let Some(heard) = self.heard() else { return };
                if let Err(err) = self.ensure_output() {
                    self.error(Some(&heard.path), &err);
                    return;
                }
                // Always start from the loaded position: a fresh stream, since the old one may
                // be at its end or ahead of what was heard.
                let reopened = self.open_track(&heard.path).and_then(|mut track| {
                    if !heard.start_pos.is_zero() {
                        track.stream.seek(heard.start_pos)?;
                    }
                    Ok(track)
                });
                match reopened {
                    Ok(track) => self.track = Some(track),
                    Err(err) => {
                        self.error(Some(&heard.path), &err);
                        return;
                    }
                }
                self.flush();
                let segment = Segment {
                    start_abs: self.produced,
                    ..heard
                };
                self.shared.timeline().segments = [segment].into();
                self.finishing = false;
                if let Err(err) = self.sink.resume() {
                    self.error(None, &err);
                    return;
                }
                self.set_state(PlayState::Playing);
            }
        }
    }

    fn pause(&mut self) {
        if self.state == PlayState::Playing {
            self.pause_output();
            self.set_state(PlayState::Paused);
        }
    }

    fn stop(&mut self) {
        if self.state == PlayState::Stopped && self.heard().is_none_or(|h| h.start_pos.is_zero()) {
            return;
        }
        self.pause_output();
        self.flush();
        if let Some(mut heard) = self.heard() {
            heard.start_abs = self.produced;
            heard.start_pos = Duration::ZERO;
            self.shared.timeline().segments = [heard].into();
        }
        self.finishing = false;
        self.set_state(PlayState::Stopped);
    }

    fn seek(&mut self, position: Duration) {
        let Some(heard) = self.heard() else { return };
        self.poll_boundary();
        let heard = self.heard().unwrap_or(heard);
        let position = match heard.duration {
            Some(duration) => position.min(duration.saturating_sub(Duration::from_millis(1))),
            None => position,
        };
        // The stream may be on the queued track already: go back to the one being heard.
        let ahead = self.track.as_ref().is_none_or(|t| t.path != heard.path);
        if ahead {
            let ahead_path = self.track.as_ref().map(|t| t.path.clone());
            match self.open_track(&heard.path) {
                Ok(track) => {
                    self.track = Some(track);
                    if self.queued.is_none() {
                        self.queued = ahead_path;
                    }
                }
                Err(err) => {
                    self.error(Some(&heard.path), &err);
                    return;
                }
            }
        }
        let Some(track) = self.track.as_mut() else {
            return;
        };
        let reached = match track.stream.seek(position) {
            Ok(reached) => reached,
            Err(err) => {
                let path = track.path.clone();
                self.error(Some(&path), &err);
                return;
            }
        };
        track.pipeline = None;
        self.flush();
        self.finishing = false;
        let mut segment = heard;
        segment.start_abs = self.produced;
        segment.start_pos = reached;
        self.shared.timeline().segments = [segment].into();
    }

    // ---- output ----

    fn pause_output(&mut self) {
        if let Err(err) = self.sink.pause() {
            log::debug!("pausing the output: {err}");
        }
    }

    /// Opens the device on first use.
    fn ensure_output(&mut self) -> Result<(), AudioError> {
        if self.output.is_some() {
            return Ok(());
        }
        let format = self.sink.format()?;
        let channels = usize::from(format.channels.max(1));
        let frames = (format.sample_rate as f32 * RING_SECONDS) as usize;
        let (producer, consumer) = RingBuffer::new(frames.max(1024) * channels);
        let consumer = Arc::new(Mutex::new(consumer));
        let renderer = Renderer::new(
            consumer.clone(),
            self.shared.clone(),
            format.sample_rate,
            format.channels,
        );
        self.sink.open(renderer)?;
        self.shared
            .device_rate
            .store(format.sample_rate, Ordering::Release);
        self.output = Some(Output {
            producer,
            consumer,
            format,
        });
        Ok(())
    }

    /// Drops everything buffered: the ring and what waits for it. Afterwards `produced`,
    /// the frames pushed and the frames played are all the same number.
    fn flush(&mut self) {
        if let Some(output) = &self.output {
            let channels = usize::from(output.format.channels.max(1));
            let mut consumer = output
                .consumer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let slots = consumer.slots();
            if slots > 0 {
                if let Ok(chunk) = consumer.read_chunk(slots) {
                    chunk.commit_all();
                }
                self.shared
                    .played
                    .fetch_add((slots / channels) as u64, Ordering::Release);
            }
        }
        self.pending.clear();
        self.pending_pos = 0;
        self.produced = self.shared.played();
    }

    /// Announces the queued track when playback crosses into it.
    fn poll_boundary(&mut self) {
        let played = self.shared.played();
        loop {
            let crossed = {
                let mut timeline = self.shared.timeline();
                if timeline.segments.len() > 1 && timeline.segments[1].start_abs <= played {
                    let from = timeline.segments.pop_front();
                    from.zip(timeline.segments.front().cloned())
                } else {
                    None
                }
            };
            let Some((from, to)) = crossed else { break };
            self.emit(PlayerEvent::Advanced {
                from: from.path,
                to: to.path.clone(),
            });
            self.emit(PlayerEvent::Started {
                path: to.path,
                info: to.info,
                duration: to.duration,
            });
        }
    }

    // ---- decoding ----

    fn step(&mut self) -> Step {
        if self.state != PlayState::Playing {
            return Step::Idle;
        }
        if self.push_pending() {
            return Step::Wait(FULL_WAIT);
        }
        if self.finishing {
            return self.finish_if_drained();
        }
        self.decode_chunk();
        Step::Busy
    }

    /// Moves pending frames into the ring. Returns true while some are left.
    fn push_pending(&mut self) -> bool {
        let Some(output) = self.output.as_mut() else {
            return false;
        };
        let channels = usize::from(output.format.channels.max(1));
        let left = self.pending.len() - self.pending_pos;
        if left == 0 {
            return false;
        }
        let room = output.producer.slots() / channels * channels;
        let count = room.min(left);
        if count > 0
            && let Ok(chunk) = output.producer.write_chunk_uninit(count)
        {
            let source = &self.pending[self.pending_pos..self.pending_pos + count];
            chunk.fill_from_iter(source.iter().copied());
            self.pending_pos += count;
        }
        if self.pending_pos >= self.pending.len() {
            self.pending.clear();
            self.pending_pos = 0;
            false
        } else {
            true
        }
    }

    fn finish_if_drained(&mut self) -> Step {
        let Some(output) = &self.output else {
            return Step::Idle;
        };
        let rate = output.format.sample_rate;
        let remaining = self.produced.saturating_sub(self.shared.played());
        if remaining > 0 {
            let wait = frames_to_duration(remaining, rate).min(FULL_WAIT * 2);
            return Step::Wait(wait.max(Duration::from_millis(1)));
        }
        self.poll_boundary();
        self.finishing = false;
        self.pause_output();
        let path = self.heard().map(|h| h.path);
        if let Some(mut heard) = self.heard() {
            heard.start_abs = self.produced;
            heard.start_pos = Duration::ZERO;
            self.shared.timeline().segments = [heard].into();
        }
        self.set_state(PlayState::Stopped);
        if let Some(path) = path {
            self.emit(PlayerEvent::Finished { path });
        }
        Step::Idle
    }

    /// Reads and processes one chunk of the stream into `pending`.
    fn decode_chunk(&mut self) {
        if let Err(err) = self.ensure_pipeline() {
            let path = self.track.as_ref().map(|t| t.path.clone());
            self.error(path.as_deref(), &err);
            self.pause_output();
            self.set_state(PlayState::Stopped);
            return;
        }
        let Some(format) = self.output.as_ref().map(|o| o.format) else {
            return;
        };
        let Some(track) = self.track.as_mut() else {
            self.finishing = true;
            return;
        };
        self.read_buf.clear();
        match track.stream.read(&mut self.read_buf) {
            Ok(true) => {
                if let Some(pipeline) = track.pipeline.as_mut() {
                    let before = self.pending.len();
                    pipeline.process(&mut self.read_buf, &mut self.pending);
                    let added = (self.pending.len() - before) / usize::from(format.channels.max(1));
                    self.produced += added as u64;
                }
            }
            Ok(false) => self.end_of_stream(),
            Err(err) => {
                let path = track.path.clone();
                self.error(Some(&path), &err);
                self.end_of_stream();
            }
        }
    }

    fn ensure_pipeline(&mut self) -> Result<(), AudioError> {
        self.ensure_output()?;
        let Some(format) = self.output.as_ref().map(|o| o.format) else {
            return Ok(());
        };
        if let Some(track) = self.track.as_mut()
            && track.pipeline.is_none()
        {
            track.pipeline = Some(Pipeline::new(
                track.stream.sample_rate(),
                track.stream.channels(),
                format.sample_rate,
                format.channels,
                &self.eq,
            )?);
        }
        Ok(())
    }

    /// The stream ended: continue with the queued track, or start draining.
    fn end_of_stream(&mut self) {
        let channels = self
            .output
            .as_ref()
            .map_or(2, |o| usize::from(o.format.channels.max(1)));
        let Some(mut old) = self.track.take() else {
            self.finishing = true;
            return;
        };
        let next = self.queued.take();
        let next_track = next.as_ref().and_then(|path| match self.open_track(path) {
            Ok(track) => Some(track),
            Err(err) => {
                self.error(Some(path), &err);
                None
            }
        });
        let Some(mut next_track) = next_track else {
            if let Some(pipeline) = old.pipeline.as_mut() {
                let before = self.pending.len();
                pipeline.flush(&mut self.pending);
                self.produced += ((self.pending.len() - before) / channels) as u64;
            }
            self.track = Some(old);
            self.finishing = true;
            return;
        };
        // Same rate and layout: keep the pipeline, so nothing is flushed and the output
        // continues without a gap. Otherwise flush the tail and build a new one.
        let reuse = old.pipeline.as_ref().is_some_and(|p| {
            p.fits(
                next_track.stream.sample_rate(),
                next_track.stream.channels(),
            )
        });
        if reuse {
            next_track.pipeline = old.pipeline.take();
        } else if let Some(pipeline) = old.pipeline.as_mut() {
            let before = self.pending.len();
            pipeline.flush(&mut self.pending);
            self.produced += ((self.pending.len() - before) / channels) as u64;
        }
        let segment = self.segment_for(&next_track, Duration::ZERO);
        self.shared.timeline().segments.push_back(segment);
        self.track = Some(next_track);
    }
}
