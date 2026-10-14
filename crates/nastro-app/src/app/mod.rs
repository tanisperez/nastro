//! The app's models, as gpui entities, and the commands that involve several of them. Views
//! render these and forward input; the rules live in `nastro-core`, `nastro-dock` and
//! `crate::state`.

pub mod commands;
pub mod equalizer;
pub mod library;
pub mod persist;
pub mod shell;
pub mod transport;

use gpui_kit::{App, Entity, Global};

use equalizer::EqModel;
use library::Library;
use shell::Shell;
use transport::Transport;

/// Every model entity, a gpui global so views and commands reach them from any `cx`.
pub struct Models {
    /// The audio engine and what it reports.
    pub transport: Entity<Transport>,
    /// The playlists.
    pub library: Entity<Library>,
    /// The equalizer settings.
    pub eq: Entity<EqModel>,
    /// Settings, skins and panel layout.
    pub shell: Entity<Shell>,
}

impl Global for Models {}

/// The models. `main` sets them before any window opens.
pub fn models(cx: &App) -> &Models {
    cx.global::<Models>()
}
