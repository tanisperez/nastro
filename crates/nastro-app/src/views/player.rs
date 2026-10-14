//! The player panel: title bar, menu bar, the display (clock, badges, visualizer, marquee),
//! the seek bar, the transport, volume and balance, and the four toggles. In shade mode it
//! collapses to a 24 px bar with a small clock, the title and the transport.

use std::rc::Rc;

use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Bounds, Context, Div, FocusHandle, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, PathBuilder, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, TextAlign, TextRun, Window, canvas, div,
    fill, font, point, px, size,
};
use nastro_audio::AnalysisFrame;
use nastro_core::PlayState;
use nastro_dock::PanelId;
use rust_i18n::t;

use crate::app::{commands, models};
use crate::assets::SkinIcon;
use crate::state::display::{self, MARQUEE_SEPARATOR};
use crate::state::settings::VisualizerMode;
use crate::state::slider::{balance_at, balance_fraction, volume_at};
use crate::theme::metrics::*;
use crate::theme::{SkinExt, SkinTheme};
use crate::views::components::buttons::{led_toggle, transport_button, window_button};
use crate::views::components::frame::{groove, panel, title_bar};
use crate::views::components::menu::MenuTrigger;
use crate::views::components::slider::{Axis, OnChange, OnRelease, SliderLook, slider};
use crate::views::components::text::icon;
use crate::views::menus;

/// How fast the marquee scrolls, in logical pixels per second of playback at 1x.
const MARQUEE_SPEED: f32 = 28.0;

