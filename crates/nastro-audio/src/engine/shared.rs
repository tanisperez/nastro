//! State shared between the `Player` handles, the engine thread and the output callback.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use nastro_core::{PlayState, StreamInfo};

/// Mono samples kept for the visualizer, a power of two.
pub(crate) const TAP_SIZE: usize = 4096;

/// A stretch of output frames that belongs to one track.
#[derive(Debug, Clone)]
pub(crate) struct Segment {
    /// Absolute output frame (frames the device ever consumed) at which the segment starts.
    pub start_abs: u64,
    /// Position in the track of its first frame.
    pub start_pos: Duration,
    pub path: PathBuf,
    pub info: StreamInfo,
    pub duration: Option<Duration>,
}

/// The transport state and the segments still to be played or being played.
#[derive(Debug, Default)]
pub(crate) struct Timeline {
    pub state: PlayState,
    /// The first segment is the one being heard, the next ones are queued behind it.
    pub segments: VecDeque<Segment>,
}

/// The latest mono samples that went to the device, for the visualizer.
#[derive(Debug)]
pub(crate) struct Tap {
    pub data: Vec<f32>,
    /// Total samples ever written.
    pub total: u64,
}

/// Everything the three sides share.
#[derive(Debug)]
pub(crate) struct Shared {
    /// Frames the output callback consumed from the ring, in device frames.
    pub played: AtomicU64,
    /// Device sample rate, 0 until the output is opened.
    pub device_rate: AtomicU32,
    /// Target amplitude (`f32` bits).
    pub amplitude: AtomicU32,
    /// Target balance (`f32` bits).
    pub balance: AtomicU32,
    pub timeline: Mutex<Timeline>,
    pub tap: Mutex<Tap>,
}

impl Shared {
    /// Fresh state: full volume, centred, stopped.
    pub fn new() -> Self {
        Shared {
            played: AtomicU64::new(0),
            device_rate: AtomicU32::new(0),
            amplitude: AtomicU32::new(1.0f32.to_bits()),
            balance: AtomicU32::new(0.0f32.to_bits()),
            timeline: Mutex::new(Timeline::default()),
            tap: Mutex::new(Tap {
                data: vec![0.0; TAP_SIZE],
                total: 0,
            }),
        }
    }

    /// Locks the timeline, recovering from a poisoned lock (the data stays consistent).
    pub fn timeline(&self) -> std::sync::MutexGuard<'_, Timeline> {
        self.timeline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Frames consumed so far.
    pub fn played(&self) -> u64 {
        self.played.load(Ordering::Acquire)
    }
}

/// Converts a frame count at `rate` to a duration.
pub(crate) fn frames_to_duration(frames: u64, rate: u32) -> Duration {
    if rate == 0 {
        return Duration::ZERO;
    }
    Duration::from_nanos((u128::from(frames) * 1_000_000_000 / u128::from(rate)) as u64)
}
