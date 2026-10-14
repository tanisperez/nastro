//! The menus of the player's menu bar. They are app chrome: gpui-kit popup menus in the fixed
//! chrome palette, never in skin colors. Each is built when it opens, so checks and the skin
//! list are always current. Items with a shortcut dispatch an action, so the menu shows the
//! key next to the label.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{Context, Window};
use nastro_dock::{LayoutPreset, PanelId};
use rust_i18n::t;

use crate::actions::*;
use crate::app::models;
use crate::state::settings::{SkinScale, TimeDisplay, VisualizerMode};

/// File: open and add files, playlists, Preferences and Quit.
pub fn file_menu(menu: PopupMenu, _: &mut Window, _: &mut Context<PopupMenu>) -> PopupMenu {
    menu.menu(t!("menu.file.open_files"), Box::new(OpenFiles))
        .menu(t!("menu.file.add_files"), Box::new(AddFiles))
        .menu(t!("menu.file.add_folder"), Box::new(AddFolder))
        .separator()
        .menu(t!("menu.file.new_playlist"), Box::new(NewPlaylist))
        .menu(t!("menu.file.rename_playlist"), Box::new(RenamePlaylist))
        .menu(t!("menu.file.export_playlist"), Box::new(ExportPlaylist))
        .menu(t!("menu.file.close_playlist"), Box::new(ClosePlaylist))
        .separator()
        .menu(t!("menu.file.preferences"), Box::new(OpenPreferences))
        .separator()
        .menu(t!("menu.file.quit"), Box::new(Quit))
}

/// Play: the transport, seeking, shuffle and repeat.
pub fn play_menu(menu: PopupMenu, _: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let order = &models(cx).library.read(cx).order;
    let (shuffle, repeat) = (order.shuffle(), order.repeat());
    menu.menu(t!("menu.play.play"), Box::new(Play))
        .menu(t!("menu.play.pause"), Box::new(Pause))
        .menu(t!("menu.play.stop"), Box::new(Stop))
        .menu(t!("menu.play.previous"), Box::new(Previous))
        .menu(t!("menu.play.next"), Box::new(Next))
        .separator()
        .menu(t!("menu.play.seek_forward"), Box::new(SeekForward))
        .menu(t!("menu.play.seek_backward"), Box::new(SeekBackward))
        .separator()
        .menu_with_check(t!("menu.play.shuffle"), shuffle, Box::new(ToggleShuffle))
        .menu_with_check(t!("menu.play.repeat"), repeat, Box::new(ToggleRepeat))
}

/// Options: skin, skin scale, clock, visualizer, Preferences.
pub fn options_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let shell = models(cx).shell.read(cx);
    let remaining = shell.settings.time_display == TimeDisplay::Remaining;
    menu.submenu(t!("menu.options.skin"), window, cx, skin_menu)
        .submenu(t!("menu.options.skin_scale"), window, cx, scale_menu)
        .submenu(t!("menu.options.visualizer"), window, cx, visualizer_menu)
        .menu_with_check(
            t!("menu.options.remaining_time"),
            remaining,
            Box::new(ToggleTimeDisplay),
        )
        .separator()
        .menu(t!("menu.file.preferences"), Box::new(OpenPreferences))
}

/// The skins on offer, the active one checked, then the user skins folder.
fn skin_menu(menu: PopupMenu, _: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let shell = models(cx).shell.read(cx);
    let active = shell.settings.skin.clone();
    let skins: Vec<(String, String)> = shell
        .skins
        .iter()
        .map(|skin| (skin.id.clone(), skin.name.clone()))
        .collect();
    let mut menu = menu;
    for (id, name) in skins {
        let checked = id == active;
        menu = menu.item(
            PopupMenuItem::new(name)
                .checked(checked)
                .on_click(move |_, _, cx| {
                    let shell = models(cx).shell.clone();
                    let id = id.clone();
                    shell.update(cx, |shell, cx| shell.set_skin(&id, cx));
                }),
        );
    }
    menu.separator()
        .item(
            PopupMenuItem::new(t!("menu.options.reload_skins")).on_click(|_, _, cx| {
                let shell = models(cx).shell.clone();
                shell.update(cx, |shell, cx| shell.reload_skins(cx));
            }),
        )
        .menu(
            t!("menu.options.open_skins_folder"),
            Box::new(OpenSkinsFolder),
        )
}

