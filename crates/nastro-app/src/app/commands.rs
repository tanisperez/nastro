//! Commands that involve several models: playing a track, moving through the playlist, keeping
//! the next track queued in the engine for gapless playback, and opening files. Menus, buttons,
//! shortcuts and engine events all end up here.

use std::path::PathBuf;

use gpui_kit::{App, AsyncApp};
use nastro_audio::{Decoder, SymphoniaDecoder};
use nastro_core::{PlayState, Track, TrackMeta};

use crate::app::models;
use crate::state::files;

/// How many files are probed before their tags are shown, so a big folder fills the playlist in
/// steps instead of repainting once per file.
const PROBE_BATCH: usize = 24;

/// Loads track `index` of tab `tab` and plays it.
pub fn play_track(tab: usize, index: usize, cx: &mut App) {
    let models = models(cx);
    let (library, transport) = (models.library.clone(), models.transport.clone());
    let path = library.update(cx, |library, cx| {
        let path = library.tabs.get(tab)?.playlist.track(index)?.path.clone();
        library.set_current(tab, Some(index));
        library.queued = None;
        cx.notify();
        Some(path)
    });
    if let Some(path) = path {
        transport.update(cx, |transport, cx| transport.load(path, true, cx));
    }
}

/// The Play button: resumes when paused, restarts the track when playing, and when nothing is
/// loaded plays the loaded track of the session, the first selected track or the first track.
pub fn play(cx: &mut App) {
    let models = models(cx);
    let transport = models.transport.clone();
    let status = transport.read(cx).status.clone();
    match status.state {
        PlayState::Paused => transport.read(cx).player.play(),
        PlayState::Playing => transport.read(cx).player.seek(std::time::Duration::ZERO),
        PlayState::Stopped if status.path.is_some() => transport.read(cx).player.play(),
        PlayState::Stopped => play_something(cx),
    }
}

/// Plays the loaded track of the session, else the first selected track of the tab shown, else
/// its first track.
fn play_something(cx: &mut App) {
    let library = models(cx).library.read(cx);
    if let Some((tab, index, _)) = library.current() {
        play_track(tab, index, cx);
        return;
    }
    let tab = library.active;
    let playlist = &library.active_tab().playlist;
    let index = playlist
        .selected_indices()
        .first()
        .copied()
        .or((!playlist.is_empty()).then_some(0));
    if let Some(index) = index {
        play_track(tab, index, cx);
    }
}

/// The Pause button: pauses, or resumes when paused.
pub fn pause(cx: &mut App) {
    let transport = models(cx).transport.read(cx);
    match transport.status.state {
        PlayState::Playing => transport.player.pause(),
        PlayState::Paused => transport.player.play(),
        PlayState::Stopped => {}
    }
}

/// Play when paused or stopped, pause when playing.
pub fn play_pause(cx: &mut App) {
    if models(cx).transport.read(cx).is_playing() {
        pause(cx);
    } else {
        play(cx);
    }
}

/// The Stop button.
pub fn stop(cx: &mut App) {
    models(cx).transport.read(cx).player.stop();
}

/// The Next button: the next track in the play order, wrapping at the end.
pub fn next(cx: &mut App) {
    step(cx, true);
}

/// The Previous button.
pub fn previous(cx: &mut App) {
    step(cx, false);
}

/// Moves one track forward or back in the play order of the playing tab, or of the tab shown
/// when nothing is loaded.
fn step(cx: &mut App, forward: bool) {
    let library = models(cx).library.clone();
    let target = library.update(cx, |library, _| {
        let (tab, current) = match library.current() {
            Some((tab, index, _)) => (tab, Some(index)),
            None => (library.active, None),
        };
        let len = library.tabs.get(tab)?.playlist.len();
        let index = if forward {
            library.order.next_manual(len, current)
        } else {
            library.order.previous(len, current)
        }?;
        Some((tab, index))
    });
    if let Some((tab, index)) = target {
        play_track(tab, index, cx);
    }
}

