//! Named sizes of the skin, in logical pixels at the 1x skin scale, from the design handoff
//! (`docs/design-system.md`, "Metrics"). Views read sizes only from here and always through
//! `SkinTheme::px`, which applies the skin scale, never as a literal.
//!
//! A few constants describe sizes the design defines for states not drawn yet, so this file
//! allows dead code for itself, like Postino's.
#![allow(dead_code)]

// Panels.

/// Width of the player and the equalizer.
pub const PANEL_WIDTH: f32 = 520.0;
/// Corner radius of every panel.
pub const PANEL_RADIUS: f32 = 8.0;
/// Border of every panel, in `frame_edge`.
pub const PANEL_BORDER: f32 = 1.0;
/// Height of a panel's title bar, its groove line included.
pub const TITLE_BAR_HEIGHT: f32 = 24.0;
/// Left padding of a title bar.
pub const TITLE_BAR_PADDING_LEFT: f32 = 10.0;
/// Right padding of a title bar.
pub const TITLE_BAR_PADDING_RIGHT: f32 = 6.0;
/// Gap between the items of a title bar.
pub const TITLE_BAR_GAP: f32 = 8.0;
/// Height of the engraved groove that fills a title bar.
pub const GROOVE_HEIGHT: f32 = 7.0;
/// Period of the groove's stripes.
pub const GROOVE_PERIOD: f32 = 3.0;
/// Period of the brushed metal stripes.
pub const BRUSH_PERIOD: f32 = 3.0;
/// Height of the player's menu bar.
pub const MENU_BAR_HEIGHT: f32 = 22.0;
/// Height of a menu bar button.
pub const MENU_BUTTON_HEIGHT: f32 = 18.0;
/// Horizontal padding of a menu bar button.
pub const MENU_BUTTON_PADDING: f32 = 7.0;
/// Radius of a menu bar button.
pub const MENU_BUTTON_RADIUS: f32 = 3.0;
/// Height of every panel in shade mode.
pub const SHADE_HEIGHT: f32 = 24.0;
/// Room around the docked group inside the transparent window, for the panel shadows.
pub const WINDOW_MARGIN: f32 = 16.0;

// Player, 520 x 274.