/// 1x, 1.5x, 2x.
fn scale_menu(menu: PopupMenu, _: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let current = models(cx).shell.read(cx).settings.skin_scale;
    let action = |scale: SkinScale| -> Box<dyn gpui_kit::Action> {
        match scale {
            SkinScale::X1 => Box::new(SkinScale1),
            SkinScale::X1_5 => Box::new(SkinScale15),
            SkinScale::X2 => Box::new(SkinScale2),
        }
    };
    SkinScale::ALL.into_iter().fold(menu, |menu, scale| {
        menu.menu_with_check(scale.label(), scale == current, action(scale))
    })
}

/// Spectrum, oscilloscope, off.
fn visualizer_menu(menu: PopupMenu, _: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let current = models(cx).shell.read(cx).settings.visualizer;
    VisualizerMode::ALL.into_iter().fold(menu, |menu, mode| {
        menu.item(
            PopupMenuItem::new(mode.label())
                .checked(mode == current)
                .on_click(move |_, _, cx| {
                    let shell = models(cx).shell.clone();
                    shell.update(cx, |shell, cx| shell.set_visualizer(mode, cx));
                }),
        )
    })
}

/// View: which panels are shown and collapsed, and the layouts.
pub fn view_menu(menu: PopupMenu, window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let shell = models(cx).shell.read(cx);
    let eq = shell.is_visible(PanelId::Equalizer);
    let playlist = shell.is_visible(PanelId::Playlist);
    let shaded = [
        shell.is_shaded(PanelId::Player),
        shell.is_shaded(PanelId::Equalizer),
        shell.is_shaded(PanelId::Playlist),
    ];
    menu.menu_with_check(
        t!("menu.view.equalizer"),
        eq,
        Box::new(ToggleEqualizerPanel),
    )
    .menu_with_check(
        t!("menu.view.playlist"),
        playlist,
        Box::new(TogglePlaylistPanel),
    )
    .separator()
    .submenu(t!("menu.view.layout"), window, cx, layout_menu)
    .separator()
    .menu_with_check(
        t!("menu.view.shade_player"),
        shaded[0],
        Box::new(ShadePlayer),
    )
    .menu_with_check(
        t!("menu.view.shade_equalizer"),
        shaded[1],
        Box::new(ShadeEqualizer),
    )
    .menu_with_check(
        t!("menu.view.shade_playlist"),
        shaded[2],
        Box::new(ShadePlaylist),
    )
}

/// The four layout presets.
fn layout_menu(menu: PopupMenu, _: &mut Window, _: &mut Context<PopupMenu>) -> PopupMenu {
    LayoutPreset::ALL.into_iter().fold(menu, |menu, preset| {
        let (label, action): (_, Box<dyn gpui_kit::Action>) = match preset {
            LayoutPreset::A => (t!("menu.view.layout_a"), Box::new(LayoutA)),
            LayoutPreset::B => (t!("menu.view.layout_b"), Box::new(LayoutB)),
            LayoutPreset::C => (t!("menu.view.layout_c"), Box::new(LayoutC)),
            LayoutPreset::D => (t!("menu.view.layout_d"), Box::new(LayoutD)),
        };
        menu.menu(label, action)
    })
}

/// Help: the user guide and About.
pub fn help_menu(menu: PopupMenu, _: &mut Window, _: &mut Context<PopupMenu>) -> PopupMenu {
    menu.menu(t!("menu.help.user_guide"), Box::new(OpenUserGuide))
        .separator()
        .menu(t!("menu.help.about"), Box::new(OpenAbout))
}
