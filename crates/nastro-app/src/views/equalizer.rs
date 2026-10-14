//! The equalizer panel: ON, AUTO, PRESETS and RESET on the left, the response curve and the
//! preamp and ten band sliders on the right. In row layouts the panel is as tall as the player
//! and the sliders stretch to fill it.

use std::rc::Rc;

use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Div, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, PathBuilder, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    canvas, div, point, px,
};
use nastro_core::{EQ_BANDS_HZ, EQ_PRESETS, EqSettings};
use nastro_dock::PanelId;
use rust_i18n::t;

use crate::app::models;
use crate::assets::SkinIcon;
use crate::state::slider::{curve_points, gain_at, gain_fraction};
use crate::theme::metrics::*;
use crate::theme::{SkinExt, SkinTheme};
use crate::views::components::buttons::{eq_button, led, window_button};
use crate::views::components::frame::{panel, title_bar};
use crate::views::components::menu::MenuTrigger;
use crate::views::components::slider::{Axis, OnChange, SliderLook, slider};
use crate::views::components::text::{icon, tracked};
use crate::views::dock::draggable;
use crate::views::player::{menu_opened, toggle_shade, toggle_visible};

/// The equalizer panel view.
pub struct EqualizerPanel {
    _root_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EqualizerPanel {
    /// The equalizer panel, repainting when the equalizer or the shell change.
    pub fn new(root_focus: FocusHandle, cx: &mut Context<Self>) -> Self {
        let m = models(cx);
        let (eq, shell) = (m.eq.clone(), m.shell.clone());
        EqualizerPanel {
            _root_focus: root_focus,
            _subscriptions: vec![
                cx.observe(&eq, |_, _, cx| cx.notify()),
                cx.observe(&shell, |_, _, cx| cx.notify()),
            ],
        }
    }
}

/// The labels of the bands: 31, 62, ..., 1k, ..., 16k.
fn band_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{}k", (hz / 1000.0).round())
    } else {
        format!("{}", hz.round())
    }
}

impl Render for EqualizerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.skin();
        let m = models(cx);
        let eq = m.eq.read(cx);
        let shell = m.shell.read(cx);
        let settings = eq.settings;
        let auto = eq.auto;
        let preset = eq.preset.clone();
        let shaded = shell.is_shaded(PanelId::Equalizer);
        // Extra height of a stretched equalizer goes to the sliders.
        let stretch = (shell.equalizer_height() - EQ_HEIGHT).max(0.0);
        let active = window.is_window_active();

        let buttons: Vec<AnyElement> = vec![
            window_button("eq-shade", SkinIcon::Shade, &t)
                .on_click(|_, _, cx| toggle_shade(PanelId::Equalizer, cx))
                .into_any_element(),
            window_button("eq-close", SkinIcon::Close, &t)
                .on_click(|_, _, cx| toggle_visible(PanelId::Equalizer, cx))
                .into_any_element(),
        ];
        let leading = shaded.then(|| led(settings.enabled, &t).into_any_element());
        let title = draggable(
            title_bar(
                &t,
                leading,
                t!("panel.equalizer").into_owned(),
                active,
                buttons,
            )
            .when(shaded, |bar| bar.border_b_0())
            .id("eq-title"),
            PanelId::Equalizer,
        );
        if shaded {
            return panel(&t).child(title).into_any_element();
        }
        panel(&t)
            .child(title)
            .child(
                div()
                    .relative()
                    .flex()
                    .gap(t.px(EQ_COLUMN_GAP))
                    .pt(t.px(EQ_PADDING))
                    .px(t.px(EQ_PADDING))
                    .pb(t.px(EQ_PADDING_BOTTOM))
                    .child(left_column(&t, &settings, auto, preset))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(t.px(EQ_CURVE_GAP))
                            .child(curve(&t, &settings))
                            .child(sliders(&t, &settings, EQ_SLIDER_HEIGHT + stretch)),
                    ),
            )
            .into_any_element()
    }
}

