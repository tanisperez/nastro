//! The player panel: title bar, menu bar, the display and seek bar (`display.rs`), the
//! transport, volume and balance, and the four toggles. In shade mode it collapses to a 24 px bar
//! with a small clock, the title and the transport.

use std::rc::Rc;

use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Window, div,
};
use nastro_core::PlayState;
use nastro_dock::PanelId;
use rust_i18n::t;

use crate::app::{commands, models};
use crate::assets::SkinIcon;
use crate::state::slider::{balance_at, balance_fraction, volume_at};
use crate::theme::metrics::*;
use crate::theme::{SkinExt, SkinTheme};
use crate::views::components::buttons::{led_toggle, transport_button, window_button};
use crate::views::components::frame::{groove, panel, title_bar};
use crate::views::components::menu::MenuTrigger;
use crate::views::components::slider::{Axis, OnChange, SliderLook, slider};
use crate::views::components::text::icon;
use crate::views::display::{Display, Shown};
use crate::views::menus;

/// The player panel view.
pub struct PlayerPanel {
    /// The root view's focus handle, where menu actions are dispatched.
    root_focus: FocusHandle,
    /// The display and the seek bar, the only part rebuilt on every frame while playing.
    display: Entity<Display>,
    _subscriptions: Vec<Subscription>,
}

impl PlayerPanel {
    /// The player panel, repainting when the transport, the playlists or the shell change.
    pub fn new(root_focus: FocusHandle, cx: &mut Context<Self>) -> Self {
        let m = models(cx);
        let (transport, library, shell) = (m.transport.clone(), m.library.clone(), m.shell.clone());
        let frame = transport.read(cx).frame.clone();
        let display = cx.new(Display::new);
        PlayerPanel {
            root_focus,
            display,
            _subscriptions: vec![
                cx.observe(&transport, |_, _, cx| cx.notify()),
                cx.observe(&library, |_, _, cx| cx.notify()),
                cx.observe(&shell, |_, _, cx| cx.notify()),
                // The shade bar has a clock and a title of its own: it follows the frames only
                // while the player is shaded.
                cx.observe(&frame, |_, _, cx| {
                    if models(cx).shell.read(cx).is_shaded(PanelId::Player) {
                        cx.notify();
                    }
                }),
            ],
        }
    }
}

impl Render for PlayerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.skin();
        let shown = Shown::read(cx);
        let active = window.is_window_active();
        if shown.shaded {
            return shade_bar(&t, &shown, active).into_any_element();
        }
        panel(&t)
            .child(player_title_bar(&t, active, false))
            .child(self.menu_bar(&t))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(t.px(PLAYER_GAP))
                    .pt(t.px(PLAYER_PADDING_TOP))
                    .px(t.px(PLAYER_PADDING))
                    .pb(t.px(PLAYER_PADDING))
                    .child(
                        self.display.clone().cached(
                            StyleRefinement::default()
                                .w_full()
                                .h(t.px(LCD_HEIGHT + PLAYER_GAP + SEEK_HEIGHT)),
                        ),
                    )
                    .child(transport_row(&t, &shown))
                    .child(toggles_row(&t, &shown)),
            )
            .into_any_element()
    }
}

