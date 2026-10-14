//! About, an app chrome window: the icon, the version, the license and the credits.

use gpui_kit::AppContext as _;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, FontWeight, Global, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, img, px, size,
};
use rust_i18n::t;

use crate::theme::{CHROME_FONT, PaletteExt};

/// The project's source code. Provisional, like the repository name.
const SOURCE_URL: &str = "https://github.com/tanisperez/nastro";

/// The open About window.
#[derive(Default)]
struct AboutWindow(Option<AnyWindowHandle>);

impl Global for AboutWindow {}

/// Opens About, or brings it to the front.
pub fn open(cx: &mut App) {
    if let Some(handle) = cx.default_global::<AboutWindow>().0
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(400.0), px(460.0)),
            cx,
        ))),
        is_resizable: false,
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(crate::APP_ID.to_string()),
        ..TitleBar::window_options()
    };
    let opened = cx.open_window(options, |window, cx| {
        window.set_window_title(&t!("about.title"));
        crate::views::apply_chrome_theme_to(window, cx);
        let view = cx.new(|_| AboutView);
        cx.new(|cx| Root::new(view, window, cx))
    });
    match opened {
        Ok(handle) => cx.default_global::<AboutWindow>().0 = Some(handle.into()),
        Err(error) => log::warn!("could not open About: {error}"),
    }
}

/// The About view.
pub struct AboutView;

impl Render for AboutView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.palette();
        let link = |id: &'static str, label: String, url: &'static str| {
            let hover = palette.fg;
            div()
                .id(id)
                .text_color(palette.accent_text)
                .cursor_pointer()
                .hover(move |style| style.text_color(hover))
                .on_click(move |_, _, cx| cx.open_url(url))
                .child(label)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.bg)
            .font_family(SharedString::from(CHROME_FONT))
            .text_color(palette.fg)
            .child(TitleBar::new())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(28.0))
                    .pt(px(16.0))
                    .child(img(crate::assets::APP_ICON).size(px(96.0)))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Nastro"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(palette.fg_muted)
                            .child(
                                t!("about.version", version = env!("CARGO_PKG_VERSION"))
                                    .into_owned(),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_center()
                            .child(t!("about.tagline").into_owned()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(16.0))
                            .text_size(px(13.0))
                            .child(link(
                                "about-guide",
                                t!("menu.help.user_guide").into_owned(),
                                crate::views::USER_GUIDE_URL,
                            ))
                            .child(link(
                                "about-source",
                                t!("about.source").into_owned(),
                                SOURCE_URL,
                            )),
                    )
                    .child(
                        div()
                            .mt(px(8.0))
                            .text_size(px(12.0))
                            .text_center()
                            .text_color(palette.fg_muted)
                            .child(t!("about.license").into_owned()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_center()
                            .text_color(palette.fg_subtle)
                            .child(t!("about.credits").into_owned()),
                    ),
            )
    }
}