/// ON, AUTO, PRESETS, RESET.
fn left_column(
    t: &Rc<SkinTheme>,
    settings: &EqSettings,
    auto: bool,
    preset: Option<String>,
) -> Div {
    let presets_button = eq_button("eq-presets", t!("eq.presets").into_owned(), None, t)
        .justify_between()
        .child(icon(SkinIcon::Chevron, t.px(8.0), t.colors.text));
    let presets = MenuTrigger::wrap(presets_button, t.clone())
        .dropdown_menu(move |menu, window, cx| presets_menu(menu, preset.clone(), window, cx))
        .on_open_change(menu_opened);
    div()
        .w(t.px(EQ_LEFT_COLUMN))
        .flex_none()
        .flex()
        .flex_col()
        .gap(t.px(EQ_BUTTON_GAP))
        .child(
            eq_button("eq-on", t!("eq.on").into_owned(), Some(settings.enabled), t).on_click(
                |_, _, cx| {
                    let eq = models(cx).eq.clone();
                    eq.update(cx, |eq, cx| eq.toggle_enabled(cx));
                },
            ),
        )
        .child(
            eq_button("eq-auto", t!("eq.auto").into_owned(), Some(auto), t).on_click(|_, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.toggle_auto(cx));
            }),
        )
        .child(presets)
        .child(
            eq_button("eq-reset", t!("eq.reset").into_owned(), None, t).on_click(|_, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.reset(cx));
            }),
        )
}

/// Every preset, the one applied last checked.
fn presets_menu(
    menu: PopupMenu,
    current: Option<String>,
    _: &mut Window,
    _: &mut Context<PopupMenu>,
) -> PopupMenu {
    EQ_PRESETS.iter().fold(menu, |menu, preset| {
        let id = preset.id;
        let checked = current.as_deref() == Some(id);
        let key = format!("eq.preset.{id}");
        menu.item(
            PopupMenuItem::new(rust_i18n::t!(key.as_str()).into_owned())
                .checked(checked)
                .on_click(move |_, _, cx| {
                    let eq = models(cx).eq.clone();
                    eq.update(cx, |eq, cx| eq.apply_preset_id(id, cx));
                }),
        )
    })
}

/// The response curve: a dashed 0 dB line and the bands with the preamp, `lcd_fg` when the
/// equalizer is on and `lcd_dim` when off.
fn curve(t: &SkinTheme, settings: &EqSettings) -> impl IntoElement {
    let c = &t.colors;
    let ink = if settings.enabled {
        c.lcd_fg
    } else {
        c.lcd_dim
    };
    let dim = c.lcd_dim;
    let preamp = settings.preamp_db;
    let bands = settings.bands_db;
    let stroke = t.px(1.6);
    let dash = t.px(3.0);
    div()
        .h(t.px(EQ_CURVE_HEIGHT))
        .flex_none()
        .rounded(t.px(EQ_CURVE_RADIUS))
        .border_1()
        .border_color(c.lcd_edge)
        .bg(t.lcd_bg())
        .shadow(t.lcd_shadow(6.0))
        .py(t.px(3.0))
        .px(t.px(8.0))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let middle = bounds.top() + bounds.size.height / 2.0;
                    let mut x = bounds.left();
                    while x < bounds.right() {
                        window.paint_quad(gpui_kit::fill(
                            gpui_kit::Bounds {
                                origin: point(x, middle),
                                size: gpui_kit::size(dash.min(bounds.right() - x), px(1.0)),
                            },
                            dim,
                        ));
                        x += dash * 2.0;
                    }
                    let points = curve_points(
                        preamp,
                        &bands,
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    );
                    let mut path = PathBuilder::stroke(stroke);
                    for (index, (x, y)) in points.into_iter().enumerate() {
                        let at = point(bounds.left() + px(x), bounds.top() + px(y));
                        if index == 0 {
                            path.move_to(at);
                        } else {
                            path.line_to(at);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ink);
                    }
                },
            )
            .size_full(),
        )
}

