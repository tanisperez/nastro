//! The skin's buttons. Every one has the normal, hover and pressed looks of the design (the
//! control gradient, its hover pair, and the gradient inverted with an inner shadow while
//! pressed). The window and transport buttons also stop the mouse down from reaching the title
//! or shade bar behind them, which would start moving the window. The others must let it
//! through: a menu trigger (the PRESETS button) opens on its container's mouse down.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Div, ElementId, FontWeight, Hsla, InteractiveElement, MouseButton, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement, Styled, div,
};

use crate::assets::SkinIcon;
use crate::theme::SkinTheme;
use crate::theme::metrics::{
    BUTTON_TRACKING, EQ_BUTTON_HEIGHT, EQ_BUTTON_RADIUS, EQ_BUTTON_SIZE, FOOTER_BUTTON_PADDING,
    FOOTER_BUTTON_RADIUS, FOOTER_HEIGHT, LED_GAP, LED_SIZE, LED_TOGGLE_PADDING_LEFT,
    LED_TOGGLE_PADDING_RIGHT, LED_TOGGLE_SIZE, PLAYLIST_BUTTON_SIZE, PLAYLIST_BUTTON_TRACKING,
    TOGGLE_HEIGHT, WINDOW_BUTTON_HEIGHT, WINDOW_BUTTON_RADIUS, WINDOW_BUTTON_WIDTH, WINDOW_GLYPH,
};

use super::text::{icon, tracked};

/// A raised control: border, control gradient, hover and pressed looks. `drop` adds the soft
/// drop shadow of the transport buttons; flat controls only have the bevel.
pub fn control(id: impl Into<ElementId>, t: &SkinTheme, drop: bool) -> Stateful<Div> {
    let normal = t.control_bg(false, false);
    let hover = t.control_bg(true, false);
    let pressed = t.control_bg(true, true);
    let pressed_shadow = t.bevel_shadow(true);
    let shadow = if drop {
        t.control_shadow(false)
    } else {
        t.bevel_shadow(false)
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(t.colors.frame_edge)
        .bg(normal)
        .shadow(shadow)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(pressed).shadow(pressed_shadow.clone()))
}

/// Keeps a press on `button` from reaching a bar behind it that moves the window.
fn stops_press(button: Stateful<Div>) -> Stateful<Div> {
    button.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// A window button of a title bar (minimize, shade, close): 16 x 14, radius 3, 8 px glyph.
pub fn window_button(id: impl Into<ElementId>, glyph: SkinIcon, t: &SkinTheme) -> Stateful<Div> {
    stops_press(control(id, t, false))
        .w(t.px(WINDOW_BUTTON_WIDTH))
        .h(t.px(WINDOW_BUTTON_HEIGHT))
        .rounded(t.px(WINDOW_BUTTON_RADIUS))
        .child(icon(glyph, t.px(WINDOW_GLYPH), t.colors.text))
}

/// A round transport button of `diameter`, its icon of `icon_size` in `ink`.
pub fn transport_button(
    id: impl Into<ElementId>,
    glyph: SkinIcon,
    diameter: f32,
    icon_size: f32,
    ink: Hsla,
    t: &SkinTheme,
) -> Stateful<Div> {
    stops_press(control(id, t, true))
        .size(t.px(diameter))
        .rounded_full()
        .child(icon(glyph, t.px(icon_size), ink))
}

/// The 6 px LED of a toggle: `accent` with its glow when on, `led_off` otherwise.
pub fn led(on: bool, t: &SkinTheme) -> Div {
    let c = &t.colors;
    div()
        .size(t.px(LED_SIZE))
        .flex_none()
        .rounded_full()
        .bg(if on { c.accent } else { c.led_off })
        .when(on, |led| led.shadow(t.led_glow()))
}

/// A pill toggle with a LED: SHUFFLE, REPEAT, EQ, PL.
pub fn led_toggle(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    t: &SkinTheme,
) -> Stateful<Div> {
    control(id, t, false)
        .h(t.px(TOGGLE_HEIGHT))
        .rounded(t.px(TOGGLE_HEIGHT / 2.0))
        .pl(t.px(LED_TOGGLE_PADDING_LEFT))
        .pr(t.px(LED_TOGGLE_PADDING_RIGHT))
        .gap(t.px(LED_GAP))
        .child(led(on, t))
        .child(tracked(
            label,
            t.px(LED_TOGGLE_SIZE),
            BUTTON_TRACKING,
            FontWeight::SEMIBOLD,
            t.colors.text,
        ))
}

/// A button of the equalizer's left column, with a LED when `led` is given.
pub fn eq_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    led_on: Option<bool>,
    t: &SkinTheme,
) -> Stateful<Div> {
    control(id, t, false)
        .w_full()
        .h(t.px(EQ_BUTTON_HEIGHT))
        .rounded(t.px(EQ_BUTTON_RADIUS))
        .px(t.px(8.0))
        .gap(t.px(6.0))
        .justify_start()
        .children(led_on.map(|on| led(on, t)))
        .child(tracked(
            label,
            t.px(EQ_BUTTON_SIZE),
            BUTTON_TRACKING,
            FontWeight::SEMIBOLD,
            t.colors.text,
        ))
}

/// A text button of the playlist footer: ADD, REMOVE, SELECT, SORT.
pub fn text_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    t: &SkinTheme,
) -> Stateful<Div> {
    control(id, t, false)
        .h(t.px(FOOTER_HEIGHT))
        .rounded(t.px(FOOTER_BUTTON_RADIUS))
        .px(t.px(FOOTER_BUTTON_PADDING))
        .child(tracked(
            label,
            t.px(PLAYLIST_BUTTON_SIZE),
            PLAYLIST_BUTTON_TRACKING,
            FontWeight::SEMIBOLD,
            t.colors.text,
        ))
}