/// The player panel view.
pub struct PlayerPanel {
    /// The root view's focus handle, where menu actions are dispatched.
    root_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl PlayerPanel {
    /// The player panel, repainting when the transport, the playlists or the shell change.
    pub fn new(root_focus: FocusHandle, cx: &mut Context<Self>) -> Self {
        let m = models(cx);
        let (transport, library, shell, eq) = (
            m.transport.clone(),
            m.library.clone(),
            m.shell.clone(),
            m.eq.clone(),
        );
        PlayerPanel {
            root_focus,
            _subscriptions: vec![
                cx.observe(&transport, |_, _, cx| cx.notify()),
                cx.observe(&library, |_, _, cx| cx.notify()),
                cx.observe(&shell, |_, _, cx| cx.notify()),
                cx.observe(&eq, |_, _, cx| cx.notify()),
            ],
        }
    }
}

/// Everything the player shows, read once per render.
struct Shown {
    state: PlayState,
    clock: String,
    badges: Vec<String>,
    marquee: String,
    marquee_position: f32,
    seek_fraction: f32,
    can_seek: bool,
    volume: f32,
    balance: f32,
    analysis: AnalysisFrame,
    visualizer: VisualizerMode,
    shuffle: bool,
    repeat: bool,
    eq_shown: bool,
    playlist_shown: bool,
    shaded: bool,
}

impl Shown {
    fn read(cx: &App) -> Self {
        let m = models(cx);
        let transport = m.transport.read(cx);
        let library = m.library.read(cx);
        let shell = m.shell.read(cx);
        let status = &transport.status;
        let position = transport.shown_position();
        let length = status.duration.or_else(|| {
            library
                .current()
                .and_then(|(_, _, track)| track.meta.duration)
        });
        let marquee = match (&transport.error, library.current()) {
            (Some(error), _) => error.clone(),
            (None, Some((_, index, track))) => display::marquee_text(index, &track.label(), length),
            (None, None) => t!("lcd.idle").into_owned(),
        };
        let seek_fraction = match length {
            Some(length) if !length.is_zero() => transport
                .scrub
                .unwrap_or(position.as_secs_f32() / length.as_secs_f32()),
            _ => 0.0,
        };
        Shown {
            state: status.state,
            clock: display::clock_text(position, length, shell.settings.time_display),
            badges: status
                .info
                .as_ref()
                .map(display::badges)
                .unwrap_or_default(),
            marquee,
            marquee_position: if status.state == PlayState::Stopped {
                0.0
            } else {
                status.position.as_secs_f32()
            },
            seek_fraction,
            can_seek: status.path.is_some() && length.is_some(),
            volume: transport.volume,
            balance: transport.balance,
            analysis: transport.analysis.clone(),
            visualizer: shell.settings.visualizer,
            shuffle: library.order.shuffle(),
            repeat: library.order.repeat(),
            eq_shown: shell.is_visible(PanelId::Equalizer),
            playlist_shown: shell.is_visible(PanelId::Playlist),
            shaded: shell.is_shaded(PanelId::Player),
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
                    .child(lcd(&t, &shown))
                    .child(seek_bar(&t, &shown))
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

/// The display: clock, badges and visualizer on top, the marquee below.
fn lcd(t: &SkinTheme, shown: &Shown) -> Div {
    let c = &t.colors;
    div()
        .h(t.px(LCD_HEIGHT))
        .flex_none()
        .flex()
        .flex_col()
        .gap(t.px(LCD_ROW_GAP))
        .rounded(t.px(LCD_RADIUS))
        .border_1()
        .border_color(c.lcd_edge)
        .bg(t.lcd_bg())
        .shadow(t.lcd_shadow(8.0))
        .pt(t.px(LCD_PADDING_TOP))
        .px(t.px(LCD_PADDING_X))
        .pb(t.px(LCD_PADDING_BOTTOM))
        .text_color(c.lcd_fg)
        .font_family(t.font_lcd.clone())
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .gap(t.px(LCD_COLUMN_GAP))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(clock_row(t, shown))
                        .child(badges_row(t, &shown.badges)),
                )
                .child(visualizer(t, shown)),
        )
        .child(marquee(t, shown))
}

/// The play state glyph and the big clock. A click switches elapsed and remaining.
fn clock_row(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    let glyph = match shown.state {
        PlayState::Playing => SkinIcon::StatePlay,
        PlayState::Paused => SkinIcon::Pause,
        PlayState::Stopped => SkinIcon::StateStop,
    };
    let size = t.px(CLOCK_SIZE);
    div()
        .id("clock")
        .flex()
        .items_center()
        .gap(t.px(STATE_GLYPH_GAP))
        .cursor_pointer()
        .on_click(|_, _, cx| {
            let shell = models(cx).shell.clone();
            shell.update(cx, |shell, cx| shell.toggle_time_display(cx));
        })
        .child(icon(glyph, t.px(STATE_GLYPH), t.colors.lcd_fg))
        .child(
            div()
                .text_size(size)
                .line_height(size)
                .whitespace_nowrap()
                .child(shown.clock.clone()),
        )
}

/// The stream badges: codec, bitrate and sample rate outlined in `lcd_dim`, the channels badge
/// filled with `lcd_fg`.
fn badges_row(t: &SkinTheme, badges: &[String]) -> impl IntoElement {
    let c = &t.colors;
    let last = badges.len().saturating_sub(1);
    div()
        .flex()
        .gap(t.px(BADGE_GAP))
        .text_size(t.px(BADGE_SIZE))
        .line_height(t.px(BADGE_SIZE + 2.0))
        .children(badges.iter().enumerate().map(|(index, badge)| {
            let filled = index == last;
            div()
                .px(t.px(BADGE_PADDING_X))
                .py(t.px(BADGE_PADDING_Y))
                .rounded(t.px(BADGE_RADIUS))
                .whitespace_nowrap()
                .when(filled, |badge| badge.bg(c.lcd_fg).text_color(c.lcd_top))
                .when(!filled, |badge| badge.border_1().border_color(c.lcd_dim))
                .child(badge.clone())
        }))
}

/// The visualizer: spectrum bars with falling peaks, the waveform, or nothing. A click cycles
/// the modes. Painted directly, one quad per lit stripe.
fn visualizer(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    let c = &t.colors;
    let (bar, peak, osc) = (c.vis_bar, c.vis_peak, c.vis_osc);
    let frame = shown.analysis.clone();
    let mode = shown.visualizer;
    let gap = t.px(SPECTRUM_GAP);
    let stripe = t.px(SPECTRUM_STRIPE);
    let lit = stripe - t.px(1.0);
    let line = t.px(1.5);
    div()
        .id("visualizer")
        .flex_1()
        .h(t.px(SPECTRUM_HEIGHT))
        .cursor_pointer()
        .on_click(|_, _, cx| {
            let shell = models(cx).shell.clone();
            shell.update(cx, |shell, cx| {
                let next = shell.settings.visualizer.next();
                shell.set_visualizer(next, cx);
            });
        })
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| match mode {
                    VisualizerMode::Spectrum => {
                        paint_spectrum(bounds, &frame, gap, stripe, lit, bar, peak, window)
                    }
                    VisualizerMode::Oscilloscope => paint_scope(bounds, &frame, line, osc, window),
                    VisualizerMode::Off => {}
                },
            )
            .size_full(),
        )
}

