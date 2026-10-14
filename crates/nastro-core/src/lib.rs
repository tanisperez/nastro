//! Nastro's domain model: tracks and their metadata, the play state and the equalizer settings.
//! Plain data and pure functions, no IO: everything here is testable without files, devices or
//! a window.

pub mod eq;
pub mod play_state;
pub mod track;

pub use eq::{EQ_BAND_COUNT, EQ_BANDS_HZ, EQ_GAIN_RANGE_DB, EqSettings};
pub use play_state::PlayState;
pub use track::{StreamInfo, Track, TrackMeta};
