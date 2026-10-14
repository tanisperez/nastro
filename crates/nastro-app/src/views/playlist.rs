//! The playlist panel: tabs, the filter, the track list and the footer. Only the visible rows
//! are built (`uniform_list`), so a playlist of thousands of tracks costs the same to draw as
//! one of twenty.

use std::rc::Rc;

use gpui_kit::AppContext as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, CursorStyle, Div, Entity, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, div, uniform_list,
};
use nastro_core::{format_length, format_total};
use nastro_dock::PanelId;
use rust_i18n::t;

use crate::actions::*;
use crate::app::{commands, models};
use crate::assets::SkinIcon;
use crate::keymap::TRACK_LIST_CONTEXT;
use crate::state::locale::plural;
use crate::theme::metrics::*;
use crate::theme::skin::white;
use crate::theme::{SkinExt, SkinTheme};
use crate::views::components::buttons::{control, text_button, window_button};
use crate::views::components::frame::{panel, title_bar};
use crate::views::components::menu::MenuTrigger;
use crate::views::components::text::icon;
use crate::views::dock::{draggable, resize_grip};
use crate::views::player::{menu_opened, toggle_shade, toggle_visible};

/// The playlist panel view.
pub struct PlaylistPanel {
    /// The root view's focus handle, where menu actions are dispatched.
    root_focus: FocusHandle,
    /// The focus of the track list, for the arrow keys.
    pub list_focus: FocusHandle,
    /// The filter field.
    filter: Entity<InputState>,
    /// The field that renames the tab shown, while renaming.
    rename: Option<Entity<InputState>>,
    /// The scroll position of the track list.
    scroll: UniformListScrollHandle,
    /// The tab the filter text belongs to, to swap texts when the tab changes.
    filter_tab: usize,
    _subscriptions: Vec<Subscription>,
}

impl PlaylistPanel {
    /// The playlist panel.
    pub fn new(root_focus: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let m = models(cx);
        let (library, shell) = (m.library.clone(), m.shell.clone());
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(t!("playlist.filter")));
        let subscriptions = vec![
            cx.observe_in(&library, window, |panel, _, window, cx| {
                panel.sync_filter(window, cx);
                cx.notify();
            }),
            cx.observe(&shell, |_, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |_, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    let library = models(cx).library.clone();
                    library.update(cx, |library, cx| library.set_filter(text, cx));
                }
            }),
        ];
        PlaylistPanel {
            root_focus,
            list_focus: cx.focus_handle(),
            filter,
            rename: None,
            scroll: UniformListScrollHandle::new(),
            filter_tab: 0,
            _subscriptions: subscriptions,
        }
    }

    /// Re-applies the placeholder after a language change.
    pub fn relocalize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |input, cx| {
            input.set_placeholder(t!("playlist.filter"), window, cx)
        });
    }

    /// Shows the filter text of the tab shown when the tab changes.
    fn sync_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let library = models(cx).library.read(cx);
        if library.active != self.filter_tab {
            self.filter_tab = library.active;
            let text = library.active_tab().filter.clone();
            self.filter
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }

    /// Puts the keyboard focus in the filter.
    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |input, cx| input.focus(window, cx));
    }

    /// Shows the rename field on the tab shown.
    pub fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = models(cx)
            .library
            .read(cx)
            .active_tab()
            .playlist
            .name()
            .to_string();
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        cx.subscribe_in(
            &input,
            window,
            |panel, entity, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let name = entity.read(cx).value().to_string();
                    let library = models(cx).library.clone();
                    library.update(cx, |library, cx| library.rename_active(name, cx));
                    panel.end_rename(window, cx);
                }
                InputEvent::Blur if panel.rename.as_ref() == Some(entity) => {
                    panel.end_rename(window, cx);
                }
                _ => {}
            },
        )
        .detach();
        self.rename = Some(input);
        cx.notify();
    }

    /// Hides the rename field and gives the focus back to the window.
    fn end_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = None;
        self.root_focus.focus(window, cx);
        cx.notify();
    }

    /// Moves the selection one row up or down within the rows shown, and scrolls to it.
    pub fn step_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        let library = models(cx).library.clone();
        let target = library.update(cx, |library, cx| {
            let tab = library.active_tab_mut();
            let rows = tab.rows();
            if rows.is_empty() {
                return None;
            }
            let selected = tab.playlist.selected_indices();
            let position = selected
                .first()
                .and_then(|first| rows.iter().position(|row| row == first));
            let next = match (position, down) {
                (None, _) => 0,
                (Some(p), true) => (p + 1).min(rows.len() - 1),
                (Some(p), false) => p.saturating_sub(1),
            };
            let index = rows[next];
            library.edit_active(cx, |playlist| playlist.select_only(index));
            Some(next)
        });
        if let Some(row) = target {
            self.scroll.scroll_to_item(row, ScrollStrategy::Center);
        }
    }

    /// Handles a press on row `index`: select, extend or toggle with the modifiers, and play on
    /// a double click.
    fn press_row(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.list_focus.focus(window, cx);
        let library = models(cx).library.clone();
        let tab = library.read(cx).active;
        let modifiers = event.modifiers;
        library.update(cx, |library, cx| {
            library.edit_active(cx, |playlist| {
                if modifiers.shift {
                    playlist.select_range_to(index);
                } else if modifiers.secondary() {
                    playlist.toggle_selected(index);
                } else {
                    playlist.select_only(index);
                }
            });
        });
        if event.click_count == 2 && !modifiers.shift && !modifiers.secondary() {
            commands::play_track(tab, index, cx);
        }
    }
}

