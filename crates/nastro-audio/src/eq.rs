//! The ten band equalizer: peaking biquads from the RBJ Audio EQ Cookbook plus a preamp.

use std::f64::consts::PI;

use nastro_core::{EQ_BAND_COUNT, EQ_BANDS_HZ, EqSettings};

/// Quality factor of every band, about one octave wide.
const BAND_Q: f64 = 1.41;

/// Normalized coefficients of one biquad (`a0` is 1).
#[derive(Debug, Clone, Copy)]
struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coefficients {
    /// A filter that passes the signal untouched.
    const IDENTITY: Coefficients = Coefficients {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// A peaking filter at `freq_hz` with `gain_db`, for `sample_rate`.
    fn peaking(sample_rate: f64, freq_hz: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * BAND_Q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Coefficients {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }

    /// Magnitude of the response at `freq_hz`.
    fn magnitude(&self, sample_rate: f64, freq_hz: f64) -> f64 {
        let w = 2.0 * PI * freq_hz / sample_rate;
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = -(self.b1 * s1 + self.b2 * s2);
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = -(self.a1 * s1 + self.a2 * s2);
        ((num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im)).sqrt()
    }
}

/// The state of one biquad on one channel (transposed direct form II).
#[derive(Debug, Clone, Copy, Default)]
struct State {
    z1: f64,
    z2: f64,
}

/// Whether the settings change nothing, so the signal can pass bit for bit.
fn is_bypass(settings: &EqSettings) -> bool {
    !settings.enabled || settings.is_flat()
}

/// Linear gain of a preamp in dB.
fn preamp_gain(settings: &EqSettings) -> f64 {
    10f64.powf(f64::from(settings.preamp_db) / 20.0)
}

/// Whether a band centre is usable at `sample_rate` (below Nyquist with some margin).
fn band_usable(freq_hz: f32, sample_rate: u32) -> bool {
    f64::from(freq_hz) < f64::from(sample_rate) * 0.45
}

/// A ten band equalizer working in place on interleaved samples.
#[derive(Debug, Clone)]
pub struct Equalizer {
    sample_rate: u32,
    channels: usize,
    bypass: bool,
    preamp: f64,
    coefficients: [Coefficients; EQ_BAND_COUNT],
    /// `states[channel * EQ_BAND_COUNT + band]`.
    states: Vec<State>,
}

impl Equalizer {
    /// A flat, bypassed equalizer for a stream of `channels` at `sample_rate`.
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let channels = usize::from(channels.max(1));
        Equalizer {
            sample_rate,
            channels,
            bypass: true,
            preamp: 1.0,
            coefficients: [Coefficients::IDENTITY; EQ_BAND_COUNT],
            states: vec![State::default(); channels * EQ_BAND_COUNT],
        }
    }

    /// Applies new settings. Coefficients are recomputed but the filter state is kept, so
    /// changing a slider while playing does not click.
    pub fn set(&mut self, settings: &EqSettings) {
        let was_bypass = self.bypass;
        self.bypass = is_bypass(settings);
        self.preamp = preamp_gain(settings);
        for (band, coefficients) in self.coefficients.iter_mut().enumerate() {
            let freq = EQ_BANDS_HZ[band];
            *coefficients = if band_usable(freq, self.sample_rate) {
                Coefficients::peaking(
                    f64::from(self.sample_rate),
                    f64::from(freq),
                    f64::from(settings.bands_db[band]),
                )
            } else {
                Coefficients::IDENTITY
            };
        }
        if was_bypass && !self.bypass {
            self.reset();
        }
    }

    /// Clears the filter memory, for after a seek.
    pub fn reset(&mut self) {
        self.states.fill(State::default());
    }

    /// Filters interleaved `samples` in place. A flat or disabled equalizer leaves them
    /// bit-identical.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.bypass {
            return;
        }
        let preamp = self.preamp;
        for frame in samples.chunks_exact_mut(self.channels) {
            for (channel, sample) in frame.iter_mut().enumerate() {
                let states = &mut self.states[channel * EQ_BAND_COUNT..][..EQ_BAND_COUNT];
                let mut x = f64::from(*sample) * preamp;
                for (c, s) in self.coefficients.iter().zip(states.iter_mut()) {
                    let y = c.b0 * x + s.z1;
                    s.z1 = c.b1 * x - c.a1 * y + s.z2;
                    s.z2 = c.b2 * x - c.a2 * y;
                    x = y;
                }
                *sample = x as f32;
            }
        }
    }
}

