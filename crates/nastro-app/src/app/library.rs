//! The playlists: the tabs of the playlist panel, which one is shown, which one holds the
//! loaded track, the play order (shuffle and repeat) and each tab's filter. A gpui entity; the
//! model itself is `nastro_core::Playlist`.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{Context, Task};
use nastro_core::{PlayOrder, Playlist, Track, TrackMeta};
use rust_i18n::t;

use crate::state::session;

/// How long after the last change a playlist is written to disk.
const SAVE_DELAY: Duration = Duration::from_secs(2);

/// One tab of the playlist panel.
pub struct PlaylistTab {
    /// Its file name in the playlists folder.
    pub file: String,
    /// The tracks.
    pub playlist: Playlist,
    /// The filter typed for this tab.
    pub filter: String,
    /// The rows shown for [`Self::filter`], cached under the playlist generation.
    rows: Rc<Vec<usize>>,
    /// The playlist generation and filter the cached rows were computed for.
    rows_key: Option<(u64, String)>,
}

impl PlaylistTab {
    fn new(file: String, playlist: Playlist) -> Self {
        PlaylistTab {
            file,
            playlist,
            filter: String::new(),
            rows: Rc::new(Vec::new()),
            rows_key: None,
        }
    }

    /// The playlist indices shown with the current filter, recomputed only when the playlist or
    /// the filter changed.
    pub fn rows(&mut self) -> Rc<Vec<usize>> {
        let key = (self.playlist.generation(), self.filter.clone());
        if self.rows_key.as_ref() != Some(&key) {
            self.rows = Rc::new(self.playlist.filter(&self.filter));
            self.rows_key = Some(key);
        }
        self.rows.clone()
    }
}

/// The playlists entity.
pub struct Library {
    /// The tabs, in order. Never empty.
    pub tabs: Vec<PlaylistTab>,
    /// The tab shown.
    pub active: usize,
    /// The tab that holds the loaded track, if any.
    pub playing: Option<usize>,
    /// Shuffle, repeat and the shuffled order.
    pub order: PlayOrder,
    /// The track handed to the engine for gapless playback after the current one.
    pub queued: Option<(usize, usize)>,
    /// Tabs with changes not written to disk yet, by file name.
    unsaved: BTreeSet<String>,
    /// The pending save.
    save_task: Option<Task<()>>,
}

impl Library {
    /// The library of `tabs` (file name and playlist), with at least one tab: an empty one named
    /// after the first playlist when `tabs` is empty.
    pub fn new(tabs: Vec<(String, Playlist)>, active: usize, seed: u64) -> Self {
        let mut tabs: Vec<PlaylistTab> = tabs
            .into_iter()
            .map(|(file, playlist)| PlaylistTab::new(file, playlist))
            .collect();
        if tabs.is_empty() {
            tabs.push(PlaylistTab::new(
                session::new_playlist_file(&[]),
                Playlist::new(default_name(1)),
            ));
        }
        let active = active.min(tabs.len() - 1);
        Library {
            tabs,
            active,
            playing: None,
            order: PlayOrder::new(seed),
            queued: None,
            unsaved: BTreeSet::new(),
            save_task: None,
        }
    }

    /// The tab shown.
    pub fn active_tab(&self) -> &PlaylistTab {
        &self.tabs[self.active]
    }

    /// The tab shown, mutable.
    pub fn active_tab_mut(&mut self) -> &mut PlaylistTab {
        &mut self.tabs[self.active]
    }

    /// The loaded track: its tab, index and the track itself.
    pub fn current(&self) -> Option<(usize, usize, &Track)> {
        let tab = self.playing?;
        let playlist = &self.tabs.get(tab)?.playlist;
        let index = playlist.current()?;
        Some((tab, index, playlist.track(index)?))
    }

    /// Makes `index` of tab `tab` the loaded track, clearing the mark in the previous tab.
    pub fn set_current(&mut self, tab: usize, index: Option<usize>) {
        if let Some(previous) = self.playing
            && previous != tab
            && let Some(old) = self.tabs.get_mut(previous)
        {
            old.playlist.set_current(None);
        }
        if let Some(target) = self.tabs.get_mut(tab) {
            target.playlist.set_current(index);
            self.playing = index.map(|_| tab);
        }
    }

