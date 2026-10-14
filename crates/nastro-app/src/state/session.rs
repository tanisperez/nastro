//! What Nastro remembers between runs besides the settings: volume and balance, shuffle and
//! repeat, the equalizer, where the panels are, which playlists exist and which track was
//! loaded. Saved to `session.toml` in the state folder (`~/.local/state/nastro/` on Linux), with
//! each playlist in its own `.m3u8` file in the data folder (`~/.local/share/nastro/playlists/`
//! on Linux), so playlists can be opened by any other player.
//!
//! Like the settings, every field is read independently and falls back to its default, so a
//! damaged or older file never loses everything.

use std::fs;
use std::path::{Path, PathBuf};

use nastro_core::{EQ_BAND_COUNT, EqSettings, Playlist, parse_m3u, write_m3u};
use nastro_dock::{Layout, LayoutPreset, PanelId, PanelState, Rect};

/// The session file name inside [`state_dir`].
const SESSION_FILE: &str = "session.toml";
/// The playlists folder name inside [`data_dir`].
const PLAYLISTS_DIR: &str = "playlists";

/// The folder of the session file: `$XDG_STATE_HOME/nastro` on Linux, the local data folder
/// elsewhere (macOS and Windows have no separate state folder).
pub fn state_dir() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .map(|base| base.join("nastro"))
}

/// The folder of Nastro's user data: `$XDG_DATA_HOME/nastro` on Linux,
/// `~/Library/Application Support/nastro` on macOS, `%APPDATA%\nastro` on Windows.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|base| base.join("nastro"))
}

/// The folder of the saved playlists.
pub fn playlists_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(PLAYLISTS_DIR))
}

/// The folder of the user's own skins: `<config dir>/nastro/skins`.
pub fn user_skins_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join("nastro").join("skins"))
}

/// The remembered state of the player.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// Volume slider position, 0..=1.
    pub volume: f32,
    /// Balance, -1..=1.
    pub balance: f32,
    /// Shuffle on or off.
    pub shuffle: bool,
    /// Repeat on or off.
    pub repeat: bool,
    /// The equalizer.
    pub eq: EqSettings,
    /// Whether the equalizer picks a preset from each track's genre.
    pub eq_auto: bool,
    /// The id of the last preset applied, shown in the presets menu.
    pub eq_preset: Option<String>,
    /// Where the panels are, in 1x units. `None` means the default layout.
    pub layout: Option<Layout>,
    /// The playlist files, in tab order (file names inside [`playlists_dir`]).
    pub playlists: Vec<String>,
    /// The tab shown.
    pub active_playlist: usize,
    /// The playlist of the loaded track.
    pub playing_playlist: Option<usize>,
    /// The loaded track, an index in that playlist.
    pub current_track: Option<usize>,
    /// Where the window's top left corner was, in screen coordinates. Never known on Wayland,
    /// where the compositor places windows.
    pub window_origin: Option<(f32, f32)>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            volume: 0.8,
            balance: 0.0,
            shuffle: false,
            repeat: false,
            eq: EqSettings::default(),
            eq_auto: false,
            eq_preset: None,
            layout: None,
            playlists: Vec::new(),
            active_playlist: 0,
            playing_playlist: None,
            current_track: None,
            window_origin: None,
        }
    }
}

/// Loads the session, the defaults when there is none.
pub fn load_session() -> Session {
    state_dir()
        .and_then(|dir| fs::read_to_string(dir.join(SESSION_FILE)).ok())
        .map(|text| parse_session(&text))
        .unwrap_or_default()
}

/// Saves the session. A failure is only logged.
pub fn save_session(session: &Session) {
    let Some(dir) = state_dir() else {
        return;
    };
    write_file(&dir.join(SESSION_FILE), &serialize_session(session));
}

/// Reads a saved playlist: its name from `#PLAYLIST`, or the file stem.
pub fn load_playlist(file: &str) -> Option<Playlist> {
    let path = playlists_dir()?.join(file);
    let text = fs::read_to_string(&path).ok()?;
    Some(playlist_from_text(
        file,
        &text,
        path.parent().unwrap_or(Path::new("")),
    ))
}

/// Saves `playlist` to `file` in the playlists folder.
pub fn save_playlist(file: &str, playlist: &Playlist) {
    let Some(dir) = playlists_dir() else {
        return;
    };
    write_file(
        &dir.join(file),
        &write_m3u(Some(playlist.name()), playlist.tracks()),
    );
}

/// Deletes a saved playlist file.
pub fn delete_playlist(file: &str) {
    let Some(dir) = playlists_dir() else {
        return;
    };
    let path = dir.join(file);
    if let Err(error) = fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("could not delete {}: {error}", path.display());
    }
}

/// A file name for a new playlist, `playlist-<n>.m3u8` with the first free `n` among `taken`.
pub fn new_playlist_file(taken: &[String]) -> String {
    (1..)
        .map(|n| format!("playlist-{n}.m3u8"))
        .find(|name| !taken.contains(name))
        .unwrap_or_else(|| "playlist.m3u8".to_string())
}

