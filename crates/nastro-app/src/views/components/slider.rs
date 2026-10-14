//! The skin's sliders: seek, volume, balance and the equalizer bands. One element for all of
//! them, horizontal or vertical, drawn from the design's track, fill and thumb.
//!
//! A press jumps to the pointer and a drag follows it; both report the new fraction (0..=1, from
//! the left or from the bottom) through `on_change`. `on_release` runs when the button is let go
//! after a press on this slider, wherever the pointer is, which is when the seek bar seeks.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::AppContext as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Bounds, Context, Div, DragMoveEvent, Empty, Global, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, Render, SharedString, Stateful, StatefulInteractiveElement,
    Styled, Window, canvas, div, relative,
};

use crate::state::slider::fraction_at;
use crate::theme::SkinTheme;

/// Which way a slider runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Bottom to top.
    Vertical,
}

/// How a slider looks, in logical pixels at 1x (scaled by the skin).
#[derive(Debug, Clone, Copy)]
pub struct SliderLook {
    /// Thickness of the track.
    pub track: f32,
    /// Size of the thumb along the slider.
    pub thumb_length: f32,
    /// Size of the thumb across the slider.
    pub thumb_breadth: f32,
    /// Radius of the thumb.
    pub thumb_radius: f32,
    /// Offset of the thumb from the slider's edge across it (1 px in the design).
    pub thumb_inset: f32,
    /// Whether the track fills with `accent` up to the thumb.
    pub fill: bool,
    /// Whether a tick marks the centre (balance).
    pub center_tick: bool,
    /// Length of the centre tick across the slider.
    pub tick_length: f32,
}

/// The drag payload of a slider, carrying its id so only that slider follows the pointer.
#[derive(Clone)]
pub struct SliderDrag(pub SharedString);

/// What a slider drag shows under the pointer: nothing.
pub struct SliderDragView;

impl Render for SliderDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// The slider pressed last and not released yet.
#[derive(Default)]
struct PressedSlider(Option<SharedString>);

impl Global for PressedSlider {}

/// A change callback: the new fraction.
pub type OnChange = Rc<dyn Fn(f32, &mut Window, &mut App)>;
/// A release callback.
pub type OnRelease = Rc<dyn Fn(&mut Window, &mut App)>;

