//! Nastro, the skinnable desktop music player. Binary entry point: logging, settings and
//! language first, then the session (playlists, equalizer, panel layout), the models, and the
//! transparent main window that holds the docked panels. Files and folders given on the command
//! line are added to the playlist shown and the first one plays.

// A GUI application on Windows: never open a console window behind the main window.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod actions;
mod app;
mod assets;
mod keymap;
mod logging;
mod state;
mod theme;
mod views;

rust_i18n::i18n!("locales", fallback = "en");

use std::path::{Path, PathBuf};

use gpui_kit::{
    AppContext as _, Bounds, Size, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowKind, WindowOptions,
};
use nastro_core::Playlist;

use app::Models;
use app::equalizer::EqModel;
use app::library::Library;
use app::shell::{Shell, find_skin, load_skins};
use app::transport::Transport;
use state::session::{self, Session};
use theme::SkinTheme;
use views::window::NastroWindow;

/// The application id: Wayland `app_id`, X11 `WM_CLASS`, Windows AppUserModelID. The Linux
/// `codes.tanis.nastro.desktop` file and the Windows installer shortcut use the same value.
pub const APP_ID: &str = "codes.tanis.nastro";

/// The user-visible application name given to the OS together with [`APP_ID`].
const APP_NAME: &str = "Nastro";

fn main() {
    // Settings first, for the log level: the logger is installed before anything else runs.
    let settings = state::settings::load_settings();
    let env_level = std::env::var(logging::ENV_VAR).ok();
    let log_level = logging::startup_level(settings.log_level, env_level.as_deref());
    let log_path = logging::init(log_level.to_filter());
    log_startup(log_level, log_path.as_deref());

    // Before the window opens, so its first frame is already in the right language.
    let language = state::locale::resolve(settings.language);
    state::locale::apply(language);
    log::info!("language: {}", language.code());

    let session = session::load_session();
    let tabs = load_playlists(&session);
    let arguments: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let application = gpui_kit::application().with_assets(assets::Assets);
    application.run(move |cx| {
        cx.set_app_identity(APP_ID, APP_NAME);
        gpui_kit::init(cx);
        keymap::bind_keys(cx);

        let skins = load_skins();
        let skin = find_skin(&skins, &settings.skin);
        log::info!("skin {} at {}x", skin.id, settings.skin_scale.factor());
        theme::install(cx, SkinTheme::new(&skin, settings.skin_scale.factor()));

        let models = build_models(cx, settings, session, tabs, skins);
        cx.set_global(models);

        // Quitting from the OS (the dock, a session logout) saves like Quit does.
        cx.on_app_quit(|cx| {
            app::persist::save_all(cx);
            async {}
        })
        .detach();
        // Closing the main window from the OS quits, like the close button.
        cx.on_window_closed(|cx, _| {
            let main_open = cx
                .windows()
                .iter()
                .any(|window| window.downcast::<NastroWindow>().is_some());
            if !main_open {
                views::quit(cx);
            }
        })
        .detach();

        open_main_window(cx);

        if !arguments.is_empty() {
            app::commands::open_paths(arguments, true, cx);
        }
    });
}

/// The playlists of the session, in tab order. Missing files are skipped.
fn load_playlists(session: &Session) -> Vec<(String, Playlist)> {
    session
        .playlists
        .iter()
        .filter_map(|file| match session::load_playlist(file) {
            Some(playlist) => Some((file.clone(), playlist)),
            None => {
                log::warn!("playlist {file} not found, skipped");
                None
            }
        })
        .collect()
}

/// Creates the model entities from the saved settings and session.
fn build_models(
    cx: &mut gpui_kit::App,
    settings: state::settings::Settings,
    session: Session,
    tabs: Vec<(String, Playlist)>,
    skins: Vec<nastro_skin::Skin>,
) -> Models {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(1);
    let mut library = Library::new(tabs, session.active_playlist, seed);
    library.order.set_shuffle(session.shuffle);
    library.order.set_repeat(session.repeat);
    if let (Some(tab), Some(index)) = (session.playing_playlist, session.current_track)
        && library
            .tabs
            .get(tab)
            .is_some_and(|t| index < t.playlist.len())
    {
        library.set_current(tab, Some(index));
    }
    let eq = session.eq;
    Models {
        transport: cx.new(|cx| Transport::new(session.volume, session.balance, eq, cx)),
        library: cx.new(|_| library),
        eq: cx.new(|_| EqModel::new(eq, session.eq_auto, session.eq_preset.clone())),
        shell: cx.new(|_| Shell::new(settings, session.layout, skins)),
    }
}

/// Opens the transparent main window, sized to the docked panels and centred on the screen.
fn open_main_window(cx: &mut gpui_kit::App) {
    let size: Size<_> = app::models(cx).shell.read(cx).window_size();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
        // The panels draw their own title bars: no OS title bar and no frame.
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::Normal,
        is_resizable: false,
        app_owns_titlebar_drag: true,
        app_id: Some(APP_ID.to_string()),
        focus: true,
        show: true,
        ..WindowOptions::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        window.set_window_title(APP_NAME);
        views::apply_chrome_theme_to(window, cx);
        cx.new(|cx| NastroWindow::new(window, cx))
    });
    // Without its main window the app has nothing to do: one of AGENTS.md's "truly impossible
    // state" cases for an `expect`.
    #[allow(clippy::expect_used)]
    opened.expect("failed to open the main window");
}

/// The first lines of every session: what runs where, the log level and the files used.
fn log_startup(level: state::settings::LogLevel, log_path: Option<&Path>) {
    log::info!(
        "Nastro {} starting ({} build) on {} {}",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    log::info!(
        "log level {level:?}, log file {}",
        log_path.map_or_else(|| "none".to_string(), |path| path.display().to_string()),
    );
    log::info!(
        "settings file {}",
        state::settings::settings_path()
            .map_or_else(|| "unknown".to_string(), |path| path.display().to_string()),
    );
}