/// Height of the player.
pub const PLAYER_HEIGHT: f32 = 274.0;
/// Top padding of the player's body, under the menu bar.
pub const PLAYER_PADDING_TOP: f32 = 2.0;
/// Side and bottom padding of the player's body.
pub const PLAYER_PADDING: f32 = 10.0;
/// Gap between the rows of the player's body.
pub const PLAYER_GAP: f32 = 8.0;
/// Height of the display.
pub const LCD_HEIGHT: f32 = 118.0;
/// Corner radius of the display.
pub const LCD_RADIUS: f32 = 6.0;
/// Top padding of the display.
pub const LCD_PADDING_TOP: f32 = 10.0;
/// Side padding of the display.
pub const LCD_PADDING_X: f32 = 12.0;
/// Bottom padding of the display.
pub const LCD_PADDING_BOTTOM: f32 = 7.0;
/// Gap between the display's top row and the marquee.
pub const LCD_ROW_GAP: f32 = 6.0;
/// Gap between the clock column and the visualizer.
pub const LCD_COLUMN_GAP: f32 = 16.0;
/// Height of the visualizer.
pub const SPECTRUM_HEIGHT: f32 = 66.0;
/// Gap between the visualizer's bars.
pub const SPECTRUM_GAP: f32 = 2.0;
/// Period of a bar's stripes: 2 px lit, 1 px dark.
pub const SPECTRUM_STRIPE: f32 = 3.0;
/// Font size of the clock.
pub const CLOCK_SIZE: f32 = 46.0;
/// Size of the play state glyph left of the clock.
pub const STATE_GLYPH: f32 = 14.0;
/// Gap between the state glyph and the clock.
pub const STATE_GLYPH_GAP: f32 = 10.0;
/// Font size of a display badge.
pub const BADGE_SIZE: f32 = 10.0;
/// Gap between badges.
pub const BADGE_GAP: f32 = 5.0;
/// Horizontal padding of a badge.
pub const BADGE_PADDING_X: f32 = 5.0;
/// Vertical padding of a badge.
pub const BADGE_PADDING_Y: f32 = 1.0;
/// Radius of a badge.
pub const BADGE_RADIUS: f32 = 3.0;
/// Font size of the marquee.
pub const MARQUEE_SIZE: f32 = 12.0;
/// Space above the marquee, under its rule.
pub const MARQUEE_PADDING_TOP: f32 = 5.0;
/// Height of the seek bar row.
pub const SEEK_HEIGHT: f32 = 16.0;
/// Height of the seek track.
pub const SEEK_TRACK: f32 = 6.0;
/// Width of the seek thumb.
pub const SEEK_THUMB_WIDTH: f32 = 28.0;
/// Height of the seek thumb.
pub const SEEK_THUMB_HEIGHT: f32 = 12.0;
/// Height of the transport row (the play button).
pub const TRANSPORT_HEIGHT: f32 = 38.0;
/// Diameter of a transport button.
pub const TRANSPORT_BUTTON: f32 = 30.0;
/// Diameter of the play button.
pub const PLAY_BUTTON: f32 = 38.0;
/// Gap between transport buttons.
pub const TRANSPORT_GAP: f32 = 4.0;
/// Size of a transport icon.
pub const TRANSPORT_ICON: f32 = 13.0;
/// Size of the play icon.
pub const PLAY_ICON: f32 = 16.0;
/// Size of the stop icon.
pub const STOP_ICON: f32 = 12.0;
/// Width of the open button.
pub const OPEN_BUTTON_WIDTH: f32 = 26.0;
/// Height of the open button.
pub const OPEN_BUTTON_HEIGHT: f32 = 22.0;
/// Radius of the open button.
pub const OPEN_BUTTON_RADIUS: f32 = 5.0;
/// Size of the open and volume icons.
pub const SMALL_ICON: f32 = 12.0;
/// Size of the volume icon.
pub const VOLUME_ICON: f32 = 14.0;
/// Gap between a slider and its icon or label.
pub const SLIDER_LABEL_GAP: f32 = 6.0;
/// Width of the volume slider.
pub const VOLUME_WIDTH: f32 = 86.0;
/// Width of the balance slider.
pub const BALANCE_WIDTH: f32 = 46.0;
/// Height of the volume and balance sliders' hit area.
pub const SMALL_SLIDER_HEIGHT: f32 = 14.0;
/// Height of the volume and balance tracks.
pub const SMALL_TRACK: f32 = 4.0;
/// Width of the volume thumb.
pub const VOLUME_THUMB_WIDTH: f32 = 14.0;
/// Width of the balance thumb.
pub const BALANCE_THUMB_WIDTH: f32 = 12.0;
/// Height of the volume and balance thumbs.
pub const SMALL_THUMB_HEIGHT: f32 = 12.0;
/// Radius of the volume and balance thumbs.
pub const SMALL_THUMB_RADIUS: f32 = 4.0;
/// Height of the balance's centre tick.
pub const BALANCE_TICK: f32 = 8.0;
/// Height of the toggles row.
pub const TOGGLE_HEIGHT: f32 = 18.0;
/// Gap between the toggles.
pub const TOGGLE_GAP: f32 = 6.0;
/// Diameter of a toggle's LED.
pub const LED_SIZE: f32 = 6.0;
/// Gap between a LED and its label.
pub const LED_GAP: f32 = 5.0;
/// Left padding of a LED toggle.
pub const LED_TOGGLE_PADDING_LEFT: f32 = 6.0;
/// Right padding of a LED toggle.
pub const LED_TOGGLE_PADDING_RIGHT: f32 = 8.0;

// Window buttons.

