//! Nastro's domain model, with no IO: tracks and their metadata, playlists (selection, sorting,
//! search), the play order with shuffle and repeat, M3U playlists as text, the play state, the
//! equalizer settings and presets, and time formatting. Plain data and pure functions, so
//! everything here is testable without files, devices or a window.

pub mod eq;
pub mod eq_presets;
pub mod m3u;
pub mod order;
pub mod play_state;
pub mod playlist;
pub mod time;
pub mod track;

mod text;

pub use eq::{EQ_BAND_COUNT, EQ_BANDS_HZ, EQ_GAIN_RANGE_DB, EqSettings};
pub use eq_presets::{EQ_PRESETS, EqPreset, eq_preset, preset_for_genre};
pub use m3u::{ParsedPlaylist, parse_m3u, write_m3u};
pub use order::PlayOrder;
pub use play_state::PlayState;
pub use playlist::{Playlist, SortKey};
pub use time::{format_clock, format_length, format_total};
pub use track::{StreamInfo, Track, TrackMeta};