/// Plays the first selected track of the tab shown.
pub fn play_selected(cx: &mut App) {
    let library = models(cx).library.read(cx);
    let tab = library.active;
    if let Some(index) = library
        .active_tab()
        .playlist
        .selected_indices()
        .first()
        .copied()
    {
        play_track(tab, index, cx);
    }
}

/// Turns shuffle on or off.
pub fn toggle_shuffle(cx: &mut App) {
    let library = models(cx).library.clone();
    library.update(cx, |library, cx| {
        let on = !library.order.shuffle();
        library.order.set_shuffle(on);
        cx.notify();
    });
    requeue(cx);
    crate::app::persist::schedule(cx);
}

/// Turns repeat on or off.
pub fn toggle_repeat(cx: &mut App) {
    let library = models(cx).library.clone();
    library.update(cx, |library, cx| {
        let on = !library.order.repeat();
        library.order.set_repeat(on);
        cx.notify();
    });
    requeue(cx);
    crate::app::persist::schedule(cx);
}

/// Hands the engine the track that follows the loaded one, so it starts without a gap, or
/// nothing when the playlist ends there. Called when a track starts and whenever the playlist,
/// shuffle or repeat change.
pub fn requeue(cx: &mut App) {
    let models = models(cx);
    let (library, transport) = (models.library.clone(), models.transport.clone());
    if transport.read(cx).status.path.is_none() {
        return;
    }
    let next = library.update(cx, |library, _| {
        let (tab, index, _) = library.current()?;
        let len = library.tabs[tab].playlist.len();
        // A copy of the order, so peeking does not move a shuffle forward.
        let next = library.order.clone().next_auto(len, Some(index))?;
        let path = library.tabs[tab].playlist.track(next)?.path.clone();
        library.queued = Some((tab, next));
        Some(path)
    });
    if next.is_none() {
        library.update(cx, |library, _| library.queued = None);
    }
    transport.read(cx).player.queue_next(next);
}

/// A track started playing: pick the automatic equalizer preset and queue the next one.
pub fn on_track_started(path: PathBuf, cx: &mut App) {
    let models = models(cx);
    let (library, eq) = (models.library.clone(), models.eq.clone());
    let track = library
        .read(cx)
        .current()
        .map(|(_, _, track)| track.clone())
        .filter(|track| track.path == path);
    if let Some(track) = track
        && eq.read(cx).auto
    {
        match track.meta.genre.clone() {
            Some(genre) => eq.update(cx, |eq, cx| eq.track_started(Some(&genre), cx)),
            None => probe_for_genre(track.path.clone(), cx),
        }
    }
    requeue(cx);
}

/// The engine moved to the queued track without a gap: mark it as the loaded one.
pub fn on_track_advanced(to: PathBuf, cx: &mut App) {
    let library = models(cx).library.clone();
    library.update(cx, |library, cx| {
        let target = library.queued.take().filter(|(tab, index)| {
            library
                .tabs
                .get(*tab)
                .and_then(|tab| tab.playlist.track(*index))
                .map(|t| &t.path)
                == Some(&to)
        });
        let target = target.or_else(|| {
            let tab = library.playing?;
            let index = library.tabs[tab]
                .playlist
                .tracks()
                .iter()
                .position(|t| t.path == to)?;
            Some((tab, index))
        });
        if let Some((tab, index)) = target {
            // Move the real order forward too, so a shuffle continues from here.
            let len = library.tabs[tab].playlist.len();
            let current = library.tabs[tab].playlist.current();
            let _ = library.order.next_auto(len, current);
            library.set_current(tab, Some(index));
        }
        cx.notify();
    });
    crate::app::persist::schedule(cx);
}

