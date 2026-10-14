//! Preferences, an app chrome window: language, the chrome theme, the skin and its scale, the
//! clock, the visualizer and the log level. Every choice applies and saves at once.

use gpui_kit::AppContext as _;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, FontWeight, Global, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowBounds, WindowDecorations, WindowOptions, div, px, size,
};
use rust_i18n::t;

use crate::app::models;
use crate::app::shell::Shell;
use crate::state::locale::{self, LanguageChoice};
use crate::state::settings::{LogLevel, SkinScale, ThemeChoice, TimeDisplay, VisualizerMode};
use crate::theme::{CHROME_FONT, PaletteExt};

/// The open Preferences window, so a second request brings it to the front.
#[derive(Default)]
struct PreferencesWindow(Option<AnyWindowHandle>);

impl Global for PreferencesWindow {}

/// Opens Preferences, or brings it to the front.
pub fn open(cx: &mut App) {
    if let Some(handle) = cx.default_global::<PreferencesWindow>().0
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(600.0), px(560.0)),
            cx,
        ))),
        window_min_size: Some(size(px(480.0), px(400.0))),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(crate::APP_ID.to_string()),
        ..TitleBar::window_options()
    };
    let opened = cx.open_window(options, |window, cx| {
        window.set_window_title(&t!("preferences.title"));
        crate::views::apply_chrome_theme_to(window, cx);
        let view = cx.new(PreferencesView::new);
        cx.new(|cx| Root::new(view, window, cx))
    });
    match opened {
        Ok(handle) => cx.default_global::<PreferencesWindow>().0 = Some(handle.into()),
        Err(error) => log::warn!("could not open Preferences: {error}"),
    }
}

/// The Preferences view.
pub struct PreferencesView {
    _subscriptions: Vec<Subscription>,
}

impl PreferencesView {
    fn new(cx: &mut Context<Self>) -> Self {
        let shell = models(cx).shell.clone();
        PreferencesView {
            _subscriptions: vec![cx.observe(&shell, |_, _, cx| cx.notify())],
        }
    }
}

/// What choosing an option does to the settings.
type Apply = Box<dyn Fn(&mut Shell, &mut Context<Shell>)>;

/// One option of a choice row.
struct Choice {
    label: String,
    selected: bool,
    apply: Apply,
}