impl Render for PlaylistPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.skin();
        let active = window.is_window_active();
        let shell = models(cx).shell.read(cx);
        let shaded = shell.is_shaded(PanelId::Playlist);

        let buttons: Vec<AnyElement> = vec![
            window_button("pl-shade", SkinIcon::Shade, &t)
                .on_click(|_, _, cx| toggle_shade(PanelId::Playlist, cx))
                .into_any_element(),
            window_button("pl-close", SkinIcon::Close, &t)
                .on_click(|_, _, cx| toggle_visible(PanelId::Playlist, cx))
                .into_any_element(),
        ];
        let title = draggable(
            title_bar(&t, None, t!("panel.playlist").into_owned(), active, buttons)
                .when(shaded, |bar| bar.border_b_0())
                .id("pl-title"),
            PanelId::Playlist,
        );
        if shaded {
            return panel(&t).child(title).into_any_element();
        }

        let library = models(cx).library.clone();
        let (rows, current, playing_here, summary) = library.update(cx, |library, _| {
            let playing_here = library.playing == Some(library.active);
            let tab = library.active_tab_mut();
            let rows = tab.rows();
            let (total, _) = tab.playlist.total_duration();
            let summary = format!(
                "{} · {}",
                plural("playlist.tracks", tab.playlist.len()),
                format_total(total)
            );
            (rows, tab.playlist.current(), playing_here, summary)
        });
        let empty_text = if library.read(cx).active_tab().playlist.is_empty() {
            t!("playlist.empty").into_owned()
        } else {
            t!("playlist.no_match").into_owned()
        };

        panel(&t)
            .child(title)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(t.px(PLAYLIST_GAP))
                    .pt(t.px(PLAYLIST_PADDING_TOP))
                    .px(t.px(PLAYLIST_PADDING))
                    .pb(t.px(PLAYLIST_PADDING))
                    .child(self.tabs_row(&t, cx))
                    .child(self.track_list(
                        &t,
                        rows,
                        if playing_here { current } else { None },
                        empty_text,
                        cx,
                    ))
                    .child(self.footer(&t, summary)),
            )
            .child(resize_grip(
                div()
                    .id("pl-resize")
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .size(t.px(RESIZE_GRIP))
                    .cursor(CursorStyle::ResizeUpLeftDownRight)
                    .child(icon(
                        SkinIcon::Grip,
                        t.px(RESIZE_GRIP - 2.0),
                        t.colors.text_muted,
                    )),
            ))
            .into_any_element()
    }
}

