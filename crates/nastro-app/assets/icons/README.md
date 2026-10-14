# Skin icons

The hand-drawn icons of the design (`docs/design-system.md`, "Icons"), drawn on a 16 px grid
(8 px for the window buttons, the plus and the chevron, 12 px for the logo bars), embedded by
`src/assets.rs` and rendered with gpui's `svg()`, which uses only their shape: the color always
comes from the skin through `text_color`, so the fills and strokes here are plain black.

`state-play.svg` and `state-stop.svg` are the play state glyphs of the display, slightly smaller
than the transport icons. `grip.svg` marks the playlist's resize corner.