impl Render for PreferencesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.palette();
        let shell = models(cx).shell.read(cx);
        let settings = shell.settings.clone();
        let skins: Vec<(String, String)> = shell
            .skins
            .iter()
            .map(|skin| (skin.id.clone(), skin.name.clone()))
            .collect();

        let detected = locale::detect(locale::system_locales().iter().cloned());
        let languages: Vec<Choice> = LanguageChoice::ALL
            .into_iter()
            .map(|choice| Choice {
                label: match choice.language() {
                    Some(language) => language.native_name().to_string(),
                    None => format!(
                        "{} ({})",
                        t!("preferences.language_auto"),
                        detected.native_name()
                    ),
                },
                selected: settings.language == choice,
                apply: Box::new(move |shell, cx| shell.set_language(choice, cx)),
            })
            .collect();
        let themes: Vec<Choice> = ThemeChoice::ALL
            .into_iter()
            .map(|choice| Choice {
                label: choice.label(),
                selected: settings.theme == choice,
                apply: Box::new(move |shell, cx| shell.set_theme(choice, cx)),
            })
            .collect();
        let skin_choices: Vec<Choice> = skins
            .into_iter()
            .map(|(id, name)| Choice {
                label: name,
                selected: settings.skin == id,
                apply: Box::new(move |shell, cx| shell.set_skin(&id, cx)),
            })
            .collect();
        let scales: Vec<Choice> = SkinScale::ALL
            .into_iter()
            .map(|scale| Choice {
                label: scale.label(),
                selected: settings.skin_scale == scale,
                apply: Box::new(move |shell, cx| shell.set_scale(scale, cx)),
            })
            .collect();
        let clocks: Vec<Choice> = [TimeDisplay::Elapsed, TimeDisplay::Remaining]
            .into_iter()
            .map(|mode| Choice {
                label: match mode {
                    TimeDisplay::Elapsed => t!("preferences.clock_elapsed").into_owned(),
                    TimeDisplay::Remaining => t!("preferences.clock_remaining").into_owned(),
                },
                selected: settings.time_display == mode,
                apply: Box::new(move |shell, cx| shell.set_time_display(mode, cx)),
            })
            .collect();
        let visualizers: Vec<Choice> = VisualizerMode::ALL
            .into_iter()
            .map(|mode| Choice {
                label: mode.label(),
                selected: settings.visualizer == mode,
                apply: Box::new(move |shell, cx| shell.set_visualizer(mode, cx)),
            })
            .collect();
        let levels: Vec<Choice> = LogLevel::ALL
            .into_iter()
            .map(|level| Choice {
                label: level.label(),
                selected: settings.log_level == level,
                apply: Box::new(move |shell, cx| shell.set_log_level(level, cx)),
            })
            .collect();

        let section =
            |id: &'static str, title: String, hint: Option<String>, choices: Vec<Choice>| {
                let palette = palette.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.fg_subtle)
                            .child(title.to_uppercase()),
                    )
                    .child(div().flex().flex_wrap().gap(px(6.0)).children(
                        choices.into_iter().enumerate().map(|(index, choice)| {
                            let hover = palette.hover;
                            let apply = choice.apply;
                            div()
                                .id((id, index))
                                .h(px(28.0))
                                .px(px(12.0))
                                .flex()
                                .items_center()
                                .rounded(px(6.0))
                                .border_1()
                                .text_size(px(13.0))
                                .cursor_pointer()
                                .when(choice.selected, |chip| {
                                    chip.bg(palette.accent_subtle)
                                        .border_color(palette.accent)
                                        .text_color(palette.accent_text)
                                        .font_weight(FontWeight::MEDIUM)
                                })
                                .when(!choice.selected, |chip| {
                                    chip.border_color(palette.border_strong)
                                        .text_color(palette.fg)
                                        .hover(move |style| style.bg(hover))
                                })
                                .on_click(move |_, _, cx| {
                                    let shell = models(cx).shell.clone();
                                    shell.update(cx, |shell, cx| apply(shell, cx));
                                })
                                .child(choice.label)
                        }),
                    ))
                    .children(hint.map(|hint| {
                        div()
                            .text_size(px(12.0))
                            .text_color(palette.fg_muted)
                            .child(hint)
                    }))
            };

        let skins_folder = crate::state::session::user_skins_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        let log_file = crate::logging::log_path()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let settings_file = crate::state::settings::settings_path()
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.bg)
            .font_family(SharedString::from(CHROME_FONT))
            .text_color(palette.fg)
            .child(
                TitleBar::new().child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t!("preferences.title").into_owned()),
                ),
            )
            .child(
                div()
                    .id("preferences-body")
                    .flex_1()
                    .overflow_y_scroll()
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .child(section(
                        "language",
                        t!("preferences.language").into_owned(),
                        None,
                        languages,
                    ))
                    .child(section(
                        "theme",
                        t!("preferences.theme").into_owned(),
                        Some(t!("preferences.theme_hint").into_owned()),
                        themes,
                    ))
                    .child(section(
                        "skin",
                        t!("preferences.skin").into_owned(),
                        Some(t!("preferences.skin_hint", folder = skins_folder).into_owned()),
                        skin_choices,
                    ))
                    .child(section(
                        "scale",
                        t!("preferences.skin_scale").into_owned(),
                        None,
                        scales,
                    ))
                    .child(section(
                        "clock",
                        t!("preferences.clock").into_owned(),
                        None,
                        clocks,
                    ))
                    .child(section(
                        "visualizer",
                        t!("menu.options.visualizer").into_owned(),
                        None,
                        visualizers,
                    ))
                    .child(section(
                        "log",
                        t!("preferences.log_level").into_owned(),
                        Some(t!("preferences.log_hint", file = log_file).into_owned()),
                        levels,
                    ))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(palette.fg_subtle)
                            .child(t!("preferences.saved_to", file = settings_file).into_owned()),
                    ),
            )
    }
}
