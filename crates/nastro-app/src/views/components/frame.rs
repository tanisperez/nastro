//! The metal of the skin: a panel's frame with its brushed gradient, the engraved groove of the
//! title bars, and the title bar itself.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{
    AnyElement, Bounds, Div, FontWeight, IntoElement, ParentElement, RenderImage, Styled, canvas,
    div, fill, point, size,
};

use crate::theme::SkinTheme;
use crate::theme::metrics::{
    BRUSH_PERIOD, GROOVE_HEIGHT, GROOVE_PERIOD, PANEL_RADIUS, PANEL_TITLE_SIZE,
    PANEL_TITLE_TRACKING, TITLE_BAR_GAP, TITLE_BAR_HEIGHT, TITLE_BAR_PADDING_LEFT,
    TITLE_BAR_PADDING_RIGHT,
};
use crate::theme::skin::vertical_gradient;

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
/// `frame_brush` opacity. It is one image per panel width (a single row of pixels stretched to
/// the panel's height) painted with the panel's rounded corners: a quad per stripe made every
/// frame of a playing player cost a few percent of a core.
fn brushed(t: &SkinTheme) -> impl IntoElement {
    let brush = t.colors.frame_brush;
    let scale = t.scale;
    let radius = t.px(PANEL_RADIUS - 1.0);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if brush <= 0.0 {
                return;
            }
            let device_scale = window.scale_factor();
            let width = (f32::from(bounds.size.width) * device_scale).round() as u32;
            let Some(image) = brushed_row(width, brush, scale * device_scale) else {
                return;
            };
            let _ = window.paint_image(bounds, bounds, radius.into(), image, 0, false);
        },
    )
    .absolute()
    .inset_0()
}

/// The images made by [`brushed_row`], by width, opacity and scale.
type BrushedCache = HashMap<(u32, u32, u32), Arc<RenderImage>>;

thread_local! {
    // Images live on the UI thread only. A handful of entries: one per panel width and skin.
    static BRUSHED: RefCell<BrushedCache> = RefCell::new(HashMap::new());
}

/// The brushed row `width` device pixels wide, made once and cached. `pixel_scale` is device
/// pixels per design pixel (the skin scale times the display scale).
fn brushed_row(width: u32, brush: f32, pixel_scale: f32) -> Option<Arc<RenderImage>> {
    if width == 0 {
        return None;
    }
    let key = (width, brush.to_bits(), pixel_scale.to_bits());
    BRUSHED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(image) = cache.get(&key) {
            return Some(image.clone());
        }
        if cache.len() > 32 {
            cache.clear();
        }
        let bytes = brushed_pixels(width, brush, pixel_scale);
        let buffer = image::RgbaImage::from_raw(width, 1, bytes)?;
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
        cache.insert(key, image.clone());
        Some(image)
    })
}

/// The pixels of a brushed row in gpui's image layout (BGRA, straight alpha): a dark wash at
/// half the opacity everywhere, with a light stripe of one design pixel every three on top.
fn brushed_pixels(width: u32, brush: f32, pixel_scale: f32) -> Vec<u8> {
    let wash = brush / 2.0;
    // White at `brush` over black at `wash`, composited once.
    let stripe_alpha = brush + wash * (1.0 - brush);
    let stripe_gray = if stripe_alpha > 0.0 {
        brush / stripe_alpha
    } else {
        0.0
    };
    let period = BRUSH_PERIOD * pixel_scale;
    let mut bytes = Vec::with_capacity(width as usize * 4);
    for x in 0..width {
        let position = (x as f32 + 0.5) % period;
        let (gray, alpha) = if position < pixel_scale {
            (stripe_gray, stripe_alpha)
        } else {
            (0.0, wash)
        };
        let gray = (gray * 255.0).round() as u8;
        bytes.extend_from_slice(&[gray, gray, gray, (alpha * 255.0).round() as u8]);
    }
    bytes
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brushed_rows_stripe_every_three_design_pixels() {
        let bytes = brushed_pixels(9, 0.05, 1.0);
        let alphas: Vec<u8> = bytes.chunks(4).map(|pixel| pixel[3]).collect();
        let stripe = alphas[0];
        let wash = alphas[1];
        assert!(stripe > wash);
        assert_eq!(
            alphas,
            vec![stripe, wash, wash, stripe, wash, wash, stripe, wash, wash]
        );
        // At 2x a stripe is two device pixels wide.
        let doubled: Vec<u8> = brushed_pixels(6, 0.05, 2.0)
            .chunks(4)
            .map(|p| p[3])
            .collect();
        assert_eq!(doubled, vec![stripe, stripe, wash, wash, wash, wash]);
    }
}