/// The engine reached the end of a track with nothing queued: play the next one in the order,
/// or stay stopped at the end of the playlist.
pub fn on_playlist_track_finished(cx: &mut App) {
    let library = models(cx).library.clone();
    let next = library.update(cx, |library, _| {
        let (tab, index, _) = library.current()?;
        let len = library.tabs[tab].playlist.len();
        Some((tab, library.order.next_auto(len, Some(index))?))
    });
    if let Some((tab, index)) = next {
        play_track(tab, index, cx);
    }
}

/// The loaded track could not be played: skip to the next one, unless every track of the
/// playlist failed in a row.
pub fn on_track_failed(cx: &mut App) {
    let models = models(cx);
    let failures = models.transport.read(cx).failures_in_a_row;
    let len = models
        .library
        .read(cx)
        .current()
        .map(|(tab, _, _)| models.library.read(cx).tabs[tab].playlist.len())
        .unwrap_or(0);
    if failures < len {
        on_playlist_track_finished(cx);
    }
}

/// Adds `paths` (files, folders, playlists) to the tab shown, reading the folders in the
/// background, then reads the tags of the new tracks. With `play`, the first new track plays.
pub fn open_paths(paths: Vec<PathBuf>, play: bool, cx: &mut App) {
    if paths.is_empty() {
        return;
    }
    let collect = cx
        .background_executor()
        .spawn(async move { files::collect_tracks(&paths) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let tracks = collect.await;
        if tracks.is_empty() {
            return;
        }
        let to_probe = cx.update(|cx| add_tracks(tracks, play, cx));
        probe_tracks(to_probe, cx).await;
    })
    .detach();
}

/// Appends `tracks` to the tab shown and returns the paths whose tags are still unknown.
fn add_tracks(tracks: Vec<Track>, play: bool, cx: &mut App) -> Vec<PathBuf> {
    let library = models(cx).library.clone();
    let to_probe: Vec<PathBuf> = tracks
        .iter()
        .filter(|track| track.meta.duration.is_none() || track.meta.title.is_none())
        .map(|track| track.path.clone())
        .collect();
    let count = tracks.len();
    let (tab, first) = library.update(cx, |library, cx| {
        let first = library.active_tab().playlist.len();
        library.edit_active(cx, |playlist| playlist.add(tracks));
        (library.active, first)
    });
    log::info!("added {count} tracks");
    if play {
        play_track(tab, first, cx);
    }
    to_probe
}

/// Reads the tags of `paths` in the background, a batch at a time, and fills them in.
async fn probe_tracks(paths: Vec<PathBuf>, cx: &mut AsyncApp) {
    for batch in paths.chunks(PROBE_BATCH) {
        let batch = batch.to_vec();
        let results = cx
            .background_executor()
            .spawn(async move { probe_all(&batch) })
            .await;
        cx.update(|cx| {
            let library = models(cx).library.clone();
            library.update(cx, |library, cx| library.update_meta(&results, cx));
        });
    }
}

/// The tags of every readable file of `paths`.
fn probe_all(paths: &[PathBuf]) -> Vec<(PathBuf, TrackMeta)> {
    let decoder = SymphoniaDecoder;
    paths
        .iter()
        .filter_map(|path| match decoder.probe(path) {
            Ok(probe) => Some((path.clone(), probe.meta)),
            Err(error) => {
                log::debug!("could not read the tags of {}: {error}", path.display());
                None
            }
        })
        .collect()
}

/// Reads the genre of `path` in the background for the automatic equalizer preset.
fn probe_for_genre(path: PathBuf, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let probe_path = path.clone();
        let results = cx
            .background_executor()
            .spawn(async move { probe_all(&[probe_path]) })
            .await;
        cx.update(|cx| {
            let models = models(cx);
            let (library, eq) = (models.library.clone(), models.eq.clone());
            let genre = results.first().and_then(|(_, meta)| meta.genre.clone());
            library.update(cx, |library, cx| library.update_meta(&results, cx));
            eq.update(cx, |eq, cx| eq.track_started(genre.as_deref(), cx));
        });
    })
    .detach();
}
