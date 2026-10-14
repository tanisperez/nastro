//! Panel geometry for Nastro: layout presets, magnetic snapping, shading and docked groups.
//!
//! The app shows its three panels (player, equalizer, playlist) inside ONE transparent OS
//! window. The app cannot move that window, so everything here lives in window coordinates and
//! never goes negative: a panel dragged past the top or left edge stops at 0, and the window
//! size is simply the bottom-right corner of the displayed panels ([`Layout::bounds`]).
//!
//! This crate is pure geometry on rectangles: no window, no events, no dependencies. The app
//! feeds it pixel positions and applies the rectangles it returns, which keeps every rule
//! testable with plain `cargo test`. All values are logical pixels at 1x; the app multiplies by
//! its scale before calling, or builds a scaled set with [`Metrics::scaled`].
//!
//! Dragging the player moves the whole OS window and is not handled here.

mod layout;
mod metrics;
mod panel;
mod rect;
mod snap;

pub use layout::Layout;
pub use metrics::Metrics;
pub use panel::{LayoutPreset, PanelId, PanelState};
pub use rect::Rect;
