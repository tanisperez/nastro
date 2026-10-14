//! Formatting of playing times for the big clock, the playlist rows and the playlist summary.
//! Every function truncates to whole seconds, it never rounds up.

use std::time::Duration;

/// The big clock, "mm:ss". Minutes have at least two digits and keep growing past 99, so a
/// 123 minute track at 45 s reads "123:45".
pub fn format_clock(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// A track length in the playlist, "m:ss", or "h:mm:ss" from one hour.
pub fn format_length(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 3_600 {
        format_total(duration)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

/// The playlist summary, always "h:mm:ss".
pub fn format_total(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}:{:02}", secs / 3_600, (secs / 60) % 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn clock_pads_minutes_and_grows_past_99() {
        assert_eq!(format_clock(secs(0)), "00:00");
        assert_eq!(format_clock(secs(65)), "01:05");
        assert_eq!(format_clock(secs(99 * 60 + 59)), "99:59");
        assert_eq!(format_clock(secs(123 * 60 + 45)), "123:45");
    }

    #[test]
    fn length_switches_to_hours() {
        assert_eq!(format_length(secs(5)), "0:05");
        assert_eq!(format_length(secs(3_599)), "59:59");
        assert_eq!(format_length(secs(3_600)), "1:00:00");
        assert_eq!(format_length(secs(3_725)), "1:02:05");
    }

    #[test]
    fn total_is_always_hours() {
        assert_eq!(format_total(secs(0)), "0:00:00");
        assert_eq!(format_total(secs(61)), "0:01:01");
        assert_eq!(format_total(secs(10 * 3_600 + 59)), "10:00:59");
    }

    #[test]
    fn fractions_are_truncated() {
        let almost = Duration::from_millis(59_999);
        assert_eq!(format_clock(almost), "00:59");
        assert_eq!(format_length(almost), "0:59");
        assert_eq!(format_total(almost), "0:00:59");
    }
}