/// Paints the spectrum bars, striped from the bottom up, and a peak line over each.
#[allow(clippy::too_many_arguments)]
fn paint_spectrum(
    bounds: Bounds<Pixels>,
    frame: &AnalysisFrame,
    gap: Pixels,
    stripe: Pixels,
    lit: Pixels,
    bar: Hsla,
    peak: Hsla,
    window: &mut Window,
) {
    let count = frame.bars.len();
    if count == 0 {
        return;
    }
    let width = (bounds.size.width - gap * (count as f32 - 1.0)) / count as f32;
    let height = bounds.size.height;
    for (index, (value, peak_value)) in frame.bars.iter().zip(frame.peaks.iter()).enumerate() {
        let x = bounds.left() + (width + gap) * index as f32;
        let top = bounds.bottom() - height * value.clamp(0.0, 1.0);
        let mut y = bounds.bottom() - lit;
        while y >= top - px(0.5) {
            window.paint_quad(fill(
                Bounds {
                    origin: point(x, y),
                    size: size(width, lit),
                },
                bar,
            ));
            y -= stripe;
        }
        if *peak_value > 0.01 {
            let peak_y = bounds.bottom() - height * peak_value.clamp(0.0, 1.0);
            window.paint_quad(fill(
                Bounds {
                    origin: point(x, peak_y.max(bounds.top())),
                    size: size(width, lit / 2.0),
                },
                peak,
            ));
        }
    }
}

/// Paints the waveform across the visualizer.
fn paint_scope(
    bounds: Bounds<Pixels>,
    frame: &AnalysisFrame,
    line: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let count = frame.scope.len();
    if count < 2 {
        return;
    }
    let middle = bounds.top() + bounds.size.height / 2.0;
    let amplitude = bounds.size.height / 2.0 - line;
    let step = bounds.size.width / (count as f32 - 1.0);
    let mut path = PathBuilder::stroke(line);
    for (index, value) in frame.scope.iter().enumerate() {
        let point = point(
            bounds.left() + step * index as f32,
            middle - amplitude * value.clamp(-1.0, 1.0),
        );
        if index == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// The marquee: the track line under a faint rule, scrolling with the playing position.
fn marquee(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    let c = &t.colors;
    let text: SharedString = format!("{}{MARQUEE_SEPARATOR}", shown.marquee).into();
    let font_size = t.px(MARQUEE_SIZE);
    let line_height = t.px(MARQUEE_SIZE + 3.0);
    let family = t.font_mono.clone();
    let color = c.lcd_fg;
    let position = shown.marquee_position;
    let scale = t.scale;
    let scrolling = shown.state != PlayState::Stopped;
    div()
        .flex_none()
        .border_t_1()
        .border_color(c.lcd_rule)
        .pt(t.px(MARQUEE_PADDING_TOP))
        .h(line_height + t.px(MARQUEE_PADDING_TOP) + px(1.0))
        .overflow_hidden()
        .child(
            canvas(
                move |_, window, _| {
                    let run = TextRun {
                        len: text.len(),
                        font: font(family),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(text, font_size, &[run], None)
                },
                move |bounds, line, window, cx| {
                    let width = f32::from(line.width);
                    let offset = if scrolling {
                        display::marquee_offset(
                            std::time::Duration::from_secs_f32(position),
                            MARQUEE_SPEED * scale,
                            width,
                        )
                    } else {
                        0.0
                    };
                    let mut x = bounds.left() - px(offset);
                    while x < bounds.right() {
                        let _ = line.paint(
                            point(x, bounds.top()),
                            line_height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                        if width <= 0.0 {
                            break;
                        }
                        x += px(width);
                    }
                },
            )
            .w_full()
            .h(line_height),
        )
}

/// The seek bar. Dragging only moves the thumb and the clock; the seek happens on release.
fn seek_bar(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
    let look = SliderLook {
        track: SEEK_TRACK,
        thumb_length: SEEK_THUMB_WIDTH,
        thumb_breadth: SEEK_THUMB_HEIGHT,
        thumb_radius: SEEK_THUMB_HEIGHT / 2.0,
        thumb_inset: 1.0,
        fill: true,
        center_tick: false,
        tick_length: 0.0,
    };
    let can_seek = shown.can_seek;
    let on_change: OnChange = Rc::new(move |fraction, _, cx| {
        if can_seek {
            let transport = models(cx).transport.clone();
            transport.update(cx, |transport, cx| transport.set_scrub(Some(fraction), cx));
        }
    });
    let on_release: OnRelease = Rc::new(|_, cx| {
        let transport = models(cx).transport.clone();
        transport.update(cx, |transport, cx| transport.commit_scrub(cx));
    });
    slider(
        "seek",
        t,
        Axis::Horizontal,
        shown.seek_fraction,
        look,
        on_change,
        Some(on_release),
    )
    .w_full()
    .h(t.px(SEEK_HEIGHT))
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
