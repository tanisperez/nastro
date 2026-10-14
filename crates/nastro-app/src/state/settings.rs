//! User settings, persisted to `<config dir>/nastro/settings.toml` (`~/.config/nastro/` on Linux)
//! and applied live: there is no Save button, every change made in Preferences or through a
//! menu writes the file immediately.
//!
//! Settings are what the user chooses once (language, skin, scale, how the clock reads). What
//! changes while listening (volume, equalizer, playlists, panel layout) is session state, saved
//! by `state::session`.

use std::fs;
use std::path::{Path, PathBuf};

use rust_i18n::t;

use super::locale::LanguageChoice;

/// The path, relative to the OS config directory, of the settings file.
const SETTINGS_FILE: &str = "nastro/settings.toml";

/// Light or dark for the app chrome (menus, Preferences, About). The skin has its own colors and
/// does not follow this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Follow the OS light or dark setting.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

impl ThemeChoice {
    /// Every choice, in the order Preferences lists them.
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    /// Parses a value as read from the settings file, case-insensitive.
    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "system" => Some(ThemeChoice::System),
            "light" => Some(ThemeChoice::Light),
            "dark" => Some(ThemeChoice::Dark),
            _ => None,
        }
    }

    /// The value written to the settings file.
    fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    /// The label shown in Preferences.
    pub fn label(self) -> String {
        match self {
            ThemeChoice::System => t!("settings.theme.system"),
            ThemeChoice::Light => t!("settings.theme.light"),
            ThemeChoice::Dark => t!("settings.theme.dark"),
        }
        .into_owned()
    }
}

/// How big the skin is drawn, independent of the OS scale factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkinScale {
    /// The design size.
    #[default]
    X1,
    /// One and a half times the design size.
    X1_5,
    /// Twice the design size.
    X2,
}

impl SkinScale {
    /// Every scale, in the order the menus list them.
    pub const ALL: [SkinScale; 3] = [SkinScale::X1, SkinScale::X1_5, SkinScale::X2];

    /// The multiplier applied to every skin metric.
    pub fn factor(self) -> f32 {
        match self {
            SkinScale::X1 => 1.0,
            SkinScale::X1_5 => 1.5,
            SkinScale::X2 => 2.0,
        }
    }

    /// The scale stored as `factor` in the settings file, or `None` for anything other than 1,
    /// 1.5 or 2.
    fn from_factor(factor: f64) -> Option<Self> {
        SkinScale::ALL
            .into_iter()
            .find(|scale| (f64::from(scale.factor()) - factor).abs() < 0.01)
    }

    /// The label shown in menus and Preferences: `1x`, `1.5x`, `2x`. Not translated, like other
    /// units, except for the decimal separator.
    pub fn label(self) -> String {
        match self {
            SkinScale::X1 => "1x".to_string(),
            SkinScale::X1_5 => format!("{}x", super::number::format_decimal(1.5, 1)),
            SkinScale::X2 => "2x".to_string(),
        }
    }
}

/// What the big clock of the player shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeDisplay {
    /// The time played so far.
    #[default]
    Elapsed,
    /// The time left, with a minus sign.
    Remaining,
}

impl TimeDisplay {
    /// Parses a value as read from the settings file.
    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "elapsed" => Some(TimeDisplay::Elapsed),
            "remaining" => Some(TimeDisplay::Remaining),
            _ => None,
        }
    }

    /// The value written to the settings file.
    fn as_str(self) -> &'static str {
        match self {
            TimeDisplay::Elapsed => "elapsed",
            TimeDisplay::Remaining => "remaining",
        }
    }

    /// The other mode, for a click on the clock.
    pub fn toggled(self) -> Self {
        match self {
            TimeDisplay::Elapsed => TimeDisplay::Remaining,
            TimeDisplay::Remaining => TimeDisplay::Elapsed,
        }
    }
}

/// What the visualizer of the player's display draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizerMode {
    /// Spectrum bars with falling peaks.
    #[default]
    Spectrum,
    /// The waveform.
    Oscilloscope,
    /// Nothing.
    Off,
}

impl VisualizerMode {
    /// Every mode, in the order a click on the visualizer cycles through them.
    pub const ALL: [VisualizerMode; 3] = [
        VisualizerMode::Spectrum,
        VisualizerMode::Oscilloscope,
        VisualizerMode::Off,
    ];