/// The preamp, the dB scale and the ten bands.
fn sliders(t: &SkinTheme, settings: &EqSettings, height: f32) -> impl IntoElement {
    let look = SliderLook {
        track: EQ_TRACK,
        thumb_length: EQ_THUMB_HEIGHT,
        thumb_breadth: EQ_THUMB_WIDTH,
        thumb_radius: EQ_THUMB_RADIUS,
        thumb_inset: 1.0,
        fill: false,
        center_tick: false,
        tick_length: 0.0,
    };
    let muted = t.colors.text_muted;
    let label_size = t.px(EQ_LABEL_SIZE);
    let column = |width: f32, slider_element: AnyElement, label: AnyElement| {
        div()
            .w(t.px(width))
            .flex()
            .flex_col()
            .items_center()
            .gap(t.px(EQ_LABEL_GAP))
            .child(slider_element)
            .child(label)
    };
    let on_preamp: OnChange = Rc::new(|fraction, _, cx| {
        let eq = models(cx).eq.clone();
        eq.update(cx, |eq, cx| eq.set_preamp(gain_at(fraction), cx));
    });
    let preamp = column(
        EQ_PREAMP_COLUMN,
        slider(
            "eq-preamp",
            t,
            Axis::Vertical,
            gain_fraction(settings.preamp_db),
            look,
            on_preamp,
            None,
        )
        .w(t.px(EQ_SLIDER_WIDTH))
        .h(t.px(height))
        .into_any_element(),
        tracked(
            t!("eq.preamp").into_owned(),
            label_size,
            0.06,
            FontWeight::SEMIBOLD,
            muted,
        )
        .into_any_element(),
    );
    let scale = div()
        .w(t.px(EQ_SCALE_WIDTH))
        .h(t.px(height))
        .flex()
        .flex_col()
        .justify_between()
        .items_end()
        .font_family(t.font_mono.clone())
        .text_size(label_size)
        .line_height(label_size)
        .text_color(muted)
        .child("+12")
        .child("0")
        .child("-12");
    let bands = settings
        .bands_db
        .iter()
        .zip(EQ_BANDS_HZ)
        .enumerate()
        .map(|(index, (gain, hz))| {
            let on_change: OnChange = Rc::new(move |fraction, _, cx| {
                let eq = models(cx).eq.clone();
                eq.update(cx, |eq, cx| eq.set_band(index, gain_at(fraction), cx));
            });
            column(
                EQ_BAND_COLUMN,
                slider(
                    format!("eq-band-{index}"),
                    t,
                    Axis::Vertical,
                    gain_fraction(*gain),
                    look,
                    on_change,
                    None,
                )
                .w(t.px(EQ_SLIDER_WIDTH))
                .h(t.px(height))
                .into_any_element(),
                div()
                    .font_family(t.font_mono.clone())
                    .text_size(label_size)
                    .line_height(t.px(EQ_LABEL_SIZE + 2.0))
                    .text_color(muted)
                    .child(band_label(hz))
                    .into_any_element(),
            )
        });
    div()
        .flex()
        .items_start()
        .justify_between()
        .child(preamp)
        .child(scale)
        .children(bands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_name_in_every_language() {
        for preset in EQ_PRESETS {
            let key = format!("eq.preset.{}", preset.id);
            for locale in ["en", "es", "gl", "it"] {
                let name = rust_i18n::t!(key.as_str(), locale = locale);
                assert_ne!(name, key, "{key} in {locale}");
            }
        }
    }

    #[test]
    fn band_labels_are_short() {
        let labels: Vec<String> = EQ_BANDS_HZ.iter().map(|hz| band_label(*hz)).collect();
        assert_eq!(
            labels,
            [
                "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"
            ]
        );
    }
}
