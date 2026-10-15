//! The main window's root view. The window is transparent and holds the docked group: each
//! displayed panel is placed at its position from `nastro-dock`, the window is resized to fit
//! them, and on Wayland and Windows its input is limited to the panels so clicks in the gaps
//! reach whatever is behind (`docs/architecture.md`, "Windows and docking").

use gpui_kit::AppContext as _;
use gpui_kit::{
    AnyElement, App, Bounds, Context, Entity, ExternalPaths, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Pixels, Render, Size, StyleRefinement, Styled,
    Subscription, Window, div,
};
use nastro_core::SortKey;
use nastro_dock::{LayoutPreset, PanelId};

use crate::actions::*;
use crate::app::{commands, models, persist};
use crate::keymap::WINDOW_CONTEXT;
use crate::platform;
use crate::state::settings::SkinScale;
use crate::theme::SkinExt;
use crate::views::equalizer::EqualizerPanel;
use crate::views::player::{PlayerPanel, toggle_shade, toggle_visible};
use crate::views::playlist::PlaylistPanel;

/// The root view of the main window.
pub struct NastroWindow {
    focus: FocusHandle,
    player: Entity<PlayerPanel>,
    equalizer: Entity<EqualizerPanel>,
    playlist: Entity<PlaylistPanel>,
    /// The window size last requested, so a resize is only asked for when it changes.
    applied_size: Option<Size<Pixels>>,
    /// The input region last set, and the window scale factor it was set at.
    applied_region: Option<(Option<Vec<Bounds<Pixels>>>, f32)>,
    _subscriptions: Vec<Subscription>,
}

impl NastroWindow {
    /// The root view, with the three panels.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let player = cx.new(|cx| PlayerPanel::new(focus.clone(), cx));
        let equalizer = cx.new(|cx| EqualizerPanel::new(focus.clone(), cx));
        let playlist = cx.new(|cx| PlaylistPanel::new(focus.clone(), window, cx));
        let shell = models(cx).shell.clone();
        let subscriptions = vec![
            cx.observe_in(&shell, window, |view, _, window, cx| {
                view.sync_window(window, cx);
                cx.notify();
            }),
            // Windows takes the region in device pixels: a new scale factor (another display)
            // needs it again.
            cx.observe_window_bounds(window, |view, window, cx| view.sync_window(window, cx)),
        ];
        focus.focus(window, cx);
        let mut view = NastroWindow {
            focus,
            player,
            equalizer,
            playlist,
            applied_size: None,
            applied_region: None,
            _subscriptions: subscriptions,
        };
        view.sync_window(window, cx);
        view
    }

    /// Resizes the window to the panels and limits its input to them.
    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = models(cx).shell.read(cx);
        let size = shell.window_size();
        let region: Vec<Bounds<Pixels>> = PanelId::ALL
            .into_iter()
            .filter_map(|id| shell.panel_bounds(id))
            .collect();
        let menu_open = shell.menu_open;
        if self.applied_size != Some(size) {
            log::debug!(
                "window size {}x{}",
                f32::from(size.width),
                f32::from(size.height)
            );
            window.resize(size);
            self.applied_size = Some(size);
        }
        // While a menu is open it may extend past the panels, so the whole window takes input.
        let region = ((!menu_open).then_some(region), window.scale_factor());
        if self.applied_region.as_ref() != Some(&region) {
            let shadow = cx.skin().panel_shadow_reach();
            platform::limit_input(window, region.0.as_deref(), shadow);
            self.applied_region = Some(region);
        }
    }

    /// Re-applies strings cached outside `render` after a language change.
    pub fn relocalize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playlist
            .update(cx, |playlist, cx| playlist.relocalize(window, cx));
    }

    /// Asks for files or a folder, then adds them (and plays the first with `play`).
    fn prompt_and_open(&mut self, directories: bool, play: bool, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: !directories,
            directories,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                cx.update(|cx| commands::open_paths(paths, play, cx));
            }
        })
        .detach();
    }

    /// Saves a copy of the playlist shown where the user says.
    fn export_playlist(&mut self, cx: &mut Context<Self>) {
        let library = models(cx).library.read(cx);
        let playlist = library.active_tab().playlist.clone();
        let suggested = format!("{}.m3u8", playlist.name());
        let directory = dirs::audio_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |_, _| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let text = nastro_core::write_m3u(Some(playlist.name()), playlist.tracks());
                match std::fs::write(&path, text) {
                    Ok(()) => log::info!("playlist exported to {}", path.display()),
                    Err(error) => log::warn!("could not export to {}: {error}", path.display()),
                }
            }
        })
        .detach();
    }

    /// Edits the playlist shown.
    fn edit(cx: &mut App, edit: impl FnOnce(&mut nastro_core::Playlist)) {
        let library = models(cx).library.clone();
        library.update(cx, |library, cx| library.edit_active(cx, edit));
    }

    /// Sorts the playlist shown.
    fn sort(cx: &mut App, key: SortKey) {
        Self::edit(cx, |playlist| playlist.sort(key));
    }

    /// Applies a layout preset.
    fn layout(cx: &mut App, preset: LayoutPreset) {
        let shell = models(cx).shell.clone();
        shell.update(cx, |shell, cx| shell.apply_preset(preset, cx));
    }

    /// Sets the skin scale.
    fn scale(cx: &mut App, scale: SkinScale) {
        let shell = models(cx).shell.clone();
        shell.update(cx, |shell, cx| shell.set_scale(scale, cx));
    }

    /// Changes the volume by `delta`.
    fn nudge_volume(cx: &mut App, delta: f32) {
        let transport = models(cx).transport.clone();
        transport.update(cx, |transport, cx| {
            let volume = transport.volume + delta;
            transport.set_volume(volume, cx);
        });
    }
}

