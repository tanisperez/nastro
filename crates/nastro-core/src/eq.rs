//! Equalizer settings: ten bands plus a preamp, in dB. The filters themselves live in
//! `nastro-audio`; this is the model the UI edits and the audio thread reads.

use std::ops::RangeInclusive;

/// Number of equalizer bands.
pub const EQ_BAND_COUNT: usize = 10;

/// Centre frequency of each band, in Hz, from left to right as the sliders show them.
pub const EQ_BANDS_HZ: [f32; EQ_BAND_COUNT] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

/// Range of a band gain and of the preamp, in dB.
pub const EQ_GAIN_RANGE_DB: RangeInclusive<f32> = -12.0..=12.0;

/// The equalizer as the user set it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqSettings {
    /// Whether the equalizer is applied. Off means the signal passes untouched.
    pub enabled: bool,
    /// Gain applied before the bands, in dB, within [`EQ_GAIN_RANGE_DB`].
    pub preamp_db: f32,
    /// Gain of each band, in dB, within [`EQ_GAIN_RANGE_DB`], in the order of [`EQ_BANDS_HZ`].
    pub bands_db: [f32; EQ_BAND_COUNT],
}

impl Default for EqSettings {
    /// On and flat, the classic default.
    fn default() -> Self {
        EqSettings {
            enabled: true,
            preamp_db: 0.0,
            bands_db: [0.0; EQ_BAND_COUNT],
        }
    }
}

impl EqSettings {
    /// Whether every band and the preamp are at 0 dB, so the equalizer would change nothing.
    pub fn is_flat(&self) -> bool {
        self.preamp_db == 0.0 && self.bands_db.iter().all(|gain| *gain == 0.0)
    }

    /// A copy with every gain clamped to [`EQ_GAIN_RANGE_DB`], for values read from a file.
    pub fn clamped(mut self) -> Self {
        let clamp = |gain: f32| {
            if gain.is_finite() {
                gain.clamp(*EQ_GAIN_RANGE_DB.start(), *EQ_GAIN_RANGE_DB.end())
            } else {
                0.0
            }
        };
        self.preamp_db = clamp(self.preamp_db);
        for gain in &mut self.bands_db {
            *gain = clamp(*gain);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_on_and_flat() {
        let eq = EqSettings::default();
        assert!(eq.enabled);
        assert!(eq.is_flat());
    }

    #[test]
    fn clamped_limits_gains_and_drops_nan() {
        let mut eq = EqSettings::default();
        eq.preamp_db = 30.0;
        eq.bands_db[0] = -40.0;
        eq.bands_db[1] = f32::NAN;
        let eq = eq.clamped();
        assert_eq!(eq.preamp_db, 12.0);
        assert_eq!(eq.bands_db[0], -12.0);
        assert_eq!(eq.bands_db[1], 0.0);
    }
}