    /// Parses a value as read from the settings file.
    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "spectrum" => Some(VisualizerMode::Spectrum),
            "oscilloscope" => Some(VisualizerMode::Oscilloscope),
            "off" => Some(VisualizerMode::Off),
            _ => None,
        }
    }

    /// The value written to the settings file.
    fn as_str(self) -> &'static str {
        match self {
            VisualizerMode::Spectrum => "spectrum",
            VisualizerMode::Oscilloscope => "oscilloscope",
            VisualizerMode::Off => "off",
        }
    }

    /// The next mode, for a click on the visualizer.
    pub fn next(self) -> Self {
        match self {
            VisualizerMode::Spectrum => VisualizerMode::Oscilloscope,
            VisualizerMode::Oscilloscope => VisualizerMode::Off,
            VisualizerMode::Off => VisualizerMode::Spectrum,
        }
    }

    /// The label shown in menus.
    pub fn label(self) -> String {
        match self {
            VisualizerMode::Spectrum => t!("menu.options.visualizer_spectrum"),
            VisualizerMode::Oscilloscope => t!("menu.options.visualizer_oscilloscope"),
            VisualizerMode::Off => t!("menu.options.visualizer_off"),
        }
        .into_owned()
    }
}

/// How much Nastro writes to its log file, chosen in Preferences, "Log level". Stored as
/// `log_level = "off" | "error" | "warn" | "info" | "debug" | "trace"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Nothing is logged.
    Off,
    /// Only errors.
    Error,
    /// Errors and warnings.
    Warn,
    /// Startup, tracks played, settings changes, plus warnings and errors.
    #[default]
    Info,
    /// Also state changes useful to diagnose a problem.
    Debug,
    /// Everything Nastro and gpui log, including every render. Verbose.
    Trace,
}

impl LogLevel {
    /// Every level, in the order Preferences lists them.
    pub const ALL: [LogLevel; 6] = [
        LogLevel::Off,
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    /// Parses a level as read from the settings file or the `NASTRO_LOG` environment variable,
    /// case-insensitive.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "off" => Some(LogLevel::Off),
            "error" => Some(LogLevel::Error),
            "warn" | "warning" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }

    /// The value written to the settings file.
    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    /// The label shown in Preferences.
    pub fn label(self) -> String {
        match self {
            LogLevel::Off => t!("settings.log_level.off"),
            LogLevel::Error => t!("settings.log_level.error"),
            LogLevel::Warn => t!("settings.log_level.warn"),
            LogLevel::Info => t!("settings.log_level.info"),
            LogLevel::Debug => t!("settings.log_level.debug"),
            LogLevel::Trace => t!("settings.log_level.trace"),
        }
        .into_owned()
    }

    /// The matching `log` crate filter.
    pub fn to_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

/// Everything the user can choose in Preferences and in the Options menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The UI language.
    pub language: LanguageChoice,
    /// Light or dark for menus and dialogs.
    pub theme: ThemeChoice,
    /// The id of the active skin: a built-in (`silver`, `graphite`, `porcelain`, `nebula`) or a
    /// user skin. An unknown id falls back to the default skin at startup.
    pub skin: String,
    /// How big the skin is drawn.
    pub skin_scale: SkinScale,
    /// What the big clock shows.
    pub time_display: TimeDisplay,
    /// What the visualizer draws.
    pub visualizer: VisualizerMode,
    /// How much goes to the log file.
    pub log_level: LogLevel,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: LanguageChoice::default(),
            theme: ThemeChoice::default(),
            skin: nastro_skin::DEFAULT_SKIN_ID.to_string(),
            skin_scale: SkinScale::default(),
            time_display: TimeDisplay::default(),
            visualizer: VisualizerMode::default(),
            log_level: LogLevel::default(),
        }
    }
}

/// Loads the settings from `<config dir>/nastro/settings.toml`, falling back to the defaults for
/// a missing file and, field by field, for missing or invalid values.
pub fn load_settings() -> Settings {
    match dirs::config_dir() {
        Some(base) => read_settings(&base),
        None => Settings::default(),
    }
}

/// Saves `settings`, creating the folder if needed. A failure is only logged: losing a setting
/// is not worth interrupting the music for.
pub fn save_settings(settings: &Settings) {
    if let Some(base) = dirs::config_dir() {
        write_settings(&base, settings);
    }
}

/// The path of the settings file, `None` when the OS config directory is unknown.
pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join(SETTINGS_FILE))
}

