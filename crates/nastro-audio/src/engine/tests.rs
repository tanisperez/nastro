//! Engine tests with an in-memory sink: no device, no network, fast.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::chunks_exact_to_as_chunks
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use nastro_core::{EqSettings, PlayState};
use pretty_assertions::assert_eq;

use super::test_sink::{Handle, TestSink};
use super::{Player, PlayerEvent};
use crate::analysis::AnalysisFrame;
use crate::decoder::SymphoniaDecoder;
use crate::test_util::{sine, sine_wav, write_wav};

const TIMEOUT: Duration = Duration::from_secs(10);

struct Rig {
    player: Player,
    sink: Handle,
    events: Receiver<PlayerEvent>,
    dir: tempfile::TempDir,
}

fn new_rig(rate: u32, channels: u16) -> Rig {
    let (sink, handle) = TestSink::new(rate, channels);
    let player = Player::with_parts(move || Box::new(sink), Arc::new(SymphoniaDecoder));
    let events = player.events();
    Rig {
        player,
        sink: handle,
        events,
        dir: tempfile::tempdir().unwrap(),
    }
}

impl Rig {
    fn wav(&self, name: &str, rate: u32, channels: u16, seconds: f32) -> PathBuf {
        sine_wav(self.dir.path(), name, rate, channels, seconds)
    }