    /// Shows tab `index`.
    pub fn activate(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() && index != self.active {
            self.active = index;
            cx.notify();
        }
    }

    /// Adds an empty tab after the others and shows it.
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let taken: Vec<String> = self.tabs.iter().map(|tab| tab.file.clone()).collect();
        let file = session::new_playlist_file(&taken);
        let name = default_name(self.tabs.len() + 1);
        self.tabs.push(PlaylistTab::new(file, Playlist::new(name)));
        self.active = self.tabs.len() - 1;
        self.mark_unsaved(self.active, cx);
        cx.notify();
    }

    /// Closes the tab shown and deletes its file. The last tab is emptied instead of closed.
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() == 1 {
            self.tabs[0].playlist.clear();
            if self.playing == Some(0) {
                self.playing = None;
            }
            self.mark_unsaved(0, cx);
            cx.notify();
            return;
        }
        let closed = self.tabs.remove(self.active);
        self.unsaved.remove(&closed.file);
        session::delete_playlist(&closed.file);
        self.playing = match self.playing {
            Some(tab) if tab == self.active => None,
            Some(tab) if tab > self.active => Some(tab - 1),
            other => other,
        };
        self.queued = None;
        self.active = self.active.min(self.tabs.len() - 1);
        cx.notify();
    }

    /// Renames the tab shown.
    pub fn rename_active(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.active_tab_mut().playlist.set_name(name);
        self.mark_unsaved(self.active, cx);
        cx.notify();
    }

    /// Applies probe results to every tab holding `path`.
    pub fn update_meta(
        &mut self,
        results: &[(std::path::PathBuf, TrackMeta)],
        cx: &mut Context<Self>,
    ) {
        let mut changed = Vec::new();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let mut any = false;
            for (path, meta) in results {
                any |= tab.playlist.update_meta(path, meta);
            }
            if any {
                changed.push(index);
            }
        }
        for index in changed {
            self.mark_unsaved(index, cx);
        }
        cx.notify();
    }

    /// Runs `edit` on the playlist of the tab shown, then invalidates the shuffled order and
    /// schedules a save when it changed the tracks.
    pub fn edit_active(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut Playlist)) {
        let index = self.active;
        self.edit_tab(index, cx, edit);
    }

    /// Like [`Self::edit_active`] for any tab.
    pub fn edit_tab(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Playlist),
    ) {
        let Some(tab) = self.tabs.get_mut(index) else {
            return;
        };
        let before = tab.playlist.generation();
        edit(&mut tab.playlist);
        if tab.playlist.generation() != before {
            if self.playing == Some(index) {
                self.order.invalidate();
                self.queued = None;
                if tab.playlist.current().is_none() {
                    self.playing = None;
                }
            }
            self.mark_unsaved(index, cx);
        }
        cx.notify();
    }

    /// Sets the filter of the tab shown.
    pub fn set_filter(&mut self, text: String, cx: &mut Context<Self>) {
        let tab = self.active_tab_mut();
        if tab.filter != text {
            tab.filter = text;
            cx.notify();
        }
    }

    /// Remembers that tab `index` changed and writes it a little later, so a burst of edits
    /// (probe results arriving, a drag of the selection) costs one write.
    fn mark_unsaved(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(index) {
            self.unsaved.insert(tab.file.clone());
        }
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = this.update(cx, |library, _| library.save_now());
        }));
        crate::app::persist::schedule(cx);
    }

    /// Writes every tab with unsaved changes. Called by the delayed save and on quit.
    pub fn save_now(&mut self) {
        for tab in &self.tabs {
            if self.unsaved.contains(&tab.file) {
                session::save_playlist(&tab.file, &tab.playlist);
            }
        }
        self.unsaved.clear();
        self.save_task = None;
    }

    /// The tab files in order, for the session.
    pub fn files(&self) -> Vec<String> {
        self.tabs.iter().map(|tab| tab.file.clone()).collect()
    }
}

/// The name of a new playlist: "Playlist" and its number, translated.
fn default_name(number: usize) -> String {
    t!("playlist.default_name", number = number).into_owned()
}
