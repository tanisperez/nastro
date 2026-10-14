//! The metal of the skin: a panel's frame with its brushed gradient, the engraved groove of the
//! title bars, and the title bar itself.

use gpui_kit::{
    AnyElement, Bounds, Div, FontWeight, IntoElement, ParentElement, Styled, canvas, div, fill,
    point, px, size,
};

use crate::theme::SkinTheme;
use crate::theme::metrics::{
    BRUSH_PERIOD, GROOVE_HEIGHT, GROOVE_PERIOD, PANEL_RADIUS, PANEL_TITLE_SIZE,
    PANEL_TITLE_TRACKING, TITLE_BAR_GAP, TITLE_BAR_HEIGHT, TITLE_BAR_PADDING_LEFT,
    TITLE_BAR_PADDING_RIGHT,
};
use crate::theme::skin::{black, vertical_gradient, white};

use super::text::tracked;

/// A panel: rounded, with the `frame_edge` border, the window shadow and the brushed metal
/// gradient (`frame_hi` at the top, `frame_mid` at 40 %, `frame_lo` at the bottom). gpui
/// gradients have two stops, so the metal is two stacked gradients.
pub fn panel(t: &SkinTheme) -> Div {
    let c = &t.colors;
    let radius = t.px(PANEL_RADIUS);
    let inner = t.px(PANEL_RADIUS - 1.0);
    div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .rounded(radius)
        .border_1()
        .border_color(c.frame_edge)
        .shadow(t.panel_shadow())
        .font_family(t.font_ui.clone())
        .text_color(c.text)
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(gpui_kit::relative(0.4))
                        .rounded_t(inner)
                        .bg(vertical_gradient(c.frame_hi, c.frame_mid)),
                )
                .child(
                    div()
                        .flex_1()
                        .rounded_b(inner)
                        .bg(vertical_gradient(c.frame_mid, c.frame_lo)),
                ),
        )
        .child(brushed(t))
}

/// The brushed texture: 1 px light stripes every 3 px over a faint dark wash, at the skin's
/// `frame_brush` opacity. Painted directly, no layout per stripe.
fn brushed(t: &SkinTheme) -> impl IntoElement {
    let brush = t.colors.frame_brush;
    let period = t.px(BRUSH_PERIOD);
    let stripe = t.px(1.0);
    let radius = t.px(PANEL_RADIUS);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if brush <= 0.0 {
                return;
            }
            window.paint_quad(fill(bounds, black(brush / 2.0)).corner_radii(radius - px(1.0)));
            // Stripes stay clear of the rounded corners' columns.
            let start = bounds.left() + radius;
            let end = bounds.right() - radius;
            let mut x = start;
            while x < end {
                window.paint_quad(fill(
                    Bounds {
                        origin: point(x, bounds.top()),
                        size: size(stripe, bounds.size.height),
                    },
                    white(brush),
                ));
                x += period;
            }
        },
    )
    .absolute()
    .inset_0()
}

/// The engraved groove: stripes of `groove`, `bevel_hi` and nothing, 3 px apart, filling the
/// free space of a title bar or a toggles row.
pub fn groove(t: &SkinTheme, opacity: f32) -> impl IntoElement {
    let groove = t.colors.groove.opacity(opacity);
    let bevel = t.colors.bevel_hi.opacity(opacity);
    let period = t.px(GROOVE_PERIOD);
    let line = t.px(1.0);
    div().flex_1().h(t.px(GROOVE_HEIGHT)).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let mut y = bounds.top();
                while y < bounds.bottom() {
                    let row = |y| Bounds {
                        origin: point(bounds.left(), y),
                        size: size(bounds.size.width, line),
                    };
                    window.paint_quad(fill(row(y), groove));
                    if y + line < bounds.bottom() {
                        window.paint_quad(fill(row(y + line), bevel));
                    }
                    y += period;
                }
            },
        )
        .size_full(),
    )
}

/// A panel's title bar: optional leading element (the logo), the uppercase title, the groove,
/// and the window buttons. When the window is inactive the title turns `text_muted` and the
/// groove disappears, the only inactive state of the design.
pub fn title_bar(
    t: &SkinTheme,
    leading: Option<AnyElement>,
    title: impl Into<gpui_kit::SharedString>,
    active: bool,
    buttons: impl IntoIterator<Item = AnyElement>,
) -> Div {
    let c = &t.colors;
    let color = if active { c.text } else { c.text_muted };
    div()
        .relative()
        .h(t.px(TITLE_BAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(t.px(TITLE_BAR_GAP))
        .pl(t.px(TITLE_BAR_PADDING_LEFT))
        .pr(t.px(TITLE_BAR_PADDING_RIGHT))
        .border_b_1()
        .border_color(c.groove)
        .children(leading)
        .child(tracked(
            title,
            t.px(PANEL_TITLE_SIZE),
            PANEL_TITLE_TRACKING,
            FontWeight::BOLD,
            color,
        ))
        .child(if active {
            groove(t, 1.0).into_any_element()
        } else {
            div().flex_1().into_any_element()
        })
        .child(
            div()
                .flex()
                .gap(t.px(crate::theme::metrics::WINDOW_BUTTON_GAP))
                .children(buttons),
        )
}