    /// Collects events until `done` matches one, which is included.
    fn until(&self, done: impl Fn(&PlayerEvent) -> bool) -> Vec<PlayerEvent> {
        let start = Instant::now();
        let mut seen = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    let stop = done(&event);
                    seen.push(event);
                    if stop {
                        return seen;
                    }
                }
                Err(_) => {
                    assert!(start.elapsed() < TIMEOUT, "timed out, saw {seen:?}");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    fn until_finished(&self) -> Vec<PlayerEvent> {
        self.until(|e| matches!(e, PlayerEvent::Finished { .. }))
    }

    fn wait(&self, what: &str, condition: impl Fn(&Rig) -> bool) {
        let start = Instant::now();
        while !condition(self) {
            assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn frames(handle: &Handle, channels: usize) -> usize {
    handle.len() / channels
}

fn kinds(events: &[PlayerEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            PlayerEvent::Started { .. } => "started",
            PlayerEvent::Finished { .. } => "finished",
            PlayerEvent::Advanced { .. } => "advanced",
            PlayerEvent::StateChanged(PlayState::Playing) => "playing",
            PlayerEvent::StateChanged(PlayState::Paused) => "paused",
            PlayerEvent::StateChanged(PlayState::Stopped) => "stopped",
            PlayerEvent::Error { .. } => "error",
        })
        .collect()
}

/// Longest run of frames whose samples are all exactly zero.
fn longest_zero_run(samples: &[f32], channels: usize) -> usize {
    let (mut best, mut run) = (0, 0);
    for frame in samples.chunks_exact(channels) {
        if frame.iter().all(|s| *s == 0.0) {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

#[test]
fn plays_a_wav_from_start_to_finish() {
    let rig = new_rig(44_100, 2);
    let path = rig.wav("a.wav", 44_100, 2, 1.0);
    rig.player.load(path.clone(), true);
    let events = rig.until_finished();
    assert_eq!(
        kinds(&events),
        ["started", "playing", "stopped", "finished"]
    );
    let PlayerEvent::Started { info, duration, .. } = &events[0] else {
        panic!("no start")
    };
    assert_eq!(info.codec, "PCM");
    assert_eq!((info.sample_rate, info.channels), (44_100, 2));
    assert_eq!(*duration, Some(Duration::from_secs(1)));
    assert_eq!(frames(&rig.sink, 2), 44_100);
    let status = rig.player.status();
    assert_eq!(status.state, PlayState::Stopped);
    assert_eq!(status.position, Duration::ZERO);
    assert_eq!(status.path, Some(path));
    assert!(!rig.sink.running(), "the device must be idle once finished");
}

#[test]
fn the_device_opens_only_when_something_plays() {
    let rig = new_rig(44_100, 2);
    let path = rig.wav("a.wav", 44_100, 2, 0.2);
    rig.player.load(path, false);
    rig.until(|e| matches!(e, PlayerEvent::Started { .. }));
    assert!(!rig.sink.opened());
    let status = rig.player.status();
    assert_eq!(status.state, PlayState::Stopped);
    assert_eq!(status.duration, Some(Duration::from_millis(200)));
    assert_eq!(status.info.map(|i| i.codec), Some("PCM".to_string()));
}

#[test]
fn resamples_48k_mono_into_a_44k_stereo_device() {
    let rig = new_rig(44_100, 2);
    rig.player.load(rig.wav("m.wav", 48_000, 1, 1.0), true);
    rig.until_finished();
    let samples = rig.sink.samples();
    let count = samples.len() / 2;
    assert!((count as i64 - 44_100).abs() <= 8, "got {count} frames");
    assert!(
        samples.chunks_exact(2).all(|f| f[0] == f[1]),
        "mono must be copied to both sides"
    );
    // A 440 Hz tone at half scale survives the resampler.
    assert!(
        (peak(&samples) - 0.5).abs() < 0.02,
        "peak {}",
        peak(&samples)
    );
}

#[test]
fn downmixes_surround_and_fills_extra_device_channels() {
    let rig = new_rig(48_000, 4);
    rig.player.load(rig.wav("s.wav", 48_000, 2, 0.25), true);
    rig.until_finished();
    let samples = rig.sink.samples();
    assert_eq!(samples.len() / 4, 12_000);
    assert!(samples.chunks_exact(4).all(|f| f[2] == 0.0 && f[3] == 0.0));
    assert!(peak(&samples) > 0.4);
}

#[test]
fn pause_stops_consumption_and_play_resumes() {
    let rig = new_rig(44_100, 2);
    rig.player.load(rig.wav("a.wav", 44_100, 2, 3.0), true);
    rig.wait("some audio", |r| r.sink.len() > 10_000);
    rig.player.pause();
    rig.until(|e| matches!(e, PlayerEvent::StateChanged(PlayState::Paused)));
    std::thread::sleep(Duration::from_millis(30));
    let at_pause = rig.sink.len();
    let position = rig.player.status().position;
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(rig.sink.len(), at_pause);
    assert_eq!(rig.player.status().position, position);
    assert_eq!(rig.player.status().state, PlayState::Paused);
    assert!(!rig.sink.running());
    rig.player.play();
    rig.wait("more audio", |r| r.sink.len() > at_pause + 10_000);
    assert!(rig.player.status().position > position);
}

#[test]
fn stop_resets_the_position_and_idles_the_device() {
    let rig = new_rig(44_100, 2);
    let path = rig.wav("a.wav", 44_100, 2, 3.0);
    rig.player.load(path.clone(), true);
    rig.wait("some audio", |r| r.sink.len() > 20_000);
    assert!(rig.player.status().position > Duration::ZERO);
    rig.player.stop();
    rig.until(|e| matches!(e, PlayerEvent::StateChanged(PlayState::Stopped)));
    let status = rig.player.status();
    assert_eq!(status.state, PlayState::Stopped);
    assert_eq!(status.position, Duration::ZERO);
    assert_eq!(status.path, Some(path));
    assert!(!rig.sink.running());
    let len = rig.sink.len();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(rig.sink.len(), len);
    // Play restarts from 0: a full second play through yields the whole track again.
    rig.player.play();
    rig.until(|e| matches!(e, PlayerEvent::StateChanged(PlayState::Playing)));
    rig.wait("restart", |r| r.sink.len() > len + 20_000);
}

#[test]
fn seek_sets_an_exact_position_and_playback_continues_from_there() {
    let rig = new_rig(44_100, 2);
    let path = rig.wav("a.wav", 44_100, 2, 1.0);
    rig.player.load(path, false);
    rig.player.seek(Duration::from_millis(500));
    rig.wait("seek", |r| {
        r.player.status().position == Duration::from_millis(500)
    });
    rig.player.play();
    rig.until_finished();
    let count = frames(&rig.sink, 2);
    assert!((count as i64 - 22_050).abs() <= 2, "got {count} frames");
}

#[test]
fn seek_while_paused_is_exact_and_the_ring_is_flushed() {
    let rig = new_rig(44_100, 2);
    rig.player.load(rig.wav("a.wav", 44_100, 2, 5.0), true);
    rig.wait("some audio", |r| r.sink.len() > 20_000);
    rig.player.pause();
    rig.until(|e| matches!(e, PlayerEvent::StateChanged(PlayState::Paused)));
    rig.player.seek(Duration::from_secs(2));
    rig.wait("seek", |r| {
        r.player.status().position == Duration::from_secs(2)
    });
    let len = rig.sink.len();
    rig.player.play();
    rig.until_finished();
    let played_after = (rig.sink.len() - len) / 2;
    // Three seconds remain, give or take what had been consumed before the pause.
    assert!(
        (played_after as i64 - 3 * 44_100).abs() <= 2,
        "got {played_after} frames"
    );
}

#[test]
fn queue_next_continues_without_a_gap() {
    let rig = new_rig(44_100, 2);
    let a = rig.wav("a.wav", 44_100, 2, 0.5);
    let b = rig.wav("b.wav", 44_100, 2, 0.5);
    rig.player.load(a.clone(), true);
    rig.player.queue_next(Some(b.clone()));
    let events = rig.until_finished();
    assert!(events.contains(&PlayerEvent::Advanced {
        from: a,
        to: b.clone()
    }));
    let started: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            PlayerEvent::Started { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 2);
    assert_eq!(started[1], b);
    let advanced = events
        .iter()
        .position(|e| matches!(e, PlayerEvent::Advanced { .. }));
    let second_start = events
        .iter()
        .position(|e| matches!(e, PlayerEvent::Started { path, .. } if *path == b));
    assert!(
        advanced < second_start,
        "Started for the queued track follows Advanced"
    );
    let samples = rig.sink.samples();
    assert_eq!(samples.len() / 2, 44_100);
    assert!(
        longest_zero_run(&samples, 2) <= 2,
        "gap of {} frames",
        longest_zero_run(&samples, 2)
    );
}

#[test]
fn queue_next_with_another_format_still_advances() {
    let rig = new_rig(44_100, 2);
    let a = rig.wav("a.wav", 48_000, 1, 0.5);
    let b = rig.wav("b.wav", 22_050, 2, 0.5);
    rig.player.load(a, true);
    rig.player.queue_next(Some(b.clone()));
    let events = rig.until_finished();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlayerEvent::Advanced { to, .. } if *to == b))
    );
    let count = frames(&rig.sink, 2);
    assert!((count as i64 - 44_100).abs() <= 16, "got {count} frames");
    assert!(longest_zero_run(&rig.sink.samples(), 2) <= 4);
}

#[test]
fn status_follows_the_queued_track_when_it_starts() {
    let rig = new_rig(44_100, 2);
    let a = rig.wav("a.wav", 44_100, 2, 0.3);
    let b = rig.wav("b.wav", 44_100, 1, 0.3);
    rig.player.load(a, true);
    rig.player.queue_next(Some(b.clone()));
    rig.until(|e| matches!(e, PlayerEvent::Advanced { .. }));
    // Now b is current, with its own info.
    let status = rig.player.status();
    assert_eq!(status.path, Some(b));
    assert_eq!(status.info.map(|i| i.channels), Some(1));
}

#[test]
fn queue_next_none_ends_after_the_current_track() {
    let rig = new_rig(44_100, 2);
    let a = rig.wav("a.wav", 44_100, 2, 0.3);
    let b = rig.wav("b.wav", 44_100, 2, 0.3);
    rig.player.load(a, true);
    rig.player.queue_next(Some(b));
    rig.player.queue_next(None);
    let events = rig.until_finished();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, PlayerEvent::Advanced { .. }))
    );
    assert_eq!(frames(&rig.sink, 2), (44_100.0 * 0.3) as usize);
}