/// Reads `<base>/nastro/settings.toml`. Split out so tests can use a temporary directory.
fn read_settings(base: &Path) -> Settings {
    match fs::read_to_string(base.join(SETTINGS_FILE)) {
        Ok(content) => parse_settings(&content),
        Err(_) => Settings::default(),
    }
}

/// Writes `<base>/nastro/settings.toml`.
fn write_settings(base: &Path, settings: &Settings) {
    let path = base.join(SETTINGS_FILE);
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        log::warn!("could not create {}: {error}", parent.display());
        return;
    }
    if let Err(error) = fs::write(&path, serialize_settings(settings)) {
        log::warn!("could not save {}: {error}", path.display());
    }
}

/// Parses the settings file. Every field is read independently: a field of the wrong type or an
/// invalid value falls back to its own default, and an unknown key is ignored, so a file written
/// by another version never loses the rest of the settings.
fn parse_settings(content: &str) -> Settings {
    let defaults = Settings::default();
    let Ok(value) = content.parse::<toml::Table>() else {
        return defaults;
    };
    let text = |key: &str| value.get(key).and_then(toml::Value::as_str);
    let skin_scale = value
        .get("skin_scale")
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_integer().map(|n| n as f64))
        })
        .and_then(SkinScale::from_factor);
    Settings {
        language: text("language")
            .and_then(LanguageChoice::parse)
            .unwrap_or(defaults.language),
        theme: text("theme")
            .and_then(ThemeChoice::parse)
            .unwrap_or(defaults.theme),
        skin: text("skin")
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or(defaults.skin),
        skin_scale: skin_scale.unwrap_or(defaults.skin_scale),
        time_display: text("time_display")
            .and_then(TimeDisplay::parse)
            .unwrap_or(defaults.time_display),
        visualizer: text("visualizer")
            .and_then(VisualizerMode::parse)
            .unwrap_or(defaults.visualizer),
        log_level: text("log_level")
            .and_then(LogLevel::parse)
            .unwrap_or(defaults.log_level),
    }
}

/// The text of the settings file, every key always written so users can find them.
fn serialize_settings(settings: &Settings) -> String {
    let mut table = toml::Table::new();
    let language = format!("{:?}", settings.language).to_ascii_lowercase();
    table.insert("language".into(), language.into());
    table.insert("theme".into(), settings.theme.as_str().into());
    table.insert("skin".into(), settings.skin.clone().into());
    table.insert(
        "skin_scale".into(),
        f64::from(settings.skin_scale.factor()).into(),
    );
    table.insert("time_display".into(), settings.time_display.as_str().into());
    table.insert("visualizer".into(), settings.visualizer.as_str().into());
    table.insert("log_level".into(), settings.log_level.as_str().into());
    toml::to_string(&table).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let settings = Settings {
            language: LanguageChoice::Gl,
            theme: ThemeChoice::Dark,
            skin: "nebula".to_string(),
            skin_scale: SkinScale::X1_5,
            time_display: TimeDisplay::Remaining,
            visualizer: VisualizerMode::Oscilloscope,
            log_level: LogLevel::Debug,
        };
        write_settings(dir.path(), &settings);
        assert_eq!(read_settings(dir.path()), settings);
    }

    #[test]
    fn invalid_values_fall_back_one_by_one() {
        let settings = parse_settings(
            "language = \"fr\"\ntheme = 3\nskin = \"\"\nskin_scale = 3.0\n\
             time_display = \"remaining\"\nunknown = true\n",
        );
        let defaults = Settings::default();
        assert_eq!(settings.language, defaults.language);
        assert_eq!(settings.theme, defaults.theme);
        assert_eq!(settings.skin, defaults.skin);
        assert_eq!(settings.skin_scale, defaults.skin_scale);
        assert_eq!(settings.time_display, TimeDisplay::Remaining);
    }

    #[test]
    fn skin_scale_accepts_integers() {
        assert_eq!(parse_settings("skin_scale = 2").skin_scale, SkinScale::X2);
        assert_eq!(
            parse_settings("skin_scale = 1.5").skin_scale,
            SkinScale::X1_5
        );
    }

    #[test]
    fn a_broken_file_gives_the_defaults() {
        assert_eq!(parse_settings("this is [not toml"), Settings::default());
    }

    #[test]
    fn visualizer_cycles_through_every_mode() {
        let mut mode = VisualizerMode::Spectrum;
        for expected in [
            VisualizerMode::Oscilloscope,
            VisualizerMode::Off,
            VisualizerMode::Spectrum,
        ] {
            mode = mode.next();
            assert_eq!(mode, expected);
        }
    }
}