impl Render for NastroWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = models(cx).shell.read(cx);
        let mut panels: Vec<AnyElement> = Vec::new();
        for id in PanelId::ALL {
            let Some(bounds) = shell.panel_bounds(id) else {
                continue;
            };
            // Cached: a panel is rebuilt only when it notifies, so the 30 frames per second of
            // the player while playing never rebuild the equalizer or the playlist.
            let style = StyleRefinement::default().size_full();
            let content: AnyElement = match id {
                PanelId::Player => self.player.clone().cached(style).into_any_element(),
                PanelId::Equalizer => self.equalizer.clone().cached(style).into_any_element(),
                PanelId::Playlist => self.playlist.clone().cached(style).into_any_element(),
            };
            panels.push(
                div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .child(content)
                    .into_any_element(),
            );
        }

        div()
            .id("nastro")
            .key_context(WINDOW_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .on_drop(|paths: &ExternalPaths, _, cx| {
                commands::open_paths(paths.paths().to_vec(), false, cx)
            })
            // Transport.
            .on_action(|_: &Play, _, cx| commands::play(cx))
            .on_action(|_: &Pause, _, cx| commands::pause(cx))
            .on_action(|_: &PlayPause, _, cx| commands::play_pause(cx))
            .on_action(|_: &Stop, _, cx| commands::stop(cx))
            .on_action(|_: &Previous, _, cx| commands::previous(cx))
            .on_action(|_: &Next, _, cx| commands::next(cx))
            .on_action(|_: &SeekForward, _, cx| {
                let transport = models(cx).transport.clone();
                transport.update(cx, |transport, cx| transport.seek_by(true, cx));
            })
            .on_action(|_: &SeekBackward, _, cx| {
                let transport = models(cx).transport.clone();
                transport.update(cx, |transport, cx| transport.seek_by(false, cx));
            })
            .on_action(|_: &VolumeUp, _, cx| {
                Self::nudge_volume(cx, crate::app::transport::VOLUME_STEP)
            })
            .on_action(|_: &VolumeDown, _, cx| {
                Self::nudge_volume(cx, -crate::app::transport::VOLUME_STEP)
            })
            .on_action(|_: &ToggleShuffle, _, cx| commands::toggle_shuffle(cx))
            .on_action(|_: &ToggleRepeat, _, cx| commands::toggle_repeat(cx))
            // Files and playlists.
            .on_action(
                cx.listener(|view, _: &OpenFiles, _, cx| view.prompt_and_open(false, true, cx)),
            )
            .on_action(
                cx.listener(|view, _: &AddFiles, _, cx| view.prompt_and_open(false, false, cx)),
            )
            .on_action(
                cx.listener(|view, _: &AddFolder, _, cx| view.prompt_and_open(true, false, cx)),
            )
            .on_action(cx.listener(|view, _: &ExportPlaylist, _, cx| view.export_playlist(cx)))
            .on_action(|_: &NewPlaylist, _, cx| {
                let library = models(cx).library.clone();
                library.update(cx, |library, cx| library.new_tab(cx));
            })
            .on_action(|_: &ClosePlaylist, _, cx| {
                let library = models(cx).library.clone();
                library.update(cx, |library, cx| library.close_active_tab(cx));
                commands::requeue(cx);
                persist::schedule(cx);
            })
            .on_action(cx.listener(|view, _: &RenamePlaylist, window, cx| {
                if !models(cx).shell.read(cx).is_visible(PanelId::Playlist) {
                    toggle_visible(PanelId::Playlist, cx);
                }
                view.playlist
                    .update(cx, |playlist, cx| playlist.begin_rename(window, cx));
            }))
            .on_action(cx.listener(|view, _: &FocusFilter, window, cx| {
                view.playlist
                    .update(cx, |playlist, cx| playlist.focus_filter(window, cx));
            }))
            .on_action(|_: &PlaySelected, _, cx| commands::play_selected(cx))
            .on_action(|_: &RemoveSelected, _, cx| {
                Self::edit(cx, |playlist| {
                    playlist.remove_selected();
                })
            })
            .on_action(|_: &CropToSelection, _, cx| {
                Self::edit(cx, |playlist| playlist.crop_to_selection())
            })
            .on_action(|_: &ClearPlaylist, _, cx| Self::edit(cx, |playlist| playlist.clear()))
            .on_action(|_: &SelectAll, _, cx| Self::edit(cx, |playlist| playlist.select_all()))
            .on_action(|_: &SelectNone, _, cx| {
                Self::edit(cx, |playlist| playlist.clear_selection())
            })
            .on_action(|_: &InvertSelection, _, cx| {
                Self::edit(cx, |playlist| playlist.invert_selection())
            })
            .on_action(|_: &MoveSelectedUp, _, cx| {
                Self::edit(cx, |playlist| playlist.move_selected(-1))
            })
            .on_action(|_: &MoveSelectedDown, _, cx| {
                Self::edit(cx, |playlist| playlist.move_selected(1))
            })
            .on_action(|_: &SortByTitle, _, cx| Self::sort(cx, SortKey::Title))
            .on_action(|_: &SortByArtist, _, cx| Self::sort(cx, SortKey::Artist))
            .on_action(|_: &SortByAlbum, _, cx| Self::sort(cx, SortKey::Album))
            .on_action(|_: &SortByFileName, _, cx| Self::sort(cx, SortKey::FileName))
            .on_action(|_: &SortByPath, _, cx| Self::sort(cx, SortKey::Path))
            .on_action(|_: &SortByLength, _, cx| Self::sort(cx, SortKey::Duration))
            .on_action(|_: &ReversePlaylist, _, cx| Self::edit(cx, |playlist| playlist.reverse()))
            .on_action(|_: &RandomizePlaylist, _, cx| {
                let seed = fastrand_seed();
                Self::edit(cx, |playlist| playlist.randomize(seed))
            })
            // Panels and layout.
            .on_action(|_: &ToggleEqualizerPanel, _, cx| toggle_visible(PanelId::Equalizer, cx))
            .on_action(|_: &TogglePlaylistPanel, _, cx| toggle_visible(PanelId::Playlist, cx))
            .on_action(|_: &ShadePlayer, _, cx| toggle_shade(PanelId::Player, cx))
            .on_action(|_: &ShadeEqualizer, _, cx| toggle_shade(PanelId::Equalizer, cx))
            .on_action(|_: &ShadePlaylist, _, cx| toggle_shade(PanelId::Playlist, cx))
            .on_action(|_: &LayoutA, _, cx| Self::layout(cx, LayoutPreset::A))
            .on_action(|_: &LayoutB, _, cx| Self::layout(cx, LayoutPreset::B))
            .on_action(|_: &LayoutC, _, cx| Self::layout(cx, LayoutPreset::C))
            .on_action(|_: &LayoutD, _, cx| Self::layout(cx, LayoutPreset::D))
            .on_action(|_: &SkinScale1, _, cx| Self::scale(cx, SkinScale::X1))
            .on_action(|_: &SkinScale15, _, cx| Self::scale(cx, SkinScale::X1_5))
            .on_action(|_: &SkinScale2, _, cx| Self::scale(cx, SkinScale::X2))
            .on_action(|_: &ToggleTimeDisplay, _, cx| {
                let shell = models(cx).shell.clone();
                shell.update(cx, |shell, cx| shell.toggle_time_display(cx));
            })
            .on_action(|_: &NextVisualizer, _, cx| {
                let shell = models(cx).shell.clone();
                shell.update(cx, |shell, cx| {
                    let next = shell.settings.visualizer.next();
                    shell.set_visualizer(next, cx);
                });
            })
            // Equalizer.
            .on_action(|_: &ToggleEqualizer, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.toggle_enabled(cx));
            })
            .on_action(|_: &ToggleEqualizerAuto, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.toggle_auto(cx));
            })
            .on_action(|_: &ResetEqualizer, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.reset(cx));
            })
            // App.
            .on_action(|_: &OpenPreferences, _, cx| crate::views::preferences::open(cx))
            .on_action(|_: &OpenAbout, _, cx| crate::views::about::open(cx))
            .on_action(|_: &OpenUserGuide, _, cx| cx.open_url(crate::views::USER_GUIDE_URL))
            .on_action(|_: &OpenSkinsFolder, _, cx| crate::views::open_skins_folder(cx))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Quit, _, cx| crate::views::quit(cx))
            .children(panels)
    }
}

/// A seed for "Randomize", different on every use.
fn fastrand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0)
}