/// Width of a window button (minimize, shade, close).
pub const WINDOW_BUTTON_WIDTH: f32 = 16.0;
/// Height of a window button.
pub const WINDOW_BUTTON_HEIGHT: f32 = 14.0;
/// Radius of a window button.
pub const WINDOW_BUTTON_RADIUS: f32 = 3.0;
/// Gap between window buttons.
pub const WINDOW_BUTTON_GAP: f32 = 3.0;
/// Size of a window button's glyph.
pub const WINDOW_GLYPH: f32 = 8.0;
/// Size of the logo glyph in the player's title bar.
pub const LOGO_GLYPH: f32 = 12.0;

// Equalizer, 520 x 207.

/// Height of the equalizer.
pub const EQ_HEIGHT: f32 = 207.0;
/// Top and side padding of the equalizer's body.
pub const EQ_PADDING: f32 = 10.0;
/// Bottom padding of the equalizer's body.
pub const EQ_PADDING_BOTTOM: f32 = 12.0;
/// Width of the left column (ON, AUTO, PRESETS, RESET).
pub const EQ_LEFT_COLUMN: f32 = 84.0;
/// Gap between the left column and the bands.
pub const EQ_COLUMN_GAP: f32 = 12.0;
/// Gap between the buttons of the left column.
pub const EQ_BUTTON_GAP: f32 = 6.0;
/// Height of an equalizer button.
pub const EQ_BUTTON_HEIGHT: f32 = 22.0;
/// Radius of an equalizer button.
pub const EQ_BUTTON_RADIUS: f32 = 5.0;
/// Height of the curve display.
pub const EQ_CURVE_HEIGHT: f32 = 40.0;
/// Radius of the curve display.
pub const EQ_CURVE_RADIUS: f32 = 4.0;
/// Gap between the curve and the sliders.
pub const EQ_CURVE_GAP: f32 = 8.0;
/// Height of a band slider at the design size; it grows when the equalizer is stretched.
pub const EQ_SLIDER_HEIGHT: f32 = 96.0;
/// Width of a band column.
pub const EQ_BAND_COLUMN: f32 = 26.0;
/// Width of the preamp column.
pub const EQ_PREAMP_COLUMN: f32 = 28.0;
/// Width of a band slider's hit area.
pub const EQ_SLIDER_WIDTH: f32 = 22.0;
/// Width of a band slider's track.
pub const EQ_TRACK: f32 = 4.0;
/// Width of a band slider's thumb.
pub const EQ_THUMB_WIDTH: f32 = 20.0;
/// Height of a band slider's thumb.
pub const EQ_THUMB_HEIGHT: f32 = 9.0;
/// Radius of a band slider's thumb.
pub const EQ_THUMB_RADIUS: f32 = 3.0;
/// Gap between a band slider and its label.
pub const EQ_LABEL_GAP: f32 = 4.0;
/// Font size of the frequency labels and the dB scale.
pub const EQ_LABEL_SIZE: f32 = 9.0;
/// Width of the dB scale column.
pub const EQ_SCALE_WIDTH: f32 = 22.0;

// Playlist, default 600 x 487.

