//! Spectrum and oscilloscope analysis for the visualizer.

use std::f32::consts::PI;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

/// Number of bars of the spectrum.
pub const SPECTRUM_BARS: usize = 24;

/// Number of points of the oscilloscope.
pub const SCOPE_POINTS: usize = 64;

/// Samples fed to each FFT.
const FFT_SIZE: usize = 2048;
/// Lowest and highest frequency shown by the bars, in Hz.
const MIN_HZ: f32 = 40.0;
const MAX_HZ: f32 = 16_000.0;
/// Range mapped to 0..1, in dB below full scale.
const RANGE_DB: f32 = 60.0;
/// Spectrum tilt in dB per octave above 1 kHz, to compensate the natural fall of music.
const TILT_DB_PER_OCTAVE: f32 = 3.0;
/// Fraction of the distance to a louder value covered per frame.
const ATTACK: f32 = 0.75;
/// Fraction of the distance to a quieter value covered per frame.
const DECAY: f32 = 0.18;
/// Frames a peak stays before it starts falling, and its gravity per frame.
const PEAK_HOLD_FRAMES: u8 = 8;
const PEAK_GRAVITY: f32 = 0.004;
/// Distance between scope points, in samples.
const SCOPE_STRIDE: usize = 16;

/// One frame for the visualizer, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisFrame {
    /// Level of each spectrum bar, `0..=1`.
    pub bars: [f32; SPECTRUM_BARS],
    /// Peak marker of each bar, `0..=1`, falling slowly.
    pub peaks: [f32; SPECTRUM_BARS],
    /// Recent waveform, `-1..=1`.
    pub scope: [f32; SCOPE_POINTS],
}

impl Default for AnalysisFrame {
    fn default() -> Self {
        AnalysisFrame {
            bars: [0.0; SPECTRUM_BARS],
            peaks: [0.0; SPECTRUM_BARS],
            scope: [0.0; SCOPE_POINTS],
        }
    }
}

/// Computes visualizer frames from the samples being played.
///
/// Feed it mono samples with [`push`](Analyzer::push) and ask for a frame about 30 times a
/// second with [`frame`](Analyzer::frame). The FFT plan and every buffer are created once.
pub struct Analyzer {
    sample_rate: u32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    /// The latest `FFT_SIZE` samples, as a circular buffer.
    history: Vec<f32>,
    write: usize,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    /// Bin range `[lo, hi)` of each bar.
    ranges: [(usize, usize); SPECTRUM_BARS],
    /// Tilt in dB of each bar.
    tilt: [f32; SPECTRUM_BARS],
    bars: [f32; SPECTRUM_BARS],
    peaks: [f32; SPECTRUM_BARS],
    hold: [u8; SPECTRUM_BARS],
    velocity: [f32; SPECTRUM_BARS],
}

impl std::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Analyzer")
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

