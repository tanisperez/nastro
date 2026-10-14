//! Text helpers of the skin: labels with letter spacing and the skin icons.

use gpui_kit::{Div, FontWeight, Hsla, ParentElement, Pixels, SharedString, Styled, Svg, div, svg};

use crate::assets::SkinIcon;

/// An uppercase label with letter spacing, as the design's panel titles and buttons have.
/// gpui has no letter spacing, so each character is its own box with a gap of `tracking` em.
/// Only for short, static labels: a dozen characters at most.
pub fn tracked(
    text: impl Into<SharedString>,
    size: Pixels,
    tracking: f32,
    weight: FontWeight,
    color: Hsla,
) -> Div {
    let text: SharedString = text.into();
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(size * tracking)
        .text_size(size)
        .line_height(size * 1.2)
        .font_weight(weight)
        .text_color(color)
        .whitespace_nowrap()
        .children(
            text.chars()
                .map(|character| div().child(SharedString::from(character.to_string()))),
        )
}

/// A skin icon at `size`, in `color`.
pub fn icon(icon: SkinIcon, size: Pixels, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(size)
        .flex_none()
        .text_color(color)
}