/// The playlist of a `.m3u8` text read from `file`.
fn playlist_from_text(file: &str, text: &str, base: &Path) -> Playlist {
    let parsed = parse_m3u(text, base);
    let name = parsed.name.unwrap_or_else(|| {
        Path::new(file)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let mut playlist = Playlist::new(name);
    playlist.add(parsed.tracks);
    playlist
}

/// Writes `text` to `path`, creating its folder. Failures are logged.
fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        log::warn!("could not create {}: {error}", parent.display());
        return;
    }
    if let Err(error) = fs::write(path, text) {
        log::warn!("could not save {}: {error}", path.display());
    }
}

/// The key of a panel in the session file.
fn panel_key(id: PanelId) -> &'static str {
    match id {
        PanelId::Player => "player",
        PanelId::Equalizer => "equalizer",
        PanelId::Playlist => "playlist",
    }
}

/// A number of a TOML table as `f32`, integers included.
fn number(table: &toml::Table, key: &str) -> Option<f32> {
    let value = table.get(key)?;
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
        .map(|n| n as f32)
        .filter(|n| n.is_finite())
}

/// An index of a TOML table.
fn index(table: &toml::Table, key: &str) -> Option<usize> {
    table
        .get(key)?
        .as_integer()
        .and_then(|n| usize::try_from(n).ok())
}

/// A boolean of a TOML table.
fn flag(table: &toml::Table, key: &str) -> Option<bool> {
    table.get(key)?.as_bool()
}

/// Parses the session file, field by field.
fn parse_session(text: &str) -> Session {
    let defaults = Session::default();
    let Ok(table) = text.parse::<toml::Table>() else {
        return defaults;
    };
    let mut session = Session {
        volume: number(&table, "volume").map_or(defaults.volume, |v| v.clamp(0.0, 1.0)),
        balance: number(&table, "balance").map_or(defaults.balance, |b| b.clamp(-1.0, 1.0)),
        shuffle: flag(&table, "shuffle").unwrap_or(defaults.shuffle),
        repeat: flag(&table, "repeat").unwrap_or(defaults.repeat),
        ..defaults
    };

    if let Some(eq) = table.get("equalizer").and_then(toml::Value::as_table) {
        session.eq.enabled = flag(eq, "enabled").unwrap_or(true);
        session.eq.preamp_db = number(eq, "preamp").unwrap_or(0.0);
        if let Some(bands) = eq.get("bands").and_then(toml::Value::as_array) {
            for (slot, value) in session.eq.bands_db.iter_mut().zip(bands) {
                *slot = value
                    .as_float()
                    .or_else(|| value.as_integer().map(|n| n as f64))
                    .unwrap_or(0.0) as f32;
            }
        }
        session.eq = session.eq.clamped();
        session.eq_auto = flag(eq, "auto").unwrap_or(false);
        session.eq_preset = eq
            .get("preset")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
    }

    if let Some(panels) = table.get("panels").and_then(toml::Value::as_table) {
        session.layout = parse_layout(panels);
    }

    if let Some(lists) = table.get("playlists").and_then(toml::Value::as_table) {
        session.playlists = lists
            .get("files")
            .and_then(toml::Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .filter(|file| is_plain_file_name(file))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        session.active_playlist = index(lists, "active").unwrap_or(0);
        session.playing_playlist = index(lists, "playing");
        session.current_track = index(lists, "current");
    }

    if let Some(window) = table.get("window").and_then(toml::Value::as_table) {
        session.window_origin = number(window, "x").zip(number(window, "y"));
    }
    session
}

/// Whether `file` is a bare file name, so a session file can never point outside the playlists
/// folder.
fn is_plain_file_name(file: &str) -> bool {
    !file.is_empty()
        && Path::new(file).file_name().is_some_and(|name| name == file)
        && file != "."
        && file != ".."
}

/// The layout of the `[panels]` table, `None` when any panel is missing or malformed.
fn parse_layout(panels: &toml::Table) -> Option<Layout> {
    let mut states = Vec::with_capacity(3);
    for id in PanelId::ALL {
        let panel = panels.get(panel_key(id))?.as_table()?;
        let rect = Rect::new(
            number(panel, "x")?.max(0.0),
            number(panel, "y")?.max(0.0),
            number(panel, "w")?,
            number(panel, "h")?,
        );
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return None;
        }
        states.push(PanelState {
            rect,
            visible: flag(panel, "visible").unwrap_or(true),
            shaded: flag(panel, "shaded").unwrap_or(false),
        });
    }
    let states: [PanelState; 3] = states.try_into().ok()?;
    Some(Layout::from_states(states))
}

