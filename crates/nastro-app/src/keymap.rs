//! The keyboard shortcuts. Single keys (the transport on Z, X, C, V, B, the arrows) only fire
//! while no text field has the focus, so typing in the playlist filter never pauses the music;
//! that is the `!Input` part of their context (`Input` is gpui-kit's text field context).
//! The list is also the source of the user guide's keyboard shortcuts page: update both.

use gpui_kit::{App, KeyBinding};

use crate::actions::*;

/// The key context of the main window's root view.
pub const WINDOW_CONTEXT: &str = "Nastro";
/// The key context of the track list.
pub const TRACK_LIST_CONTEXT: &str = "TrackList";

/// Registers every key binding.
pub fn bind_keys(cx: &mut App) {
    let single = Some("Nastro && !Input");
    let list = Some("TrackList && !Input");
    cx.bind_keys([
        KeyBinding::new("z", Previous, single),
        KeyBinding::new("x", Play, single),
        KeyBinding::new("c", Pause, single),
        KeyBinding::new("v", Stop, single),
        KeyBinding::new("b", Next, single),
        KeyBinding::new("space", PlayPause, single),
        KeyBinding::new("left", SeekBackward, single),
        KeyBinding::new("right", SeekForward, single),
        KeyBinding::new("up", VolumeUp, single),
        KeyBinding::new("down", VolumeDown, single),
        KeyBinding::new("s", ToggleShuffle, single),
        KeyBinding::new("r", ToggleRepeat, single),
        KeyBinding::new("l", OpenFiles, single),
        KeyBinding::new("t", ToggleTimeDisplay, single),
        KeyBinding::new("delete", RemoveSelected, single),
        KeyBinding::new("enter", PlaySelected, single),
        KeyBinding::new("up", SelectPrevious, list),
        KeyBinding::new("down", SelectNext, list),
        KeyBinding::new("alt-up", MoveSelectedUp, list),
        KeyBinding::new("alt-down", MoveSelectedDown, list),
        KeyBinding::new("alt-g", ToggleEqualizerPanel, None),
        KeyBinding::new("alt-e", TogglePlaylistPanel, None),
        KeyBinding::new("alt-w", ShadePlayer, None),
        KeyBinding::new("f1", OpenUserGuide, None),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFiles, None),
        KeyBinding::new("cmd-shift-o", AddFolder, None),
        KeyBinding::new("cmd-n", NewPlaylist, None),
        KeyBinding::new("cmd-f", FocusFilter, None),
        KeyBinding::new("cmd-a", SelectAll, single),
        KeyBinding::new("cmd-,", OpenPreferences, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenFiles, None),
        KeyBinding::new("ctrl-shift-o", AddFolder, None),
        KeyBinding::new("ctrl-n", NewPlaylist, None),
        KeyBinding::new("ctrl-f", FocusFilter, None),
        KeyBinding::new("ctrl-a", SelectAll, single),
        KeyBinding::new("ctrl-p", OpenPreferences, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}
