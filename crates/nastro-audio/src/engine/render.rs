//! The output stage: what the device callback runs.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use rtrb::Consumer;

use super::shared::{Shared, TAP_SIZE};

/// Time a gain takes to go from silence to full scale, so that moving the volume or balance
/// does not make zipper noise.
const RAMP_SECONDS: f32 = 0.01;

/// Amplitude for a volume slider fraction: a cubic curve, so the slider feels even to the ear.
/// Half way is -18 dB and a tenth is -60 dB. Zero is true silence.
pub(crate) fn volume_to_amplitude(volume: f32) -> f32 {
    if !volume.is_finite() {
        return 0.0;
    }
    volume.clamp(0.0, 1.0).powi(3)
}

/// The consumer side of the pipeline: drains the ring, applies volume and balance, and feeds
/// the analysis tap. [`render`](Renderer::render) never allocates, blocks, logs or waits for
/// a lock: on contention it writes silence.
pub struct Renderer {
    consumer: Arc<Mutex<Consumer<f32>>>,
    shared: Arc<Shared>,
    channels: usize,
    step: f32,
    gains: [f32; 2],
    started: bool,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

impl Renderer {
    pub(crate) fn new(
        consumer: Arc<Mutex<Consumer<f32>>>,
        shared: Arc<Shared>,
        sample_rate: u32,
        channels: u16,
    ) -> Self {
        Renderer {
            consumer,
            shared,
            channels: usize::from(channels.max(1)),
            step: 1.0 / (RAMP_SECONDS * sample_rate.max(1) as f32),
            gains: [1.0; 2],
            started: false,
        }
    }

    /// Target gain of the first two channels from the volume and balance atomics. The side
    /// away from the balance is attenuated linearly, the centre leaves both at unity.
    fn target_gains(&self) -> [f32; 2] {
        let amplitude = f32::from_bits(self.shared.amplitude.load(Ordering::Relaxed));
        let balance = f32::from_bits(self.shared.balance.load(Ordering::Relaxed));
        let balance = if balance.is_finite() {
            balance.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let left = amplitude * (1.0 - balance).min(1.0);
        let right = amplitude * (1.0 + balance).min(1.0);
        if self.channels == 1 {
            [(left + right) * 0.5; 2]
        } else {
            [left, right]
        }
    }

    /// Fills `out` (interleaved, device layout) and returns how many real frames came from
    /// the ring; the rest of `out` is silence.
    pub fn render(&mut self, out: &mut [f32]) -> usize {
        let channels = self.channels;
        let real = self.drain(out);
        out[real * channels..].fill(0.0);
        if real == 0 {
            return 0;
        }
        self.tap(&out[..real * channels]);
        let target = self.target_gains();
        if !self.started {
            self.gains = target;
            self.started = true;
        }
        let step = self.step;
        for frame in out[..real * channels].chunks_exact_mut(channels) {
            for (gain, goal) in self.gains.iter_mut().zip(target) {
                *gain += (goal - *gain).clamp(-step, step);
            }
            frame[0] *= self.gains[0];
            if channels > 1 {
                frame[1] *= self.gains[1];
            }
        }
        self.shared.played.fetch_add(real as u64, Ordering::Release);
        real
    }

    /// Moves whole frames from the ring into `out`, returning the frame count.
    fn drain(&mut self, out: &mut [f32]) -> usize {
        let channels = self.channels;
        let Ok(mut consumer) = self.consumer.try_lock() else {
            return 0;
        };
        let wanted = out.len() / channels * channels;
        let count = consumer.slots().min(wanted) / channels * channels;
        if count == 0 {
            return 0;
        }
        let Ok(chunk) = consumer.read_chunk(count) else {
            return 0;
        };
        let (first, second) = chunk.as_slices();
        out[..first.len()].copy_from_slice(first);
        out[first.len()..count].copy_from_slice(second);
        chunk.commit_all();
        count / channels
    }

    /// Writes a mono mix of the first two channels into the visualizer tap.
    fn tap(&self, samples: &[f32]) {
        let Ok(mut tap) = self.shared.tap.try_lock() else {
            return;
        };
        let channels = self.channels;
        let mut write = (tap.total % TAP_SIZE as u64) as usize;
        for frame in samples.chunks_exact(channels) {
            let mono = if channels > 1 {
                (frame[0] + frame[1]) * 0.5
            } else {
                frame[0]
            };
            tap.data[write] = mono;
            write = (write + 1) % TAP_SIZE;
        }
        tap.total += (samples.len() / channels) as u64;
    }
}
