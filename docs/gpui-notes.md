# Working with gpui-kit

Findings about `gpui-kit` 0.7.0 and `gpui-pre` 0.3.7 that are not obvious from their docs.
Read this before touching windows, rendering performance, menus, sliders or themes. Postino's
`docs/gpui-notes.md` has more (dialogs, code editors, charts); the parts that apply here are
repeated below.

`gpui` changes quickly. Read the real source under `~/.cargo/registry/src/*/` (`gpui-pre-0.3.7`,
`gpui-component-0.7.0`, `gpui-base-0.7.0`, `gpui-pre-linux-0.3.7`) instead of trusting memory.
Note that `gpui-kit = "0.7.0"` in a fresh lockfile resolves to a newer 0.7.x that pins a newer
`gpui-pre`; Nastro's `Cargo.lock` started from Postino's to keep 0.7.0 and 0.3.7.

## The transparent main window

- `WindowOptions { window_background: WindowBackgroundAppearance::Transparent, titlebar: None,
  window_decorations: Some(WindowDecorations::Client), .. }` gives a frameless window whose
  unpainted pixels are transparent, on Wayland and on X11 (gpui picks a 32-bit visual).
- Do not wrap the main window in gpui-kit's `Root`. Its `WindowState` plugin paints the theme
  background over the whole surface, and its `window_border` decoration, with client side
  decorations on Linux, adds a 20 px shadow inset, a border and resize edges. Without `Root`,
  `Input` still works (it calls `WindowState::try_update` and tolerates its absence), but
  tooltips and gpui-kit dialogs do not appear in that window.
- `Window::resize(size)` sets the content size. On Wayland the top left corner stays put. On
  X11 under KWin the window may be moved when it is resized, so do not trust screen positions
  across a resize when testing on XWayland.
- `Window::set_input_region(Some(&rects))` (Wayland only, a no-op elsewhere) makes clicks
  outside the rectangles reach the windows behind. `None` restores the whole window. Popups
  drawn inside the window (menus) are subject to it, so lift it while one is open.
- `Window::start_window_move()` on mouse down hands the drag to the compositor. Events may stop
  arriving during the move, so do not expect a mouse up afterwards.
- `WindowOptions::app_owns_titlebar_drag` matters only on macOS; set it for windows that move
  themselves.

## Rendering cost

- `cx.notify()` on any entity repaints the window, and by default every view in it renders
  again. Wrap child views in `.cached(style)` (`AnyView::cached`, `Entity::cached`) so they
  are rebuilt only when they notify; the style must give the view its size.
- Observing an entity that notifies 30 times per second makes the observer render 30 times
  per second. Put per-frame data in a small entity of its own (`Transport::frame`) and make only
  the view that draws it observe it.
- `canvas(prepaint, paint)` paints quads (`window.paint_quad(fill(bounds, color))`), paths
  (`PathBuilder::stroke`, `window.paint_path`) and shaped text (`window.text_system()
  .shape_line(..)` then `line.paint(..)`) with no layout per item: use it for stripes, bars and
  the scrolling marquee. A parent with `overflow_hidden` clips what a canvas paints.
- `uniform_list(id, count, cx.processor(|view, range, window, cx| ..))` builds only the visible
  rows. Rows need `.w_full()`, or a `flex_1` child does not stretch.

## Styling

- Gradients have two stops (`linear_gradient(angle, from, to)`, 180 is top to bottom). A
  three-stop gradient is two stacked elements.
- `BoxShadow` has an `inset` flag in this version. Its `blur_radius` is the gaussian's standard
  deviation, half of a CSS blur.
- There is no letter spacing and no text shadow. Letter spacing can be faked for short labels
  with one box per character and a gap.
- gpui clips children to rectangles, not to rounded corners: a child painting into a rounded
  parent's corner must round itself.