impl PlaylistPanel {
    /// The tabs, the new playlist button and the filter.
    fn tabs_row(&self, t: &Rc<SkinTheme>, cx: &mut Context<Self>) -> Div {
        let c = &t.colors;
        let library = models(cx).library.read(cx);
        let active = library.active;
        let names: Vec<String> = library
            .tabs
            .iter()
            .map(|tab| tab.playlist.name().to_string())
            .collect();
        let tab_hover = c.text.opacity(0.12);
        let tabs = names.into_iter().enumerate().map(|(index, name)| {
            let selected = index == active;
            let renaming = selected && self.rename.is_some();
            let tab = div()
                .id(("tab", index))
                .h(t.px(TAB_HEIGHT))
                .px(t.px(TAB_PADDING))
                .flex()
                .items_center()
                .border_1()
                .rounded_t(t.px(TAB_RADIUS))
                .text_size(t.px(TAB_SIZE))
                .whitespace_nowrap()
                .cursor_pointer();
            let tab = if selected {
                tab.border_color(c.lcd_edge)
                    .bg(c.pl_bg)
                    .text_color(c.pl_current)
                    .font_weight(FontWeight::SEMIBOLD)
            } else {
                tab.border_color(gpui_kit::transparent_black())
                    .text_color(c.text)
                    .font_weight(FontWeight::MEDIUM)
                    .hover(move |style| style.bg(tab_hover))
            };
            match (&self.rename, renaming) {
                (Some(input), true) => tab
                    .w(t.px(140.0))
                    .child(
                        Input::new(input)
                            .appearance(false)
                            .text_size(t.px(TAB_SIZE))
                            .text_color(c.pl_current)
                            .py_0()
                            .h_full(),
                    )
                    .into_any_element(),
                _ => tab
                    .on_click(cx.listener(move |panel, event: &ClickEvent, window, cx| {
                        let library = models(cx).library.clone();
                        library.update(cx, |library, cx| library.activate(index, cx));
                        if event.click_count() == 2 {
                            panel.begin_rename(window, cx);
                        }
                    }))
                    .child(name)
                    .into_any_element(),
            }
        });
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(t.px(4.0))
            .child(div().flex().gap(t.px(TAB_GAP)).children(tabs))
            .child(
                control("new-playlist", t, false)
                    .size(t.px(NEW_TAB_BUTTON))
                    .rounded(t.px(NEW_TAB_RADIUS))
                    .child(icon(SkinIcon::Plus, t.px(9.0), c.text))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(NewPlaylist), cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .h(t.px(TAB_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(t.px(6.0))
                    .px(t.px(FILTER_PADDING))
                    .rounded(t.px(FILTER_RADIUS))
                    .border_1()
                    .border_color(c.frame_edge)
                    .bg(c.field_bg)
                    .text_color(c.text_muted)
                    .child(icon(SkinIcon::Search, t.px(FILTER_ICON), c.text_muted))
                    .child(
                        div().w(t.px(FILTER_INPUT_WIDTH)).child(
                            Input::new(&self.filter)
                                .appearance(false)
                                .text_size(t.px(TAB_SIZE))
                                .text_color(c.text)
                                .py_0()
                                .h(t.px(TAB_HEIGHT - 2.0)),
                        ),
                    ),
            )
    }

    /// The recessed list of tracks, or the empty message.
    fn track_list(
        &self,
        t: &Rc<SkinTheme>,
        rows: Rc<Vec<usize>>,
        current: Option<usize>,
        empty_text: String,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<Div> {
        let c = &t.colors;
        let theme = t.clone();
        let count = rows.len();
        let list = uniform_list(
            "tracks",
            count,
            cx.processor(move |_panel, range: std::ops::Range<usize>, _window, cx| {
                let library = models(cx).library.read(cx);
                let playlist = &library.active_tab().playlist;
                range
                    .filter_map(|row| rows.get(row).copied())
                    .filter_map(|index| {
                        let track = playlist.track(index)?;
                        Some(track_row(
                            &theme,
                            index,
                            track.label(),
                            track.meta.duration.map(format_length),
                            playlist.is_selected(index),
                            current == Some(index),
                            cx.listener(
                                move |panel: &mut PlaylistPanel,
                                      event: &MouseDownEvent,
                                      window,
                                      cx| {
                                    panel.press_row(index, event, window, cx)
                                },
                            ),
                        ))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full();
        div()
            .id("track-list")
            .track_focus(&self.list_focus)
            .key_context(TRACK_LIST_CONTEXT)
            .flex_1()
            .min_h_0()
            .rounded_tr(t.px(LIST_RADIUS))
            .rounded_b(t.px(LIST_RADIUS))
            .border_1()
            .border_color(c.lcd_edge)
            .bg(c.pl_bg)
            .shadow(t.list_shadow())
            .py(t.px(LIST_PADDING))
            .on_action(
                cx.listener(|panel, _: &SelectPrevious, _, cx| panel.step_selection(false, cx)),
            )
            .on_action(cx.listener(|panel, _: &SelectNext, _, cx| panel.step_selection(true, cx)))
            .when(count > 0, |box_| box_.child(list))
            .when(count == 0, |box_| {
                box_.child(
                    div()
                        .py(t.px(24.0))
                        .px(t.px(12.0))
                        .text_center()
                        .text_size(t.px(ROW_SIZE))
                        .text_color(c.pl_fg)
                        .child(empty_text),
                )
            })
    }

    /// ADD, REMOVE, SELECT, SORT, and the summary.
    fn footer(&self, t: &Rc<SkinTheme>, summary: String) -> Div {
        let focus = self.root_focus.clone();
        let menu_button =
            |id: &'static str, label: SharedString, build: fn(PopupMenu) -> PopupMenu| {
                let focus = focus.clone();
                MenuTrigger::wrap(text_button(id, label, t), t.clone())
                    .dropdown_menu_with_anchor(gpui_kit::Anchor::BottomLeft, move |menu, _, _| {
                        build(menu.action_context(focus.clone()))
                    })
                    .on_open_change(menu_opened)
            };
        div()
            .h(t.px(FOOTER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(t.px(FOOTER_GAP))
            .child(menu_button("pl-add", t!("playlist.add").into(), add_menu))
            .child(menu_button(
                "pl-remove",
                t!("playlist.remove").into(),
                remove_menu,
            ))
            .child(menu_button(
                "pl-select",
                t!("playlist.select").into(),
                select_menu,
            ))
            .child(menu_button(
                "pl-sort",
                t!("playlist.sort").into(),
                sort_menu,
            ))
            .child(div().flex_1())
            .child(
                div()
                    .mr(t.px(RESIZE_GRIP - 4.0))
                    .font_family(t.font_mono.clone())
                    .text_size(t.px(ROW_NUMBER_SIZE))
                    .text_color(t.colors.text_muted)
                    .whitespace_nowrap()
                    .child(summary),
            )
    }
}

/// One track row: index (or the playing glyph), label, length.
fn track_row(
    t: &SkinTheme,
    index: usize,
    label: String,
    length: Option<String>,
    selected: bool,
    playing: bool,
    on_press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let c = &t.colors;
    let bg = if selected {
        c.pl_selected_bg
    } else {
        gpui_kit::transparent_black()
    };
    let hover = if selected {
        c.pl_selected_bg.blend(white(0.07))
    } else {
        c.pl_hover
    };
    let fg = if playing { c.pl_current } else { c.pl_fg };
    let index_color = if playing { c.pl_current } else { c.pl_index };
    let number = div()
        .w(t.px(INDEX_COLUMN))
        .flex_none()
        .flex()
        .justify_end()
        .font_family(t.font_mono.clone())
        .text_size(t.px(ROW_NUMBER_SIZE))
        .text_color(index_color);
    let number = if playing {
        number.child(icon(SkinIcon::StatePlay, t.px(PLAYING_GLYPH), index_color))
    } else {
        number.child((index + 1).to_string())
    };
    div()
        .id(("row", index))
        .w_full()
        .h(t.px(ROW_HEIGHT))
        .flex()
        .items_center()
        .gap(t.px(ROW_GAP))
        .px(t.px(ROW_PADDING))
        .bg(bg)
        .hover(move |style| style.bg(hover))
        .text_size(t.px(ROW_SIZE))
        .text_color(fg)
        .font_weight(if playing {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_press)
        .child(number)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .children(length.map(|length| {
            div()
                .font_family(t.font_mono.clone())
                .text_size(t.px(ROW_NUMBER_SIZE))
                .text_color(index_color)
                .child(length)
        }))
        .into_any_element()
}

/// ADD: files and folders.
fn add_menu(menu: PopupMenu) -> PopupMenu {
    menu.menu(t!("menu.file.add_files"), Box::new(AddFiles))
        .menu(t!("menu.file.add_folder"), Box::new(AddFolder))
}

/// REMOVE: the selection, everything but the selection, everything.
fn remove_menu(menu: PopupMenu) -> PopupMenu {
    menu.menu(t!("playlist.remove_selected"), Box::new(RemoveSelected))
        .menu(t!("playlist.crop"), Box::new(CropToSelection))
        .menu(t!("playlist.clear"), Box::new(ClearPlaylist))
}

/// SELECT: all, none, invert.
fn select_menu(menu: PopupMenu) -> PopupMenu {
    menu.menu(t!("playlist.select_all"), Box::new(SelectAll))
        .menu(t!("playlist.select_none"), Box::new(SelectNone))
        .menu(t!("playlist.invert_selection"), Box::new(InvertSelection))
}

/// SORT: the keys, reverse and randomize.
fn sort_menu(menu: PopupMenu) -> PopupMenu {
    menu.menu(t!("playlist.sort_title"), Box::new(SortByTitle))
        .menu(t!("playlist.sort_artist"), Box::new(SortByArtist))
        .menu(t!("playlist.sort_album"), Box::new(SortByAlbum))
        .menu(t!("playlist.sort_file_name"), Box::new(SortByFileName))
        .menu(t!("playlist.sort_path"), Box::new(SortByPath))
        .menu(t!("playlist.sort_length"), Box::new(SortByLength))
        .separator()
        .menu(t!("playlist.reverse"), Box::new(ReversePlaylist))
        .menu(t!("playlist.randomize"), Box::new(RandomizePlaylist))
}