impl Analyzer {
    /// An analyzer for samples at `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|n| 0.5 - 0.5 * (2.0 * PI * n as f32 / FFT_SIZE as f32).cos())
            .collect();
        let spectrum = fft.make_output_vec();
        let scratch = fft.make_scratch_vec();
        let resolution = sample_rate.max(1) as f32 / FFT_SIZE as f32;
        let max_bin = FFT_SIZE / 2;
        let ratio = (MAX_HZ / MIN_HZ).powf(1.0 / SPECTRUM_BARS as f32);
        let mut ranges = [(0, 1); SPECTRUM_BARS];
        let mut tilt = [0.0; SPECTRUM_BARS];
        for (bar, (range, tilt)) in ranges.iter_mut().zip(tilt.iter_mut()).enumerate() {
            let lo_hz = MIN_HZ * ratio.powi(bar as i32);
            let hi_hz = lo_hz * ratio;
            let lo = ((lo_hz / resolution).round() as usize).clamp(1, max_bin - 1);
            let hi = ((hi_hz / resolution).round() as usize).clamp(lo + 1, max_bin);
            *range = (lo, hi);
            *tilt = TILT_DB_PER_OCTAVE * ((lo_hz * hi_hz).sqrt() / 1_000.0).log2();
        }
        Analyzer {
            sample_rate,
            fft,
            window,
            history: vec![0.0; FFT_SIZE],
            write: 0,
            input: vec![0.0; FFT_SIZE],
            spectrum,
            scratch,
            ranges,
            tilt,
            bars: [0.0; SPECTRUM_BARS],
            peaks: [0.0; SPECTRUM_BARS],
            hold: [0; SPECTRUM_BARS],
            velocity: [0.0; SPECTRUM_BARS],
        }
    }

    /// The sample rate the analyzer was built for.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Appends mono samples to the history, dropping the oldest.
    pub fn push(&mut self, samples: &[f32]) {
        let tail = &samples[samples.len().saturating_sub(FFT_SIZE)..];
        for sample in tail {
            self.history[self.write] = *sample;
            self.write = (self.write + 1) % FFT_SIZE;
        }
    }

    /// Computes the next frame from the latest samples, with attack, decay and peak hold.
    pub fn frame(&mut self, out: &mut AnalysisFrame) {
        for n in 0..FFT_SIZE {
            self.input[n] = self.history[(self.write + n) % FFT_SIZE] * self.window[n];
        }
        let transformed =
            self.fft
                .process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch);
        if transformed.is_err() {
            self.spectrum.fill(Complex::new(0.0, 0.0));
        }
        // A full scale sine gives a magnitude of FFT_SIZE / 4 with a Hann window.
        let norm = 4.0 / FFT_SIZE as f32;
        for bar in 0..SPECTRUM_BARS {
            let (lo, hi) = self.ranges[bar];
            let peak = self.spectrum[lo..hi]
                .iter()
                .map(|c| c.norm())
                .fold(0.0f32, f32::max);
            let db = 20.0 * (peak * norm).max(1e-6).log10() + self.tilt[bar];
            let target = ((db + RANGE_DB) / RANGE_DB).clamp(0.0, 1.0);
            let level = &mut self.bars[bar];
            let rate = if target > *level { ATTACK } else { DECAY };
            *level += (target - *level) * rate;
            if *level >= self.peaks[bar] {
                self.peaks[bar] = *level;
                self.hold[bar] = PEAK_HOLD_FRAMES;
                self.velocity[bar] = 0.0;
            } else if self.hold[bar] > 0 {
                self.hold[bar] -= 1;
            } else {
                self.velocity[bar] += PEAK_GRAVITY;
                self.peaks[bar] = (self.peaks[bar] - self.velocity[bar]).max(*level);
            }
        }
        out.bars = self.bars;
        out.peaks = self.peaks;
        let newest = self.write + FFT_SIZE;
        for (n, point) in out.scope.iter_mut().enumerate() {
            let age = (SCOPE_POINTS - 1 - n) * SCOPE_STRIDE;
            *point = self.history[(newest - 1 - age) % FFT_SIZE].clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn sine(freq: f32, rate: u32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|n| 0.8 * (2.0 * PI * freq * n as f32 / rate as f32).sin())
            .collect()
    }

    fn loudest(bars: &[f32]) -> usize {
        let mut best = 0;
        for (i, v) in bars.iter().enumerate() {
            if *v > bars[best] {
                best = i;
            }
        }
        best
    }

    #[test]
    fn one_khz_sine_lands_in_its_bar() {
        for rate in [44_100, 48_000] {
            let mut analyzer = Analyzer::new(rate);
            let mut frame = AnalysisFrame::default();
            analyzer.push(&sine(1_000.0, rate, 4_096));
            for _ in 0..5 {
                analyzer.frame(&mut frame);
            }
            // Bar 12 spans about 800 to 1026 Hz.
            assert_eq!(loudest(&frame.bars), 12, "rate {rate}: {:?}", frame.bars);
            assert!(frame.bars[12] > 0.8);
            assert!(frame.bars[2] < 0.3 && frame.bars[22] < 0.3);
            assert!(frame.scope.iter().any(|v| v.abs() > 0.5));
        }
    }

    #[test]
    fn silence_decays_and_peaks_fall() {
        let mut analyzer = Analyzer::new(44_100);
        let mut frame = AnalysisFrame::default();
        analyzer.push(&sine(1_000.0, 44_100, 2_048));
        analyzer.frame(&mut frame);
        let loud = frame.bars[12];
        analyzer.push(&[0.0; 2_048]);
        for _ in 0..60 {
            analyzer.frame(&mut frame);
        }
        assert!(frame.bars[12] < loud * 0.1);
        assert!(frame.peaks[12] < loud);
        assert!(frame.peaks[12] >= frame.bars[12]);
    }
}
