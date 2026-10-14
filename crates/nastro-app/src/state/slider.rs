//! The arithmetic of the skin's sliders: where the thumb sits for a value and which value a
//! pointer position means. Plain Rust, in logical pixels, so the views only forward positions.

use nastro_core::EQ_GAIN_RANGE_DB;

/// The fraction (0..=1) of a horizontal slider under the pointer. The thumb's centre follows
/// the pointer, so the usable length is the track minus the thumb.
pub fn fraction_at(pointer: f32, start: f32, length: f32, thumb: f32) -> f32 {
    let usable = length - thumb;
    if usable <= 0.0 {
        return 0.0;
    }
    ((pointer - start - thumb / 2.0) / usable).clamp(0.0, 1.0)
}

/// The volume (0..=1) for a slider fraction, in steps of 1 %.
pub fn volume_at(fraction: f32) -> f32 {
    (fraction.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

/// How close to the centre the balance snaps to exactly centred, as a fraction of its range.
const BALANCE_DETENT: f32 = 0.08;

/// The balance (-1..=1) for a slider fraction, snapping to the centre near it.
pub fn balance_at(fraction: f32) -> f32 {
    let balance = fraction.clamp(0.0, 1.0) * 2.0 - 1.0;
    if balance.abs() < BALANCE_DETENT {
        0.0
    } else {
        (balance * 100.0).round() / 100.0
    }
}

/// The slider fraction of a balance.
pub fn balance_fraction(balance: f32) -> f32 {
    (balance.clamp(-1.0, 1.0) + 1.0) / 2.0
}

/// The gain in whole dB for a vertical equalizer slider, `fraction` measured from the bottom.
pub fn gain_at(fraction: f32) -> f32 {
    let (low, high) = (*EQ_GAIN_RANGE_DB.start(), *EQ_GAIN_RANGE_DB.end());
    (low + fraction.clamp(0.0, 1.0) * (high - low)).round()
}

/// The fraction from the bottom of a vertical equalizer slider for `gain_db`.
pub fn gain_fraction(gain_db: f32) -> f32 {
    let (low, high) = (*EQ_GAIN_RANGE_DB.start(), *EQ_GAIN_RANGE_DB.end());
    ((gain_db - low) / (high - low)).clamp(0.0, 1.0)
}

/// The points of the equalizer curve in a box `width` x `height`: one point per band, evenly
/// spaced from edge to edge, the preamp added to each band and clamped to the gain range. 0 dB
/// sits on the middle line and the range uses `14 / 16` of the half height, like the design.
pub fn curve_points(preamp: f32, bands: &[f32], width: f32, height: f32) -> Vec<(f32, f32)> {
    let high = *EQ_GAIN_RANGE_DB.end();
    let middle = height / 2.0;
    let amplitude = middle * 14.0 / 16.0;
    let last = bands.len().saturating_sub(1).max(1) as f32;
    bands
        .iter()
        .enumerate()
        .map(|(index, gain)| {
            let total = (gain + preamp).clamp(-high, high);
            let x = index as f32 * width / last;
            (x, middle - total / high * amplitude)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn fraction_follows_the_thumb_centre() {
        assert_eq!(fraction_at(7.0, 0.0, 114.0, 14.0), 0.0);
        assert_eq!(fraction_at(107.0, 0.0, 114.0, 14.0), 1.0);
        assert_eq!(fraction_at(57.0, 0.0, 114.0, 14.0), 0.5);
        assert_eq!(fraction_at(-50.0, 0.0, 114.0, 14.0), 0.0);
        assert_eq!(fraction_at(5.0, 0.0, 10.0, 14.0), 0.0);
    }

    #[test]
    fn balance_snaps_to_the_centre() {
        assert_eq!(balance_at(0.52), 0.0);
        assert_eq!(balance_at(0.0), -1.0);
        assert_eq!(balance_at(1.0), 1.0);
        assert_eq!(balance_at(0.75), 0.5);
        assert_eq!(balance_fraction(0.0), 0.5);
    }

    #[test]
    fn gains_are_whole_db() {
        assert_eq!(gain_at(0.5), 0.0);
        assert_eq!(gain_at(1.0), 12.0);
        assert_eq!(gain_at(0.0), -12.0);
        assert_eq!(gain_at(0.52), 0.0);
        assert_eq!(gain_fraction(6.0), 0.75);
    }

    #[test]
    fn curve_spans_the_box_and_clamps() {
        let points = curve_points(6.0, &[0.0, 12.0, -12.0], 300.0, 32.0);
        assert_eq!(points[0], (0.0, 16.0 - 6.0 / 12.0 * 14.0));
        assert_eq!(points[1], (150.0, 2.0));
        assert_eq!(points[2].0, 300.0);
        assert_eq!(points[2].1, 16.0 + 6.0 / 12.0 * 14.0);
    }
}
