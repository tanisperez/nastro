# Nastro design system

Reference for anyone touching the UI. It summarizes the design handoff of 2026-10-14 as
implemented. If this file and the code disagree, the code wins; fix this file. The handoff and
its artboards (the player in the four skins and the layout presets) were removed once
implemented; they are kept under the git tag `design-mockups`.

Nastro has two visual layers:

1. **The skin**: the player, the equalizer, the playlist and their shade bars. Every color comes
   from the active `skin.toml` (`docs/skin-format.md`); the app never hardcodes a skin color.
2. **The app chrome**: menus, Preferences and About. One fixed palette, Postino's, in light and
   dark, following the OS or the user's choice (Preferences, "Menus and windows").

## Where things live

All under `crates/nastro-app/src/`:

| What | File |
| --- | --- |
| Skin tokens as gpui colors, shadows and gradients | `theme/skin.rs` (`SkinTheme`, `cx.skin()`) |
| Skin sizes at 1x | `theme/metrics.rs` |
| Chrome palette and the gpui-kit theme JSON | `theme/palette.rs` (`cx.palette()`) |
| Fonts, theme registration | `theme/mod.rs` |
| Skin building blocks | `views/components/` |
| Skin icons | `assets/icons/`, `assets.rs` (`SkinIcon`) |
| The built-in skins | `crates/nastro-skin/skins/*.toml` |

## Rules

- Skin colors only from `cx.skin()`, chrome colors only from `cx.palette()`, never a literal in
  a view. Shadows that the design gives in black (drop shadows, inner shadows) are the same in
  every skin and use `theme::skin::black`.
- Skin sizes only from `theme::metrics`, always through `SkinTheme::px`, which applies the skin
  scale (1x, 1.5x, 2x, independent of the OS scale). Chrome sizes are plain pixels.
- The accent is for what is active or playing: LEDs on, progress and volume fills, the current
  track, the focus. Never decoration. On light metals (Silver, Porcelain) fills use `accent`
  and icons on metal use the darker `accent_ink`.
- Text on metal at least 4.5:1, icons and focus at least 3:1. `nastro-skin`'s contrast checks
  enforce it for the built-ins; three designed pairs miss it and are recorded as known
  exceptions until the design decides (Silver `pl_fg` on `pl_selected_bg` 4.19; Graphite `text`
  on `control_hi` 4.44 and `text_muted` on `frame_mid` 4.39).
- An inactive window changes only the title text (to `text_muted`) and hides the title groove.
- Every control has normal, hover and pressed looks: the control gradient, its hover pair, and
  the gradient inverted with an inner shadow while pressed.
- Render paths stay cheap and nothing animates while paused or stopped (`AGENTS.md`,
  "Performance").

## Skin tokens

What each token is for is in `docs/skin-format.md`; the values of the four built-in skins are
their files, `crates/nastro-skin/skins/*.toml`. Derived at runtime, not stored:
`text_disabled` (text at 45 %), the playlist row hover (white at 7 %), the pressed control (the
gradient inverted).

### Gradients and shadows

| Name | Implementation |
| --- | --- |
| Metal | Two stacked two-stop gradients (`frame_hi` to `frame_mid` over the top 40 %, `frame_mid` to `frame_lo` below), since gpui gradients have two stops, plus the brushed stripes painted on a canvas: 1 px of white at `frame_brush` every 3 px over black at half of it. |
| Control | `control_hi` to `control_lo`; hover uses the hover pair; pressed inverts it. |
| LCD | `lcd_top` to `lcd_bottom`. |
| Groove | Lines of `groove` and `bevel_hi` every 3 px, painted on a canvas. |
| Spectrum bars | 2 px lit, 1 px dark, from the bottom up, in `vis_bar`; peaks in `vis_peak`. |
| Shadows | `SkinTheme::panel_shadow`, `control_shadow`, `bevel_shadow`, `lcd_shadow`, `list_shadow`, `slot_shadow`, `led_glow`. gpui supports inset shadows; CSS blurs are halved for gpui (`docs/gpui-notes.md`). |

## Typography

Fonts bundled in the binary (`crates/nastro-app/assets/fonts/`, SIL OFL 1.1): Geist and Geist
Mono (the skin's text and numbers, and the chrome), Share Tech Mono (the display's clock and
badges only).