/// Total magnitude response of `settings` at `frequency_hz` for a stream at `sample_rate`, in
/// dB, preamp included. Zero for a disabled equalizer.
pub fn response_db(settings: &EqSettings, frequency_hz: f32, sample_rate: u32) -> f32 {
    if !settings.enabled {
        return 0.0;
    }
    let rate = f64::from(sample_rate);
    let mut db = f64::from(settings.preamp_db);
    for (band, gain) in settings.bands_db.iter().enumerate() {
        let freq = EQ_BANDS_HZ[band];
        if band_usable(freq, sample_rate) {
            let c = Coefficients::peaking(rate, f64::from(freq), f64::from(*gain));
            db += 20.0 * c.magnitude(rate, f64::from(frequency_hz)).log10();
        }
    }
    db as f32
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const RATE: u32 = 44_100;

    fn sine(freq: f32, frames: usize, channels: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * channels);
        for n in 0..frames {
            let v = 0.5 * (2.0 * std::f32::consts::PI * freq * n as f32 / RATE as f32).sin();
            out.extend(std::iter::repeat_n(v, channels));
        }
        out
    }

    fn rms_tail(samples: &[f32]) -> f64 {
        let tail = &samples[samples.len() / 2..];
        (tail.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    }

    #[test]
    fn flat_is_bit_identical() {
        let mut eq = Equalizer::new(RATE, 2);
        eq.set(&EqSettings::default());
        let original = sine(1_000.0, 4_096, 2);
        let mut data = original.clone();
        eq.process(&mut data);
        assert_eq!(data, original);
    }

    #[test]
    fn disabled_is_bit_identical() {
        let mut eq = Equalizer::new(RATE, 2);
        let settings = EqSettings {
            enabled: false,
            bands_db: [12.0; EQ_BAND_COUNT],
            ..EqSettings::default()
        };
        eq.set(&settings);
        let original = sine(1_000.0, 4_096, 2);
        let mut data = original.clone();
        eq.process(&mut data);
        assert_eq!(data, original);
        assert_eq!(response_db(&settings, 1_000.0, RATE), 0.0);
    }

    #[test]
    fn band_centre_gets_its_gain() {
        for (band, freq) in EQ_BANDS_HZ.iter().enumerate() {
            let mut settings = EqSettings::default();
            settings.bands_db[band] = 12.0;
            let db = response_db(&settings, *freq, RATE);
            assert!((db - 12.0).abs() < 1.5, "band {band}: {db} dB");
        }
    }

    #[test]
    fn preamp_shifts_every_frequency() {
        let settings = EqSettings {
            preamp_db: -6.0,
            ..EqSettings::default()
        };
        for freq in [50.0, 1_000.0, 12_000.0] {
            let db = response_db(&settings, freq, RATE);
            assert!((db + 6.0).abs() < 0.01, "{freq} Hz: {db} dB");
        }
    }

    #[test]
    fn sine_through_a_boosted_band_measures_the_boost() {
        let mut settings = EqSettings::default();
        settings.bands_db[5] = 6.0;
        let mut eq = Equalizer::new(RATE, 1);
        eq.set(&settings);
        let input = sine(1_000.0, 44_100, 1);
        let mut data = input.clone();
        eq.process(&mut data);
        let gain_db = 20.0 * (rms_tail(&data) / rms_tail(&input)).log10();
        assert!((gain_db - 6.0).abs() < 0.3, "{gain_db} dB");
    }

    #[test]
    fn bands_above_nyquist_are_skipped() {
        let mut settings = EqSettings::default();
        settings.bands_db[9] = 12.0;
        let mut eq = Equalizer::new(22_050, 1);
        eq.set(&settings);
        let mut data = sine(1_000.0, 1_024, 1);
        eq.process(&mut data);
        assert!(data.iter().all(|s| s.is_finite()));
        assert!(response_db(&settings, 8_000.0, 22_050).abs() < 0.5);
    }
}