#[test]
fn volume_zero_is_silence_and_half_follows_the_cubic_curve() {
    let rig = new_rig(44_100, 2);
    rig.player.set_volume(0.0);
    rig.player.load(rig.wav("a.wav", 44_100, 2, 0.3), true);
    rig.until_finished();
    assert!(rig.sink.samples().iter().all(|s| *s == 0.0));

    let rig2 = new_rig(44_100, 2);
    rig2.player.set_volume(0.5);
    rig2.player.load(rig2.wav("a.wav", 44_100, 2, 0.3), true);
    rig2.until_finished();
    let peak = peak(&rig2.sink.samples());
    assert!((peak - 0.5 * 0.125).abs() < 0.002, "peak {peak}");
}

#[test]
fn balance_attenuates_the_far_side() {
    let rig = new_rig(44_100, 2);
    rig.player.set_balance(-1.0);
    rig.player.load(rig.wav("a.wav", 44_100, 2, 0.3), true);
    rig.until_finished();
    let samples = rig.sink.samples();
    assert!(samples.chunks_exact(2).all(|f| f[1] == 0.0));
    assert!(samples.chunks_exact(2).any(|f| f[0].abs() > 0.4));
}

#[test]
fn equalizer_preamp_changes_the_level() {
    let rig = new_rig(44_100, 2);
    let eq = EqSettings {
        preamp_db: 6.0,
        ..EqSettings::default()
    };
    rig.player.set_eq(eq);
    rig.player.load(rig.wav("a.wav", 44_100, 2, 0.3), true);
    rig.until_finished();
    let peak = peak(&rig.sink.samples());
    assert!((peak - 0.5 * 1.995).abs() < 0.02, "peak {peak}");
}

