//! Saving the session: a burst of changes (dragging the volume, moving a panel) is written once,
//! [`SAVE_DELAY`] after the last change, and everything is written again on quit.

use std::time::Duration;

use gpui_kit::{App, Global};

use crate::app::models;
use crate::state::session::{Session, save_session};

/// How long after the last change the session is written.
const SAVE_DELAY: Duration = Duration::from_secs(1);

/// The number of the latest scheduled save; an older timer that fires does nothing.
#[derive(Default)]
struct PendingSave(u64);

impl Global for PendingSave {}

/// Writes the session [`SAVE_DELAY`] from now, unless another change comes first.
pub fn schedule(cx: &mut App) {
    let epoch = {
        let pending = cx.default_global::<PendingSave>();
        pending.0 += 1;
        pending.0
    };
    cx.spawn(async move |cx| {
        cx.background_executor().timer(SAVE_DELAY).await;
        cx.update(|cx| {
            if cx
                .try_global::<PendingSave>()
                .is_some_and(|pending| pending.0 == epoch)
            {
                save_now(cx);
            }
        });
    })
    .detach();
}

/// The session as the models hold it now, with the main window's position when known.
pub fn snapshot(cx: &App, window_origin: Option<(f32, f32)>) -> Option<Session> {
    let models = cx.try_global::<crate::app::Models>()?;
    let transport = models.transport.read(cx);
    let library = models.library.read(cx);
    let eq = models.eq.read(cx);
    let shell = models.shell.read(cx);
    let (playing_playlist, current_track) = match library.current() {
        Some((tab, index, _)) => (Some(tab), Some(index)),
        None => (None, None),
    };
    Some(Session {
        volume: transport.volume,
        balance: transport.balance,
        shuffle: library.order.shuffle(),
        repeat: library.order.repeat(),
        eq: eq.settings,
        eq_auto: eq.auto,
        eq_preset: eq.preset.clone(),
        layout: Some(shell.layout.clone()),
        playlists: library.files(),
        active_playlist: library.active,
        playing_playlist,
        current_track,
        window_origin,
    })
}

/// Where the main window is on screen. `None` on Wayland, where it is never known (gpui reports
/// the origin as zero there), and when the main window is closed.
fn main_window_origin(cx: &mut App) -> Option<(f32, f32)> {
    let handle = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<crate::views::window::NastroWindow>())?;
    let origin = handle
        .update(cx, |_, window, _| window.bounds().origin)
        .ok()?;
    let (x, y) = (f32::from(origin.x), f32::from(origin.y));
    (x != 0.0 || y != 0.0).then_some((x, y))
}

/// Writes the session now.
pub fn save_now(cx: &mut App) {
    let origin = main_window_origin(cx);
    if let Some(session) = snapshot(cx, origin) {
        save_session(&session);
        log::debug!("session saved");
    }
}

/// Writes everything before quitting: the session and every unsaved playlist.
pub fn save_all(cx: &mut App) {
    save_now(cx);
    let library = models(cx).library.clone();
    library.update(cx, |library, _| library.save_now());
}