- `relative(fraction)` works for `left`, `bottom`, `w` and `h`: a thumb that must stay inside
  its track goes in a box one thumb shorter than the track, at `left(relative(fraction))`.

## Menus

- `gpui_kit::component::menu::DropdownMenu` works on any element that is `Styled`,
  `InteractiveElement`, `StatefulInteractiveElement`, `Selectable` and `IntoElement`. A skinned
  trigger is a small `RenderOnce` struct wrapping a `Stateful<Div>` (`MenuTrigger`).
- The popover opens on its container's mouse down in the bubble phase and stops propagation
  there. So the trigger itself must not stop propagation (or it never opens), and a parent that
  starts a window move on mouse down is not reached when a trigger is pressed.
- The menu is built when it opens, so checks and dynamic items are current. Items made with
  `menu(label, action)` dispatch the action from `action_context(focus_handle)` and show its key
  binding; `PopupMenuItem::new(label).checked(..).on_click(..)` is for items without an action.
  Submenus open to the left when there is no room on the right.
- `DropdownMenuPopover::on_open_change` reports opening and closing.

## Sliders and drags

- `on_drag(value, |value, offset, window, cx| cx.new(|_| EmptyView))` starts a drag after the
  pointer moves a few pixels; `offset` is the pointer position inside the element at that
  moment. Store it in an `Rc<Cell<..>>` inside the payload if the move handler needs it.
- `on_drag_move::<T>` fires for every element registered for `T`, wherever the pointer is;
  `event.bounds` is the registering element's bounds. Filter by an id in the payload.
- A mouse down handler gets no element bounds. Capture them with an invisible `canvas` child
  whose prepaint writes them into an `Rc<Cell<Option<Bounds>>>`.
- `on_mouse_up` plus `on_mouse_up_out` catch the release wherever it happens; remember which
  slider was pressed (a global) so only that one reacts.

## Async

- `cx.spawn(async move |this, cx| ..)`; `AsyncApp::update` returns the closure's value
  directly in this version, not a `Result`. `WeakEntity::update` does return a `Result`.
- Timers: `cx.background_executor().timer(duration).await`. Dropping a `Task` cancels it; a
  task must not drop its own handle, so keep a flag instead.
- Background work: `cx.background_executor().spawn(async move { .. })`, awaited from a
  foreground task.

## Themes, fonts, icons

- Load the chrome theme family after `gpui_kit::init`, then assign the light and dark configs to
  `Theme::global_mut(cx).light_theme` and `.dark_theme` yourself (`theme::install`).
- `cx.text_system().add_fonts(..)` registers bundled TTFs.
- `svg().path(..)` draws a monochrome SVG tinted by `text_color`. The asset source
  (`assets::Assets`) serves the skin icons under `nastro/` and falls back to gpui-kit's curated
  icons, which the popup menus need for checks and submenu chevrons.
- A single-line `Input` needs `.py_0()` or its descenders are clipped; `.appearance(false)`
  removes its own box so a skinned field can draw one.

## Testing the UI by hand on Linux

- Run the binary under XWayland (`WAYLAND_DISPLAY=` empty) to drive it with `xdotool` and
  capture it with `spectacle -b -n -a`. Native Wayland renders sharper and is what users get,
  but cannot be driven by `xdotool`.
- On a scaled KWin output, a screenshot pixel `P` of the window is at X11 position `X + P * f`
  (`f` = X11 window width / logical width), and KWin multiplies absolute warp requests by `f`,
  so move the pointer to `(X + P * f) / f`. Relative moves are not scaled.
- Another session can share the same pointer: coordinate before driving the desktop.
- Use an isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_DATA_HOME`, so tests never
  touch the real settings, session and playlists. An isolated `XDG_RUNTIME_DIR` keeps the test
  instance off the real `nastro.sock`, but it must be short (a socket path is limited to 108
  bytes) and must link the real `wayland-0`, `pipewire-0` and `pulse` sockets, or there is no
  window on Wayland and no sound anywhere.
