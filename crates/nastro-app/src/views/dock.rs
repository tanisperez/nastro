//! Dragging panels inside the window. The player's title bar moves the OS window itself
//! (`start_window_move`); the equalizer's and the playlist's title bars move that panel alone,
//! snapping magnetically to the others through `nastro-dock`, and the playlist's corner grip
//! resizes it.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::AppContext as _;
use gpui_kit::Div;
use gpui_kit::{
    App, Context, DragMoveEvent, Empty, InteractiveElement, IntoElement, Pixels, Point, Render,
    Stateful, StatefulInteractiveElement, Window,
};
use nastro_dock::PanelId;

use crate::app::models;

/// The payload of a panel drag: which panel, and where in its title bar it was grabbed.
#[derive(Clone)]
pub struct PanelDrag {
    id: PanelId,
    grab: Rc<Cell<Point<Pixels>>>,
}

/// The payload of a playlist resize.
#[derive(Clone)]
pub struct ResizeDrag;

/// What a panel drag shows under the pointer: nothing, the panel itself moves.
pub struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Makes `title` (an element with an id) drag panel `id` around the window.
pub fn draggable(title: Stateful<Div>, id: PanelId) -> Stateful<Div> {
    let drag = PanelDrag {
        id,
        grab: Rc::new(Cell::new(Point::default())),
    };
    title
        .on_drag(drag, |drag, offset, _, cx| {
            drag.grab.set(offset);
            cx.new(|_| DragGhost)
        })
        .on_drag_move(
            move |event: &DragMoveEvent<PanelDrag>, _: &mut Window, cx: &mut App| {
                let drag = event.drag(cx);
                if drag.id != id {
                    return;
                }
                let grab = drag.grab.get();
                let shell = models(cx).shell.clone();
                shell.update(cx, |shell, cx| {
                    let scale = shell.scale();
                    let margin = shell.margin();
                    // The title bar sits inside the panel's 1 px border.
                    let x = (f32::from(event.event.position.x - grab.x) - margin - 1.0) / scale;
                    let y = (f32::from(event.event.position.y - grab.y) - margin - 1.0) / scale;
                    shell.move_panel(id, x, y, cx);
                });
            },
        )
}

/// Makes `grip` resize the playlist to follow the pointer.
pub fn resize_grip(grip: Stateful<Div>) -> Stateful<Div> {
    grip.on_drag(ResizeDrag, |_, _, _, cx| cx.new(|_| DragGhost))
        .on_drag_move(
            |event: &DragMoveEvent<ResizeDrag>, _: &mut Window, cx: &mut App| {
                let shell = models(cx).shell.clone();
                shell.update(cx, |shell, cx| {
                    let Some(bounds) = shell.panel_bounds(PanelId::Playlist) else {
                        return;
                    };
                    let scale = shell.scale();
                    let width = f32::from(event.event.position.x - bounds.origin.x) / scale;
                    let height = f32::from(event.event.position.y - bounds.origin.y) / scale;
                    shell.resize_playlist(width, height, cx);
                });
            },
        )
}
