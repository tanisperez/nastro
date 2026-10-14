//! The player's display and seek bar: the state glyph and clock, the stream badges, the
//! visualizer, the marquee and the seek bar. Its own view, because it is the only part of the
//! player rebuilt on every frame while playing (it observes the transport's `Frame`).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Bounds, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, TextAlign,
    TextRun, Window, canvas, div, fill, font, point, px, size,
};
use nastro_audio::AnalysisFrame;
use nastro_core::PlayState;
use nastro_dock::PanelId;
use rust_i18n::t;
use std::rc::Rc;

use crate::app::models;
use crate::assets::SkinIcon;
use crate::state::display::{self, MARQUEE_SEPARATOR};
use crate::state::settings::VisualizerMode;
use crate::theme::metrics::*;
use crate::theme::{SkinExt, SkinTheme};
use crate::views::components::slider::{Axis, OnChange, OnRelease, SliderLook, slider};
use crate::views::components::text::icon;

/// How fast the marquee scrolls, in logical pixels per second of playback at 1x.
const MARQUEE_SPEED: f32 = 28.0;

/// The display view.
pub struct Display {
    _subscriptions: Vec<Subscription>,
}

impl Display {
    /// The display, repainting on every frame and when the track, the settings or the
    /// transport change.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let m = models(cx);
        let (transport, library, shell) = (m.transport.clone(), m.library.clone(), m.shell.clone());
        let frame = transport.read(cx).frame.clone();
        Display {
            _subscriptions: vec![
                cx.observe(&frame, |_, _, cx| cx.notify()),
                cx.observe(&transport, |_, _, cx| cx.notify()),
                cx.observe(&library, |_, _, cx| cx.notify()),
                cx.observe(&shell, |_, _, cx| cx.notify()),
            ],
        }
    }
}

impl Render for Display {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.skin();
        let shown = Shown::read(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(t.px(PLAYER_GAP))
            .child(lcd(&t, &shown))
            .child(seek_bar(&t, &shown))
    }
}

/// Everything the player shows, read once per render.
pub(crate) struct Shown {
    pub(crate) state: PlayState,
    pub(crate) clock: String,
    pub(crate) badges: Vec<String>,
    pub(crate) marquee: String,
    pub(crate) marquee_position: f32,
    pub(crate) seek_fraction: f32,
    pub(crate) can_seek: bool,
    pub(crate) volume: f32,
    pub(crate) balance: f32,
    pub(crate) analysis: AnalysisFrame,
    pub(crate) visualizer: VisualizerMode,
    pub(crate) shuffle: bool,
    pub(crate) repeat: bool,
    pub(crate) eq_shown: bool,
    pub(crate) playlist_shown: bool,
    pub(crate) shaded: bool,
}

impl Shown {
    pub(crate) fn read(cx: &App) -> Self {
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
            analysis: transport.frame.read(cx).analysis.clone(),
            visualizer: shell.settings.visualizer,
            shuffle: library.order.shuffle(),
            repeat: library.order.repeat(),
            eq_shown: shell.is_visible(PanelId::Equalizer),
            playlist_shown: shell.is_visible(PanelId::Playlist),
            shaded: shell.is_shaded(PanelId::Player),
        }
    }
}

/// The display: clock, badges and visualizer on top, the marquee below.
pub(crate) fn lcd(t: &SkinTheme, shown: &Shown) -> Div {
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
pub(crate) fn seek_bar(t: &SkinTheme, shown: &Shown) -> impl IntoElement {
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