impl PlayerPanel {
    /// File, Play, Options, View and Help. An empty spot of the bar moves the window.
    fn menu_bar(&self, t: &Rc<SkinTheme>) -> Div {
        let focus = self.root_focus.clone();
        let trigger =
            |id: &'static str, label: SharedString| MenuTrigger::new(id, label, t.clone());
        let f = focus.clone();
        let file =
            trigger("menu-file", t!("menu.file").into()).dropdown_menu(move |menu, window, cx| {
                menus::file_menu(menu.action_context(f.clone()), window, cx)
            });
        let f = focus.clone();
        let play =
            trigger("menu-play", t!("menu.play").into()).dropdown_menu(move |menu, window, cx| {
                menus::play_menu(menu.action_context(f.clone()), window, cx)
            });
        let f = focus.clone();
        let options = trigger("menu-options", t!("menu.options").into()).dropdown_menu(
            move |menu, window, cx| menus::options_menu(menu.action_context(f.clone()), window, cx),
        );
        let f = focus.clone();
        let view =
            trigger("menu-view", t!("menu.view").into()).dropdown_menu(move |menu, window, cx| {
                menus::view_menu(menu.action_context(f.clone()), window, cx)
            });
        let f = focus;
        let help =
            trigger("menu-help", t!("menu.help").into()).dropdown_menu(move |menu, window, cx| {
                menus::help_menu(menu.action_context(f.clone()), window, cx)
            });
        let (file, play, options, view, help) = (
            file.on_open_change(menu_opened),
            play.on_open_change(menu_opened),
            options.on_open_change(menu_opened),
            view.on_open_change(menu_opened),
            help.on_open_change(menu_opened),
        );
        div()
            .h(t.px(MENU_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(t.px(2.0))
            .px(t.px(6.0))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(file)
            .child(play)
            .child(options)
            .child(view)
            .child(help)
    }
}

/// Tells the shell a menu opened or closed, so the whole window takes input while it is open.
pub fn menu_opened(open: &bool, _: &mut Window, cx: &mut App) {
    let shell = models(cx).shell.clone();
    let open = *open;
    shell.update(cx, |shell, cx| shell.set_menu_open(open, cx));
}

/// The player's title bar: logo, NASTRO, groove, minimize, shade and close. Dragging it moves
/// the whole window.
fn player_title_bar(t: &SkinTheme, active: bool, shaded: bool) -> Div {
    let buttons: Vec<AnyElement> = vec![
        window_button("player-minimize", SkinIcon::Minimize, t)
            .on_click(|_, window, _| window.minimize_window())
            .into_any_element(),
        window_button("player-shade", SkinIcon::Shade, t)
            .on_click(|_, _, cx| toggle_shade(PanelId::Player, cx))
            .into_any_element(),
        window_button("player-close", SkinIcon::Close, t)
            .on_click(|_, _, cx| crate::views::quit(cx))
            .into_any_element(),
    ];
    let logo = icon(SkinIcon::Logo, t.px(LOGO_GLYPH), t.colors.accent_ink).into_any_element();
    title_bar(
        t,
        Some(logo),
        t!("panel.player").into_owned(),
        active,
        buttons,
    )
    .when(shaded, |bar| bar.border_b_0())
    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}

/// Collapses or expands a panel.
pub fn toggle_shade(id: PanelId, cx: &mut App) {
    let shell = models(cx).shell.clone();
    shell.update(cx, |shell, cx| shell.toggle_shade(id, cx));
}

/// The transport buttons, the open button, volume and balance.
fn transport_row(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    let c = &t.colors;
    let play_ink = if shown.state == PlayState::Playing {
        c.accent_ink
    } else {
        c.text
    };
    let pause_ink = if shown.state == PlayState::Paused {
        c.accent_ink
    } else {
        c.text
    };
    let small = SliderLook {
        track: SMALL_TRACK,
        thumb_length: VOLUME_THUMB_WIDTH,
        thumb_breadth: SMALL_THUMB_HEIGHT,
        thumb_radius: SMALL_THUMB_RADIUS,
        thumb_inset: 1.0,
        fill: true,
        center_tick: false,
        tick_length: 0.0,
    };
    let balance_look = SliderLook {
        thumb_length: BALANCE_THUMB_WIDTH,
        fill: false,
        center_tick: true,
        tick_length: BALANCE_TICK,
        ..small
    };
    let on_volume: OnChange = Rc::new(|fraction, _, cx| {
        let transport = models(cx).transport.clone();
        transport.update(cx, |transport, cx| {
            transport.set_volume(volume_at(fraction), cx)
        });
    });
    let on_balance: OnChange = Rc::new(|fraction, _, cx| {
        let transport = models(cx).transport.clone();
        transport.update(cx, |transport, cx| {
            transport.set_balance(balance_at(fraction), cx)
        });
    });
    div()
        .h(t.px(TRANSPORT_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(t.px(PLAYER_GAP))
        .child(
            div()
                .flex()
                .items_center()
                .gap(t.px(TRANSPORT_GAP))
                .child(
                    transport_button(
                        "previous",
                        SkinIcon::Previous,
                        TRANSPORT_BUTTON,
                        TRANSPORT_ICON,
                        c.text,
                        t,
                    )
                    .on_click(|_, _, cx| commands::previous(cx)),
                )
                .child(
                    transport_button("play", SkinIcon::Play, PLAY_BUTTON, PLAY_ICON, play_ink, t)
                        .on_click(|_, _, cx| commands::play(cx)),
                )
                .child(
                    transport_button(
                        "pause",
                        SkinIcon::Pause,
                        TRANSPORT_BUTTON,
                        TRANSPORT_ICON,
                        pause_ink,
                        t,
                    )
                    .on_click(|_, _, cx| commands::pause(cx)),
                )
                .child(
                    transport_button(
                        "stop",
                        SkinIcon::Stop,
                        TRANSPORT_BUTTON,
                        STOP_ICON,
                        c.text,
                        t,
                    )
                    .on_click(|_, _, cx| commands::stop(cx)),
                )
                .child(
                    transport_button(
                        "next",
                        SkinIcon::Next,
                        TRANSPORT_BUTTON,
                        TRANSPORT_ICON,
                        c.text,
                        t,
                    )
                    .on_click(|_, _, cx| commands::next(cx)),
                ),
        )
        .child(
            crate::views::components::buttons::control("open", t, false)
                .w(t.px(OPEN_BUTTON_WIDTH))
                .h(t.px(OPEN_BUTTON_HEIGHT))
                .rounded(t.px(OPEN_BUTTON_RADIUS))
                .child(icon(SkinIcon::Open, t.px(SMALL_ICON), c.text))
                .on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(crate::actions::OpenFiles), cx)
                }),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(t.px(SLIDER_LABEL_GAP))
                .child(icon(SkinIcon::Volume, t.px(VOLUME_ICON), c.text_muted))
                .child(
                    slider(
                        "volume",
                        t,
                        Axis::Horizontal,
                        shown.volume,
                        small,
                        on_volume,
                        None,
                    )
                    .w(t.px(VOLUME_WIDTH))
                    .h(t.px(SMALL_SLIDER_HEIGHT)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(t.px(SLIDER_LABEL_GAP))
                .child(crate::views::components::text::tracked(
                    t!("player.balance").into_owned(),
                    t.px(LED_TOGGLE_SIZE),
                    BUTTON_TRACKING,
                    FontWeight::SEMIBOLD,
                    c.text_muted,
                ))
                .child(
                    slider(
                        "balance",
                        t,
                        Axis::Horizontal,
                        balance_fraction(shown.balance),
                        balance_look,
                        on_balance,
                        None,
                    )
                    .w(t.px(BALANCE_WIDTH))
                    .h(t.px(SMALL_SLIDER_HEIGHT)),
                ),
        )
}

/// SHUFFLE, REPEAT, a fainter groove, EQ and PL.
fn toggles_row(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    div()
        .h(t.px(TOGGLE_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(t.px(TOGGLE_GAP))
        .child(
            led_toggle(
                "shuffle",
                t!("player.shuffle").into_owned(),
                shown.shuffle,
                t,
            )
            .on_click(|_, _, cx| commands::toggle_shuffle(cx)),
        )
        .child(
            led_toggle("repeat", t!("player.repeat").into_owned(), shown.repeat, t)
                .on_click(|_, _, cx| commands::toggle_repeat(cx)),
        )
        .child(groove(t, 0.6))
        .child(
            led_toggle("toggle-eq", t!("player.eq").into_owned(), shown.eq_shown, t)
                .on_click(|_, _, cx| toggle_visible(PanelId::Equalizer, cx)),
        )
        .child(
            led_toggle(
                "toggle-pl",
                t!("player.pl").into_owned(),
                shown.playlist_shown,
                t,
            )
            .on_click(|_, _, cx| toggle_visible(PanelId::Playlist, cx)),
        )
}

/// Shows or hides a panel.
pub fn toggle_visible(id: PanelId, cx: &mut App) {
    let shell = models(cx).shell.clone();
    shell.update(cx, |shell, cx| shell.toggle_visible(id, cx));
}

/// The player in shade mode: logo, a small clock, the title and the transport.
fn shade_bar(t: &SkinTheme, shown: &Shown, active: bool) -> Div {
    let c = &t.colors;
    let text_size = t.px(SHADE_TEXT_SIZE);
    let mini = |id: &'static str, glyph: SkinIcon| {
        transport_button(id, glyph, SHADE_BUTTON, 8.0, c.text, t)
    };
    let buttons: Vec<AnyElement> = vec![
        window_button("player-minimize", SkinIcon::Minimize, t)
            .on_click(|_, window, _| window.minimize_window())
            .into_any_element(),
        window_button("player-shade", SkinIcon::Shade, t)
            .on_click(|_, _, cx| toggle_shade(PanelId::Player, cx))
            .into_any_element(),
        window_button("player-close", SkinIcon::Close, t)
            .on_click(|_, _, cx| crate::views::quit(cx))
            .into_any_element(),
    ];
    panel(t).child(
        div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .gap(t.px(TITLE_BAR_GAP))
            .pl(t.px(TITLE_BAR_PADDING_LEFT))
            .pr(t.px(TITLE_BAR_PADDING_RIGHT))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(icon(SkinIcon::Logo, t.px(LOGO_GLYPH), c.accent_ink))
            .child(
                div()
                    .px(t.px(5.0))
                    .rounded(t.px(BADGE_RADIUS))
                    .bg(t.lcd_bg())
                    .border_1()
                    .border_color(c.lcd_edge)
                    .text_color(c.lcd_fg)
                    .font_family(t.font_lcd.clone())
                    .text_size(text_size)
                    .whitespace_nowrap()
                    .child(shown.clock.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(text_size)
                    .text_color(if active { c.text } else { c.text_muted })
                    .child(shown.marquee.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap(t.px(2.0))
                    .child(
                        mini("shade-previous", SkinIcon::Previous)
                            .on_click(|_, _, cx| commands::previous(cx)),
                    )
                    .child(
                        mini("shade-play", SkinIcon::Play).on_click(|_, _, cx| commands::play(cx)),
                    )
                    .child(
                        mini("shade-pause", SkinIcon::Pause)
                            .on_click(|_, _, cx| commands::pause(cx)),
                    )
                    .child(
                        mini("shade-stop", SkinIcon::Stop).on_click(|_, _, cx| commands::stop(cx)),
                    )
                    .child(
                        mini("shade-next", SkinIcon::Next).on_click(|_, _, cx| commands::next(cx)),
                    ),
            )
            .child(div().flex().gap(t.px(WINDOW_BUTTON_GAP)).children(buttons)),
    )
}