| Role | Font | Size and weight |
| --- | --- | --- |
| Clock | Share Tech Mono | 46 / 400 |
| Display badge | Share Tech Mono | 10 / 400, 1 px `lcd_dim` box, radius 3; the channels badge filled with `lcd_fg` |
| Marquee | Geist Mono | 12 / 400, scrolls only while playing |
| Panel title | Geist | 10 / 700, uppercase, 0.18 em |
| Menu bar | Geist | 11 / 500 |
| Equalizer button | Geist | 10 / 600, uppercase, 0.1 em |
| LED toggle, BAL | Geist | 9 / 600, uppercase, 0.1 em |
| Playlist button | Geist | 10 / 600, uppercase, 0.06 em |
| Tab | Geist | 11 / 600 active, 500 inactive |
| Playlist row | Geist | 12 / 400, 600 when playing |
| Playlist numbers | Geist Mono | 11 / 400 |
| Equalizer numbers | Geist Mono | 9 / 400 |

## Metrics

All in `theme/metrics.rs`, logical pixels at 1x:

- Panels: player and equalizer 520 wide; playlist from 520, default 600 x 487; radius 8; 1 px
  `frame_edge` border; title bar 24; menu bar 22; shade bar 24.
- Player, 520 x 274: title 24, menu 22, padding 2 / 10 / 10, display 118, seek 16, transport
  38, toggles 18, gaps 8.
- Equalizer, 520 x 207: title 24, padding 10 / 10 / 12, curve 40, sliders 96, labels 15, left
  column 84, gap 12. In row layouts (B and D) it is as tall as the player and the sliders grow.
- Playlist: padding 8 / 10 / 10, tabs and filter 22, rows of 22, footer 20, gaps 6. Everything
  but the rows is 106 (`PLAYLIST_CHROME_HEIGHT`, also given to `nastro-dock` so the height snaps
  to whole rows).
- Display: radius 6, padding 10 / 12 / 7, 24 spectrum bars with gap 2 and height 66.
- Buttons: transport 30 round, play 38 round, open 26 x 22 radius 5, window buttons 16 x 14
  radius 3, LED toggles 18 high radius 9 with a 6 px LED, equalizer buttons 22 high radius 5,
  playlist buttons 20 high radius 4.
- Sliders: seek track 6 and thumb 28 x 12; volume 86 wide, track 4, thumb 14 x 12; balance 46
  wide, thumb 12 x 12 with a centre tick; equalizer track 4 x 96, thumb 20 x 9 radius 3.
- Docking: gap 6, snap distance 12, window margin 16.

## Components (`views/components/`)

`frame` (panel with metal, groove, title bar), `buttons` (control, window button, transport
button, LED, LED toggle, equalizer button, text button), `slider` (horizontal and vertical, with
fill and centre tick), `menu` (`MenuTrigger`, a skinned button that opens a gpui-kit popup
menu), `text` (tracked uppercase labels, icons).

The fourteen hand-drawn icons of the design are SVGs on a 16 px grid in `assets/icons/`
(previous, play, pause, stop, next, open, volume, search, plus, chevron, minimize, shade,
close, logo), plus the display's state glyphs and the playlist's resize grip. gpui draws only
their shape; the color comes from the skin.

## Chrome palette

Postino's design system (`~/dev/postino/docs/design-system.md`), the same tokens in
`theme/palette.rs`: `bg`, `surface`, `raised`, `overlay`, `border`, `border_strong`, `fg`,
`fg_muted`, `fg_subtle`, `accent` (indigo), `accent_fg`, `accent_text`, `accent_subtle`,
`hover`, `warning`, `danger`, and the two-layer shadow. Preferences uses 28 px choice chips:
selected ones are `accent_subtle` with an `accent` border and `accent_text` label.

## Where the code differs from the design

- **Letter spacing**: gpui has none, so tracked uppercase labels are one box per character with
  a gap of the tracking. Kerning is lost, invisible at these sizes.
- **Text shadows**: gpui has none, so the clock glow (`lcd_glow`) and the panel titles' 1 px
  `bevel_hi` shadow are not drawn.
- **Focus rings**: the skin's controls are not keyboard focusable yet, so the 2 px `focus` ring
  is not drawn.
- **Shade bars**: the handoff names them but its export has no artboard for them. The player's
  bar holds the logo, a small display clock, the track line, a small transport and the window
  buttons; the equalizer's adds its ON LED before the title; the playlist's is its title bar.
- **Menus**: the artboards show only the menu bar; the menus themselves are gpui-kit popup
  menus in the chrome palette, as the two-layer design says.
- **Empty and idle states**: the display shows "Nastro. Drop music here or press L to open
  files." when nothing is loaded, and an engine error in place of the track line until the
  next track starts; the playlist shows a hint when empty and "No tracks match this filter."
  when the filter hides everything.
- **The resize grip** at the playlist's bottom right corner is not in the artboards.