/// A slider at `fraction`. The caller sets its size.
pub fn slider(
    id: impl Into<SharedString>,
    t: &SkinTheme,
    axis: Axis,
    fraction: f32,
    look: SliderLook,
    on_change: OnChange,
    on_release: Option<OnRelease>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let fraction = fraction.clamp(0.0, 1.0);
    let thumb_length = t.px(look.thumb_length);
    let bounds_cell: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));

    let to_fraction = move |bounds: Bounds<Pixels>, position: gpui_kit::Point<Pixels>| {
        let thumb = f32::from(thumb_length);
        match axis {
            Axis::Horizontal => fraction_at(
                f32::from(position.x),
                f32::from(bounds.left()),
                f32::from(bounds.size.width),
                thumb,
            ),
            Axis::Vertical => fraction_at(
                f32::from(bounds.bottom() - position.y),
                0.0,
                f32::from(bounds.size.height),
                thumb,
            ),
        }
    };

    let press = {
        let bounds_cell = bounds_cell.clone();
        let on_change = on_change.clone();
        let id = id.clone();
        move |event: &gpui_kit::MouseDownEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            cx.default_global::<PressedSlider>().0 = Some(id.clone());
            if let Some(bounds) = bounds_cell.get() {
                on_change(to_fraction(bounds, event.position), window, cx);
            }
        }
    };
    let drag_move = {
        let on_change = on_change.clone();
        let id = id.clone();
        move |event: &DragMoveEvent<SliderDrag>, window: &mut Window, cx: &mut App| {
            if event.drag(cx).0 == id {
                on_change(to_fraction(event.bounds, event.event.position), window, cx);
            }
        }
    };
    let release = {
        let id = id.clone();
        move |_: &gpui_kit::MouseUpEvent, window: &mut Window, cx: &mut App| {
            let pressed = cx.default_global::<PressedSlider>();
            if pressed.0.as_ref() == Some(&id) {
                pressed.0 = None;
                if let Some(on_release) = &on_release {
                    on_release(window, cx);
                }
            }
        }
    };

    let element_id = id.clone();
    div()
        .id(element_id)
        .relative()
        .flex_none()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, press)
        .on_drag(SliderDrag(id), |_, _, _, cx| cx.new(|_| SliderDragView))
        .on_drag_move(drag_move)
        .on_mouse_up(MouseButton::Left, release.clone())
        .on_mouse_up_out(MouseButton::Left, release)
        .child(
            canvas(
                move |bounds, _, _| bounds_cell.set(Some(bounds)),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .child(track(t, axis, look))
        .when(look.fill, |slider| {
            slider.child(fill_bar(t, axis, look, fraction))
        })
        .when(look.center_tick, |slider| {
            slider.child(center_tick(t, axis, look))
        })
        .child(thumb(t, axis, look, fraction))
}

/// The recessed track, centred across the slider.
fn track(t: &SkinTheme, axis: Axis, look: SliderLook) -> Div {
    let thickness = t.px(look.track);
    let base = div()
        .absolute()
        .rounded(thickness / 2.0)
        .bg(t.colors.slot)
        .shadow(t.slot_shadow());
    match axis {
        Axis::Horizontal => base
            .left_0()
            .right_0()
            .top(relative(0.5))
            .mt(-thickness / 2.0)
            .h(thickness),
        Axis::Vertical => base
            .top_0()
            .bottom_0()
            .left(relative(0.5))
            .ml(-thickness / 2.0)
            .w(thickness),
    }
}

/// The `accent` fill from the start of the track to `fraction`.
fn fill_bar(t: &SkinTheme, axis: Axis, look: SliderLook, fraction: f32) -> Div {
    let thickness = t.px(look.track);
    let base = div()
        .absolute()
        .rounded(thickness / 2.0)
        .bg(t.colors.accent);
    match axis {
        Axis::Horizontal => base
            .left_0()
            .top(relative(0.5))
            .mt(-thickness / 2.0)
            .h(thickness)
            .w(relative(fraction)),
        Axis::Vertical => base
            .bottom_0()
            .left(relative(0.5))
            .ml(-thickness / 2.0)
            .w(thickness)
            .h(relative(fraction)),
    }
}

/// The tick at the centre of the balance.
fn center_tick(t: &SkinTheme, axis: Axis, look: SliderLook) -> Div {
    let length = t.px(look.tick_length);
    let base = div().absolute().bg(t.colors.text_muted);
    match axis {
        Axis::Horizontal => base
            .left(relative(0.5))
            .top(relative(0.5))
            .mt(-length / 2.0)
            .w(t.px(1.0))
            .h(length),
        Axis::Vertical => base
            .top(relative(0.5))
            .left(relative(0.5))
            .ml(-length / 2.0)
            .h(t.px(1.0))
            .w(length),
    }
}

/// The thumb at `fraction`, never leaving the track: it sits in a box one thumb shorter than
/// the slider, positioned by a relative offset.
fn thumb(t: &SkinTheme, axis: Axis, look: SliderLook, fraction: f32) -> Div {
    let length = t.px(look.thumb_length);
    let breadth = t.px(look.thumb_breadth);
    let inset = t.px(look.thumb_inset);
    let mut shadow = t.bevel_shadow(false);
    shadow.extend(t.control_shadow(false).into_iter().skip(1));
    let knob = div()
        .absolute()
        .rounded(t.px(look.thumb_radius))
        .border_1()
        .border_color(t.colors.frame_edge)
        .bg(t.control_bg(false, false))
        .shadow(shadow);
    match axis {
        Axis::Horizontal => div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .right(length)
            .child(
                knob.left(relative(fraction))
                    .top(relative(0.5))
                    .mt(-breadth / 2.0)
                    .w(length)
                    .h(breadth),
            ),
        Axis::Vertical => div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .top(length)
            .child(
                knob.bottom(relative(fraction))
                    .left(inset)
                    .w(breadth)
                    .h(length),
            ),
    }
}