#[test]
fn a_bad_file_is_reported_and_leaves_nothing_loaded() {
    let rig = new_rig(44_100, 2);
    rig.player
        .load(PathBuf::from("/nonexistent/nope.wav"), true);
    let events = rig.until(|e| matches!(e, PlayerEvent::Error { .. }));
    assert!(
        matches!(&events[0], PlayerEvent::Error { path: Some(p), .. } if p.ends_with("nope.wav"))
    );
    rig.wait("cleared", |r| r.player.status().path.is_none());
    assert_eq!(rig.player.status().state, PlayState::Stopped);

    let text = rig.dir.path().join("notes.wav");
    std::fs::write(&text, b"this is not audio at all").unwrap();
    rig.player.load(text, true);
    rig.until(|e| matches!(e, PlayerEvent::Error { .. }));
}

#[test]
fn analysis_sees_the_tone_that_was_played() {
    let rig = new_rig(44_100, 2);
    let path = rig.dir.path().join("k.wav");
    write_wav(&path, 44_100, 2, &sine(44_100, 2, 0.5, 1_000.0, 0.8));
    rig.player.load(path, true);
    rig.until_finished();
    let mut frame = AnalysisFrame::default();
    rig.player.analysis(&mut frame);
    let loudest = (0..frame.bars.len())
        .max_by(|a, b| frame.bars[*a].total_cmp(&frame.bars[*b]))
        .unwrap();
    assert_eq!(loudest, 12, "{:?}", frame.bars);
    assert!(frame.scope.iter().any(|v| v.abs() > 0.3));
}

#[test]
fn analysis_before_playing_is_empty() {
    let rig = new_rig(44_100, 2);
    let mut frame = AnalysisFrame::default();
    frame.bars[3] = 1.0;
    rig.player.analysis(&mut frame);
    assert_eq!(frame, AnalysisFrame::default());
}

#[test]
fn dropping_the_last_handle_shuts_the_engine_down() {
    let rig = new_rig(44_100, 2);
    let Rig {
        player,
        sink,
        events: _events,
        dir,
    } = rig;
    player.load(sine_wav(dir.path(), "a.wav", 44_100, 2, 5.0), true);
    let clone = player.clone();
    drop(player);
    std::thread::sleep(Duration::from_millis(20));
    assert!(!sink.closed(), "a handle is still alive");
    drop(clone);
    let start = Instant::now();
    while !sink.closed() {
        assert!(start.elapsed() < TIMEOUT, "the engine did not stop");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn player_is_send_and_sync() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<Player>();
}