/// The text of the session file.
fn serialize_session(session: &Session) -> String {
    let mut table = toml::Table::new();
    table.insert("volume".into(), f64::from(session.volume).into());
    table.insert("balance".into(), f64::from(session.balance).into());
    table.insert("shuffle".into(), session.shuffle.into());
    table.insert("repeat".into(), session.repeat.into());

    let mut eq = toml::Table::new();
    eq.insert("enabled".into(), session.eq.enabled.into());
    eq.insert("auto".into(), session.eq_auto.into());
    if let Some(preset) = &session.eq_preset {
        eq.insert("preset".into(), preset.clone().into());
    }
    eq.insert("preamp".into(), f64::from(session.eq.preamp_db).into());
    let bands: Vec<toml::Value> = session.eq.bands_db[..EQ_BAND_COUNT]
        .iter()
        .map(|gain| f64::from(*gain).into())
        .collect();
    eq.insert("bands".into(), bands.into());
    table.insert("equalizer".into(), eq.into());

    if let Some(layout) = &session.layout {
        let mut panels = toml::Table::new();
        for (id, state) in layout.states() {
            let mut panel = toml::Table::new();
            panel.insert("x".into(), f64::from(state.rect.x).into());
            panel.insert("y".into(), f64::from(state.rect.y).into());
            panel.insert("w".into(), f64::from(state.rect.w).into());
            panel.insert("h".into(), f64::from(state.rect.h).into());
            panel.insert("visible".into(), state.visible.into());
            panel.insert("shaded".into(), state.shaded.into());
            panels.insert(panel_key(id).into(), panel.into());
        }
        table.insert("panels".into(), panels.into());
    }

    let mut lists = toml::Table::new();
    let files: Vec<toml::Value> = session.playlists.iter().map(|f| f.clone().into()).collect();
    lists.insert("files".into(), files.into());
    lists.insert("active".into(), (session.active_playlist as i64).into());
    if let Some(playing) = session.playing_playlist {
        lists.insert("playing".into(), (playing as i64).into());
    }
    if let Some(current) = session.current_track {
        lists.insert("current".into(), (current as i64).into());
    }
    table.insert("playlists".into(), lists.into());

    if let Some((x, y)) = session.window_origin {
        let mut window = toml::Table::new();
        window.insert("x".into(), f64::from(x).into());
        window.insert("y".into(), f64::from(y).into());
        table.insert("window".into(), window.into());
    }
    toml::to_string(&table).unwrap_or_default()
}

/// The default layout, used on the first run and by "Reset layout".
pub fn default_layout(metrics: &nastro_dock::Metrics) -> Layout {
    Layout::preset(LayoutPreset::default(), metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_session_round_trips() {
        let metrics = nastro_dock::Metrics::default();
        let mut layout = default_layout(&metrics);
        layout.set_shaded(PanelId::Equalizer, true, &metrics);
        let mut eq = EqSettings::default();
        eq.bands_db[3] = 5.0;
        eq.preamp_db = -2.0;
        let session = Session {
            volume: 0.5,
            balance: -0.25,
            shuffle: true,
            repeat: true,
            eq,
            eq_auto: true,
            eq_preset: Some("rock".to_string()),
            layout: Some(layout),
            playlists: vec!["playlist-1.m3u8".to_string(), "playlist-2.m3u8".to_string()],
            active_playlist: 1,
            playing_playlist: Some(0),
            current_track: Some(4),
            window_origin: Some((120.0, 80.0)),
        };
        assert_eq!(parse_session(&serialize_session(&session)), session);
    }

    #[test]
    fn bad_values_fall_back_one_by_one() {
        let session = parse_session(
            "volume = 7\nbalance = \"left\"\n[playlists]\nfiles = [\"../evil.m3u8\", \"ok.m3u8\"]\n",
        );
        assert_eq!(session.volume, 1.0);
        assert_eq!(session.balance, 0.0);
        assert_eq!(session.playlists, vec!["ok.m3u8".to_string()]);
        assert_eq!(session.layout, None);
    }

    #[test]
    fn a_broken_file_gives_the_defaults() {
        assert_eq!(parse_session("[[[["), Session::default());
    }

    #[test]
    fn new_playlist_files_take_the_first_free_number() {
        let taken = vec!["playlist-1.m3u8".to_string(), "playlist-3.m3u8".to_string()];
        assert_eq!(new_playlist_file(&taken), "playlist-2.m3u8");
        assert_eq!(new_playlist_file(&[]), "playlist-1.m3u8");
    }

    #[test]
    fn playlists_take_their_name_from_the_file() {
        let playlist = playlist_from_text(
            "playlist-1.m3u8",
            "#EXTM3U\n#PLAYLIST:Focus\n/a.mp3\n",
            Path::new("/"),
        );
        assert_eq!(playlist.name(), "Focus");
        assert_eq!(playlist.len(), 1);
        let unnamed = playlist_from_text("Road trip.m3u8", "/a.mp3\n", Path::new("/"));
        assert_eq!(unnamed.name(), "Road trip");
    }
}