/// Default width of the playlist.
pub const PLAYLIST_WIDTH: f32 = 600.0;
/// Minimum width of the playlist.
pub const PLAYLIST_MIN_WIDTH: f32 = 520.0;
/// Default height of the playlist.
pub const PLAYLIST_HEIGHT: f32 = 487.0;
/// Top padding of the playlist's body.
pub const PLAYLIST_PADDING_TOP: f32 = 8.0;
/// Side and bottom padding of the playlist's body.
pub const PLAYLIST_PADDING: f32 = 10.0;
/// Gap between the rows of the playlist's body.
pub const PLAYLIST_GAP: f32 = 6.0;
/// Height of a tab and of the filter field.
pub const TAB_HEIGHT: f32 = 22.0;
/// Horizontal padding of a tab.
pub const TAB_PADDING: f32 = 10.0;
/// Radius of a tab's top corners.
pub const TAB_RADIUS: f32 = 5.0;
/// Gap between tabs.
pub const TAB_GAP: f32 = 2.0;
/// Side of the new playlist button.
pub const NEW_TAB_BUTTON: f32 = 20.0;
/// Radius of the new playlist button.
pub const NEW_TAB_RADIUS: f32 = 4.0;
/// Radius of the filter field.
pub const FILTER_RADIUS: f32 = 11.0;
/// Horizontal padding of the filter field.
pub const FILTER_PADDING: f32 = 9.0;
/// Width of the filter's text input.
pub const FILTER_INPUT_WIDTH: f32 = 120.0;
/// Size of the filter's search icon.
pub const FILTER_ICON: f32 = 11.0;
/// Height of a track row.
pub const ROW_HEIGHT: f32 = 22.0;
/// Horizontal padding of a track row.
pub const ROW_PADDING: f32 = 10.0;
/// Gap between the columns of a track row.
pub const ROW_GAP: f32 = 8.0;
/// Width of the index column, right aligned.
pub const INDEX_COLUMN: f32 = 22.0;
/// Size of the playing glyph in the index column.
pub const PLAYING_GLYPH: f32 = 9.0;
/// Vertical padding inside the track list.
pub const LIST_PADDING: f32 = 3.0;
/// Radius of the track list's box.
pub const LIST_RADIUS: f32 = 4.0;
/// Height of the footer and its buttons.
pub const FOOTER_HEIGHT: f32 = 20.0;
/// Horizontal padding of a footer button.
pub const FOOTER_BUTTON_PADDING: f32 = 9.0;
/// Radius of a footer button.
pub const FOOTER_BUTTON_RADIUS: f32 = 4.0;
/// Gap between footer buttons.
pub const FOOTER_GAP: f32 = 4.0;
/// Side of the resize grip at the playlist's bottom right corner.
pub const RESIZE_GRIP: f32 = 12.0;
/// Everything in the playlist except its rows: borders, title bar, paddings, tabs, gaps, the
/// list's own padding and border, the footer.
pub const PLAYLIST_CHROME_HEIGHT: f32 = 2.0 * PANEL_BORDER
    + TITLE_BAR_HEIGHT
    + PLAYLIST_PADDING_TOP
    + TAB_HEIGHT
    + 2.0 * PLAYLIST_GAP
    + 2.0 * (LIST_PADDING + 1.0)
    + FOOTER_HEIGHT
    + PLAYLIST_PADDING;

// Shade bars.

/// Width of the clock in the player's shade bar.
pub const SHADE_CLOCK_WIDTH: f32 = 44.0;
/// Font size of the shade bar's clock and title.
pub const SHADE_TEXT_SIZE: f32 = 11.0;
/// Diameter of the transport buttons in the player's shade bar.
pub const SHADE_BUTTON: f32 = 16.0;

// Focus.

/// Width of the focus ring.
pub const FOCUS_RING: f32 = 2.0;

// Type scale (font sizes, in px at 1x).

/// Panel title, uppercase with 0.18 em tracking.
pub const PANEL_TITLE_SIZE: f32 = 10.0;
/// Panel title tracking, in em.
pub const PANEL_TITLE_TRACKING: f32 = 0.18;
/// Menu bar.
pub const MENU_SIZE: f32 = 11.0;
/// Equalizer buttons, uppercase with 0.1 em tracking.
pub const EQ_BUTTON_SIZE: f32 = 10.0;
/// Equalizer and LED button tracking, in em.
pub const BUTTON_TRACKING: f32 = 0.1;
/// LED toggles and the BAL label.
pub const LED_TOGGLE_SIZE: f32 = 9.0;
/// Playlist buttons, uppercase with 0.06 em tracking.
pub const PLAYLIST_BUTTON_SIZE: f32 = 10.0;
/// Playlist button tracking, in em.
pub const PLAYLIST_BUTTON_TRACKING: f32 = 0.06;
/// Tabs.
pub const TAB_SIZE: f32 = 11.0;
/// Playlist rows.
pub const ROW_SIZE: f32 = 12.0;
/// Playlist numbers: index, duration, summary.
pub const ROW_NUMBER_SIZE: f32 = 11.0;
/// Badge tracking, in em.
pub const BADGE_TRACKING: f32 = 0.04;
