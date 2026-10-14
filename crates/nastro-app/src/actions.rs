//! The actions of the app: what menus and keyboard shortcuts trigger. `gpui`'s `actions!` macro
//! turns each name into a unit struct implementing `gpui::Action`; the root view handles them
//! with `on_action` and `keymap.rs` binds them to keys.

use gpui_kit::actions;

actions!(
    nastro,
    [
        /// Starts or resumes playback.
        Play,
        /// Pauses, or resumes when paused.
        Pause,
        /// Plays when paused or stopped, pauses when playing.
        PlayPause,
        /// Stops and goes back to the start of the track.
        Stop,
        /// Plays the previous track.
        Previous,
        /// Plays the next track.
        Next,
        /// Jumps 5 seconds forward.
        SeekForward,
        /// Jumps 5 seconds back.
        SeekBackward,
        /// Raises the volume a step.
        VolumeUp,
        /// Lowers the volume a step.
        VolumeDown,
        /// Turns shuffle on or off.
        ToggleShuffle,
        /// Turns repeat on or off.
        ToggleRepeat,
        /// Adds files to the playlist shown and plays the first one.
        OpenFiles,
        /// Adds files to the playlist shown.
        AddFiles,
        /// Adds a folder to the playlist shown.
        AddFolder,
        /// Adds a new, empty playlist tab.
        NewPlaylist,
        /// Closes the playlist tab shown, deleting the playlist.
        ClosePlaylist,
        /// Renames the playlist tab shown.
        RenamePlaylist,
        /// Saves a copy of the playlist shown as an M3U file.
        ExportPlaylist,
        /// Plays the first selected track.
        PlaySelected,
        /// Removes the selected tracks from the playlist.
        RemoveSelected,
        /// Keeps only the selected tracks.
        CropToSelection,
        /// Removes every track of the playlist shown.
        ClearPlaylist,
        /// Selects every track.
        SelectAll,
        /// Clears the selection.
        SelectNone,
        /// Inverts the selection.
        InvertSelection,
        /// Moves the selection one row up.
        SelectPrevious,
        /// Moves the selection one row down.
        SelectNext,
        /// Moves the selected tracks one row up.
        MoveSelectedUp,
        /// Moves the selected tracks one row down.
        MoveSelectedDown,
        /// Sorts the playlist by title.
        SortByTitle,
        /// Sorts the playlist by artist.
        SortByArtist,
        /// Sorts the playlist by album.
        SortByAlbum,
        /// Sorts the playlist by file name.
        SortByFileName,
        /// Sorts the playlist by full path.
        SortByPath,
        /// Sorts the playlist by length.
        SortByLength,
        /// Reverses the playlist.
        ReversePlaylist,
        /// Shuffles the playlist order once.
        RandomizePlaylist,
        /// Puts the keyboard focus in the playlist filter.
        FocusFilter,
        /// Shows or hides the equalizer.
        ToggleEqualizerPanel,
        /// Shows or hides the playlist.
        TogglePlaylistPanel,
        /// Collapses or expands the player.
        ShadePlayer,
        /// Collapses or expands the equalizer.
        ShadeEqualizer,
        /// Collapses or expands the playlist.
        ShadePlaylist,
        /// Arranges the panels in a column.
        LayoutA,
        /// Arranges the player and equalizer in a row, the playlist below.
        LayoutB,
        /// Stacks the player and equalizer, the playlist at their right (the default).
        LayoutC,
        /// Arranges every panel in one row.
        LayoutD,
        /// Draws the skin at its design size.
        SkinScale1,
        /// Draws the skin at one and a half times its size.
        SkinScale15,
        /// Draws the skin at twice its size.
        SkinScale2,
        /// Switches the clock between elapsed and remaining time.
        ToggleTimeDisplay,
        /// Switches the visualizer between spectrum, oscilloscope and off.
        NextVisualizer,
        /// Turns the equalizer on or off.
        ToggleEqualizer,
        /// Turns the automatic equalizer preset on or off.
        ToggleEqualizerAuto,
        /// Sets every equalizer band and the preamp to 0 dB.
        ResetEqualizer,
        /// Opens Preferences.
        OpenPreferences,
        /// Opens the About window.
        OpenAbout,
        /// Opens the user guide in the browser.
        OpenUserGuide,
        /// Opens the folder of the user's own skins.
        OpenSkinsFolder,
        /// Minimizes the window.
        Minimize,
        /// Quits Nastro.
        Quit,
    ]
);
