//! The texts of the player's display: the clock, the stream badges and the marquee line. Plain
//! Rust so the formatting rules are unit tested; the view only lays them out.

use std::time::Duration;

use nastro_core::{StreamInfo, format_clock, format_length};
use rust_i18n::t;

use super::number;
use super::settings::TimeDisplay;

/// What the big clock reads: the elapsed time, or the time left with a minus sign when the
/// length is known.
pub fn clock_text(position: Duration, length: Option<Duration>, mode: TimeDisplay) -> String {
    match (mode, length) {
        (TimeDisplay::Remaining, Some(length)) => {
            format!("-{}", format_clock(length.saturating_sub(position)))
        }
        _ => format_clock(position),
    }
}

/// The badges of a stream, left to right: codec, bitrate, sample rate, channels. The bitrate is
/// left out when unknown.
pub fn badges(info: &StreamInfo) -> Vec<String> {
    let mut badges = vec![info.codec.clone()];
    if let Some(kbps) = info.bitrate_kbps {
        badges.push(format!("{} kbps", number::format_integer(u64::from(kbps))));
    }
    badges.push(sample_rate_text(info.sample_rate));
    badges.push(channels_text(info.channels));
    badges
}

/// A sample rate in kHz with at most two decimals and no trailing zeros: `44.1 kHz`, `48 kHz`,
/// `22.05 kHz`, with the decimal separator of the UI language.
pub fn sample_rate_text(hz: u32) -> String {
    let khz = f64::from(hz) / 1000.0;
    let decimals = if hz.is_multiple_of(1000) {
        0
    } else if hz.is_multiple_of(100) {
        1
    } else {
        2
    };
    format!("{} kHz", number::format_decimal(khz, decimals))
}

/// The channels badge: `MONO`, `STEREO`, or the count followed by `CH`.
pub fn channels_text(channels: u16) -> String {
    match channels {
        1 => t!("lcd.mono").into_owned(),
        2 => t!("lcd.stereo").into_owned(),
        n => format!("{n} CH"),
    }
}

/// The marquee line of the playing track: `8. Artist - Title (3:47)`, the number being its
/// position in the playlist, counted from 1.
pub fn marquee_text(index: usize, label: &str, length: Option<Duration>) -> String {
    match length {
        Some(length) => format!("{}. {label} ({})", index + 1, format_length(length)),
        None => format!("{}. {label}", index + 1),
    }
}

/// The text placed between two copies of the marquee line, so the loop reads continuously.
pub const MARQUEE_SEPARATOR: &str = "     ***     ";

/// How far the marquee has scrolled, in logical pixels, for a loop `loop_width` wide (one copy of
/// the line plus the separator). The offset follows the playing position, so the marquee only
/// moves while the music plays and freezes on pause, with no clock of its own.
pub fn marquee_offset(position: Duration, speed_px_per_s: f32, loop_width: f32) -> f32 {
    if loop_width <= 0.0 {
        return 0.0;
    }
    (position.as_secs_f32() * speed_px_per_s) % loop_width
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn info(codec: &str, kbps: Option<u32>, rate: u32, channels: u16) -> StreamInfo {
        StreamInfo {
            codec: codec.to_string(),
            bitrate_kbps: kbps,
            sample_rate: rate,
            channels,
        }
    }

    #[test]
    fn clock_shows_elapsed_or_remaining() {
        let position = Duration::from_secs(72);
        let length = Some(Duration::from_secs(227));
        assert_eq!(clock_text(position, length, TimeDisplay::Elapsed), "01:12");
        assert_eq!(
            clock_text(position, length, TimeDisplay::Remaining),
            "-02:35"
        );
        assert_eq!(clock_text(position, None, TimeDisplay::Remaining), "01:12");
    }

    #[test]
    fn sample_rates_drop_trailing_zeros() {
        // English separators: the tests never change the process locale.
        assert_eq!(sample_rate_text(48_000), "48 kHz");
        assert_eq!(sample_rate_text(44_100), "44.1 kHz");
        assert_eq!(sample_rate_text(22_050), "22.05 kHz");
    }

    #[test]
    fn badges_skip_an_unknown_bitrate() {
        assert_eq!(
            badges(&info("FLAC", Some(912), 44_100, 2)),
            vec!["FLAC", "912 kbps", "44.1 kHz", "STEREO"]
        );
        assert_eq!(
            badges(&info("PCM", None, 48_000, 1)),
            vec!["PCM", "48 kHz", "MONO"]
        );
        assert_eq!(channels_text(6), "6 CH");
    }

    #[test]
    fn marquee_numbers_from_one() {
        assert_eq!(
            marquee_text(
                7,
                "Edvard Grieg - Morning Mood",
                Some(Duration::from_secs(227))
            ),
            "8. Edvard Grieg - Morning Mood (3:47)"
        );
        assert_eq!(marquee_text(0, "Intro", None), "1. Intro");
    }

    #[test]
    fn marquee_offset_loops() {
        assert_eq!(marquee_offset(Duration::from_secs(2), 30.0, 100.0), 60.0);
        assert_eq!(marquee_offset(Duration::from_secs(4), 30.0, 100.0), 20.0);
        assert_eq!(marquee_offset(Duration::from_secs(4), 30.0, 0.0), 0.0);
    }
}
