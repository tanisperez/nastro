//! What the main window needs from each platform beyond gpui's own API.

#[cfg(windows)]
mod win32;

use gpui_kit::{Bounds, Edges, Pixels, Window};

/// Limits where the main window takes input to `panels` (window coordinates), so a click
/// anywhere else reaches the window behind; `None` gives the whole window back. `shadow` is how
/// far the panels paint past their bounds, kept where the limit also clips drawing.
///
/// On Wayland it is gpui's input region, which does not clip drawing. On Windows it is a window
/// region, which does, so it covers the panels with their shadows. On X11 and macOS it does
/// nothing yet: the whole window takes input.
pub fn limit_input(window: &Window, panels: Option<&[Bounds<Pixels>]>, shadow: Edges<Pixels>) {
    #[cfg(windows)]
    win32::set_window_region(window, panels, shadow);
    #[cfg(not(windows))]
    {
        // Only a region that clips drawing needs the shadows.
        let _ = shadow;
        window.set_input_region(panels);
    }
}
