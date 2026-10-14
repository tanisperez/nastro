# Nastro: design and technical handoff

Notes for the agent that bootstraps the Nastro repository. They collect every decision taken
during the design session of 2026-10-14. The artboards in this export are the visual reference;
this file is the readable summary. Writing rule inherited from Postino: never use the em dash.

## 1. What Nastro is

- A Winamp-style desktop music player: player window, 10-band equalizer and playlist, docked
  together with magnetic snapping, skinnable.
- Rust (edition 2024, stable toolchain) with `gpui` through `gpui-kit` 0.7.0, the same versions
  and pins as Postino (`gpui-pre` 0.3.7, `cargo tree -d --depth 0 | grep gpui` must stay empty).
- Targets: Linux, Windows and macOS.
- Follow Postino's conventions as they are (read Postino's `AGENTS.md` and `docs/architecture.md`
  first): Cargo workspace of small crates, thin UI, UI logic in plain Rust under `state/` with unit
  tests, no `unsafe` (workspace lint `unsafe_code = "forbid"`), clippy `all`, `unwrap_used` and
  `expect_used` as warnings, `thiserror` in libraries and `anyhow` only in the app, versions once in
  `[workspace.dependencies]`, rustfmt max width 100, idle window does no work, render paths stay
  cheap, logging behind the `log` facade, translations with `rust-i18n` (en, es, gl, it), user docs
  updated with every user-visible change.
- License: same as Postino (Apache-2.0) unless decided otherwise.

## 2. Name

- Name: **Nastro** (Italian for "tape", as in cassette tape). Sister name to Postino.
- Pending: trademark search in EUIPO and OEPM, class 9 (software). Known association: the beer
  brand "Nastro Azzurro" (different class).
- The crate name `nastro` was free on crates.io on 2026-10-14.
- Never use Winamp, AIMP or any other player's name, logo or imagery. Importing classic skins is
  fine to mention.

## 3. Proposed crate layout

```
nastro-app ──> nastro-dock ──> nastro-core
     ├──────> nastro-skin ───> nastro-core
     ├──────> nastro-audio ──> nastro-core
     └──────> nastro-core
```

| Crate | Responsibility |
| --- | --- |
| `nastro-core` | Domain model: tracks, playlists, queue, play state. No IO. |
| `nastro-skin` | Skin model and loaders: native `skin.toml` first, classic `.wsz` later. Token set, component tree contract. |
| `nastro-audio` | Decoding, resampling, EQ, analysis (FFT for the visualizer), output. `Decoder` trait. |
| `nastro-dock` | Magnetic snapping and layout presets as pure geometry. Unit tested, no window. |
| `nastro-app` | The gpui binary. Only UI and glue. |

## 4. Skins

### Two layers

1. **App chrome** (preferences, library, dialogs, menus, about, updater): one fixed design system
   like Postino (`theme/palette.rs`, `theme/metrics.rs`, Geist and Geist Mono, gpui-kit
   components). It does not change with the skin. It could later be shared with Postino as a
   crate (undecided).
2. **Skin** (player, equalizer, playlist and their shade modes): data loaded at runtime. One
   component tree; each skin backend only decides how each piece is painted. The app never
   hardcodes a skin color.

### Formats

| Format | Plan | Notes |
| --- | --- | --- |
| Native `skin.toml` | Phase 1, default | Tokens and metrics, optional SVG or PNG. Vector, crisp at any scale. The four built-in themes are four `skin.toml` files. |
| Classic `.wsz` | Phase 1 or 2 | ZIP of BMP sprites plus `pledit.txt`, `viscolor.txt`, `region.txt`. Fixed geometry (main window 275 x 116). Slice sprites and scale with nearest neighbour to 1x, 2x, 3x at load time, cached per scale, never in render. |
| Modern `.wal` | Research only | XML layout plus compiled MAKI scripts (`.maf`). Needs a MAKI VM and a free layout engine. Out of scope. |

### Native skin file

```toml
[skin]
name = "Silver"
format = 1

[font]
ui = "Geist"
mono = "Geist Mono"
lcd = "Share Tech Mono"

[color]
frame_hi = "#f3f5f7"
frame_mid = "#d6dbe0"
frame_lo = "#b3bac2"
frame_brush = 0.05
frame_edge = "#7b838d"
bevel_hi = "#ffffffd9"
groove = "#0000002e"
text = "#262c33"
text_muted = "#525963"
control_hi = "#fbfcfd"
control_lo = "#c3c9d0"
control_hover_hi = "#ffffff"
control_hover_lo = "#d0d5db"
slot = "#5b636c"
field_bg = "#f7f9fb"
focus = "#0f6fae"
lcd_top = "#0d1d2f"
lcd_bottom = "#12314f"
lcd_edge = "#0a121b"
lcd_fg = "#a6e6ff"
lcd_dim = "#4a7a99"
lcd_glow = "#78d2ff73"
lcd_rule = "#a6e6ff29"
vis_bar = "#a6e6ff"
vis_osc = "#7fd4ff"
vis_peak = "#ffffff"
accent = "#36b4ff"
accent_ink = "#0f6fae"
led_off = "#8a929b"

[playlist]  # same fields as pledit.txt in a classic skin
bg = "#163a62"           # NormalBG
fg = "#cfe0f2"           # Normal
current = "#ffffff"      # Current
selected_bg = "#2f6aa5"  # SelectedBG
index = "#8fb0d2"
```

### Classic skin mapping

Tokens filled from a classic skin's text files, so new panels inherit its colors:

| Token | Classic source |
| --- | --- |
| `pl_bg`, `pl_fg`, `pl_current`, `pl_selected_bg`, playlist font | `pledit.txt`: NormalBG, Normal, Current, SelectedBG, Font |
| `lcd_top` / `lcd_bottom` | `viscolor.txt` color 0 |
| `vis_bar` | `viscolor.txt` colors 2 to 17 |
| `vis_osc` | `viscolor.txt` colors 18 to 22 |
| `vis_peak` | `viscolor.txt` color 23 |
| `accent` | derived from `pledit.txt` Current |
| frame tokens | none: the frame is the bitmap itself |

Component tree contract and the classic sprite each piece comes from:

| Component | Classic sprite |
| --- | --- |
| Title bar | `titlebar.bmp` |
| Transport buttons | `cbuttons.bmp` |
| Time digits | `numbers.bmp` or `nums_ex.bmp` |
| Scrolling title | `text.bmp` |
| Play, pause, stop state | `playpaus.bmp` |
| Mono and stereo | `monoster.bmp` |
| Seek bar | `posbar.bmp` |
| Volume | `volume.bmp` |
| Balance | `balance.bmp` |
| Shuffle, repeat, EQ, PL toggles | `shufrep.bmp` |
| Equalizer and curve | `eqmain.bmp` |
| Playlist frame | `pledit.bmp` |
| Window shape | `region.txt` |

## 5. Design system (native skins)

Artboards: `Foundations.dc.html` (color), `TypeMetrics.dc.html` (type and metrics),
`Components*.dc.html` (every component and state, one sheet per theme).

### Themes

| Theme | Character |
| --- | --- |
| Silver (default) | Light brushed aluminium, ice blue LCD, navy playlist. |
| Graphite | Dark graphite, amber LCD. |
| Porcelain | Matte white, black glass LCD with vermilion LEDs, ink black playlist. |
| Nebula | Dark violet anodized metal, mint LCD, magenta accent. |

### Color tokens

Alpha values as `#rrggbbaa`.

| Token | Silver | Graphite | Porcelain | Nebula | Use |
| --- | --- | --- | --- | --- | --- |
| `frame_hi` | `#f3f5f7` | `#50555d` | `#ffffff` | `#3d3a5c` | Metal, top |
| `frame_mid` | `#d6dbe0` | `#363a40` | `#f1efea` | `#29263f` | Metal at 40 % |
| `frame_lo` | `#b3bac2` | `#25282d` | `#dedad2` | `#1b192b` | Metal, bottom |
| `frame_brush` | `0.05` | `0.025` | `0.015` | `0.03` | Brushed texture opacity |
| `frame_edge` | `#7b838d` | `#0f1113` | `#a39e95` | `#0b0a14` | Panel and control border |
| `bevel_hi` | `#ffffffd9` | `#ffffff24` | `#fffffff2` | `#ffffff1f` | 1 px top highlight |
| `groove` | `#0000002e` | `#00000073` | `#281e141f` | `#00000080` | Engraved grooves |
| `text` | `#262c33` | `#d4d8de` | `#1f1d1a` | `#e4e1f5` | Text and icons on metal |
| `text_muted` | `#525963` | `#9aa1ab` | `#5a554e` | `#a7a2c8` | Secondary labels, inactive title |
| `control_hi` | `#fbfcfd` | `#5a6068` | `#ffffff` | `#4a4670` | Button, top |
| `control_lo` | `#c3c9d0` | `#2c3035` | `#e4e0d8` | `#25223a` | Button, bottom |
| `control_hover_hi` | `#ffffff` | `#646a73` | `#ffffff` | `#565280` | Hover, top |
| `control_hover_lo` | `#d0d5db` | `#353a40` | `#ece9e3` | `#2d2a46` | Hover, bottom |
| `slot` | `#5b636c` | `#121417` | `#3a3734` | `#0e0d18` | Slider track |
| `field_bg` | `#f7f9fb` | `#1c1f23` | `#ffffff` | `#1c1a2c` | Search field |
| `focus` | `#0f6fae` | `#ffb54d` | `#c23a1f` | `#ff7ac6` | 2 px focus ring |
| `lcd_top` | `#0d1d2f` | `#0d0c09` | `#0c0b0b` | `#060914` | LCD, top |
| `lcd_bottom` | `#12314f` | `#1a1610` | `#191716` | `#0d1430` | LCD, bottom |
| `lcd_edge` | `#0a121b` | `#000000` | `#000000` | `#000000` | Recessed border |
| `lcd_fg` | `#a6e6ff` | `#ffb54d` | `#ff5a3c` | `#7cf5d4` | Clock, marquee, badges |
| `lcd_dim` | `#4a7a99` | `#7a5524` | `#6b2a1f` | `#2f6f68` | Badge borders, EQ curve when off |
| `lcd_glow` | `#78d2ff73` | `#ffaa3c66` | `#ff5a3c80` | `#7cf5d473` | Clock glow |
| `lcd_rule` | `lcd_fg` at 16 % | same | same | same | Rule above the marquee |
| `vis_bar` | `#a6e6ff` | `#ffb54d` | `#ff5a3c` | `#7cf5d4` | Spectrum bars |
| `vis_osc` | `#7fd4ff` | `#ffcf8a` | `#ff8a6e` | `#b6fff0` | Oscilloscope |
| `vis_peak` | `#ffffff` | `#fff1d6` | `#ffd2c4` | `#ff7ac6` | Peaks |
| `pl_bg` | `#163a62` | `#141619` | `#1b1a19` | `#120f24` | Playlist background |
| `pl_fg` | `#cfe0f2` | `#c6cad1` | `#e9e5de` | `#d6d2ee` | Track title |
| `pl_index` | `#8fb0d2` | `#7e858f` | `#9a948b` | `#8f89b8` | Index and duration |
| `pl_current` | `#ffffff` | `#ffb54d` | `#ff7a5c` | `#7cf5d4` | Playing track, active tab |
| `pl_selected_bg` | `#2f6aa5` | `#3b4048` | `#3d3936` | `#3a2f6b` | Selected row |
| `accent` | `#36b4ff` | `#ffb54d` | `#ff5a3c` | `#ff7ac6` | LED on, progress, volume fill |
| `accent_ink` | `#0f6fae` | `#ffb54d` | `#c23a1f` | `#ff7ac6` | Active icon on metal |
| `led_off` | `#8a929b` | `#4a4f57` | `#bdb7ad` | `#4a4766` | LED off |

Derived at runtime, not stored: `text_disabled` is `text` at 45 %; `pl_hover` is white at 7 %
over the row; `control_pressed` is the control gradient inverted.

### Gradients and shadows (same in every theme)

| Name | Recipe |
| --- | --- |
| metal | 90 degree brushed stripes (`frame_brush`, 3 px period) over 180 degree `frame_hi` 0 %, `frame_mid` 40 %, `frame_lo` 100 % |
| control | 180 degree `control_hi` to `control_lo` (hover uses the hover pair, pressed is inverted) |
| lcd | 180 degree `lcd_top` to `lcd_bottom` |
| groove | 180 degree stripes, 3 px period: `groove` 1 px, `bevel_hi` 1 px, transparent 1 px |
| vis bars | 0 degree stripes, 3 px period: `vis_bar` 2 px, transparent 1 px |
| shadow window | inset 0 1 0 `bevel_hi`, 0 12 32 black 35 % |
| shadow control | inset 0 1 0 `bevel_hi`, 0 1 2 black 25 % |
| shadow pressed | inset 0 2 4 black 35 % |
| shadow lcd | inset 0 2 8 black 60 %, 0 1 0 `bevel_hi` |
| shadow list | inset 0 2 6 black 35 %, 0 1 0 `bevel_hi` |
| shadow slot | inset 0 1 2 black 50 %, 0 1 0 `bevel_hi` |
| led glow | 0 0 6 `accent` |
| clock glow | text shadow 0 0 10 `lcd_glow` |

Values are CSS. gpui's `blur_radius` is half a CSS blur (see Postino's design system).

### Rules

- Colors only from the active skin, never a literal in a view.
- The accent is for what is active or playing: LEDs on, progress, current track, focus. Never
  decoration.
- Text on metal at least 4.5:1; icons and focus at least 3:1. Light metals (Silver, Porcelain)
  use `accent` for fills and the darker `accent_ink` for strokes on metal.
- Inactive window: only the title text turns `text_muted` and the groove disappears.
- A new theme is validated against the same table: if a pair misses contrast, fix the token.

### Typography

Fonts bundled in the binary (SIL OFL 1.1): Geist, Geist Mono, Share Tech Mono (LCD only).

| Role | Font | Size / weight | Details |
| --- | --- | --- | --- |
| Clock | Share Tech Mono | 46 / 400 | 0.02em, line height 1, clock glow |
| LCD badge | Share Tech Mono | 10 / 400 | 0.04em, 1 px `lcd_dim` box, radius 3 |
| Marquee | Geist Mono | 12 / 400 | Scrolls only while playing |
| Panel title | Geist | 10 / 700 | Uppercase, 0.18em, text shadow 0 1 `bevel_hi` |
| Menu | Geist | 11 / 500 | 18 px box, radius 3 |
| EQ button | Geist | 10 / 600 | Uppercase, 0.1em |
| LED toggle | Geist | 9 / 600 | Uppercase, 0.1em, 6 px LED before |
| Playlist button | Geist | 10 / 600 | Uppercase, 0.06em |
| Tab | Geist | 11 / 600 active, 500 inactive | |
| Playlist row | Geist | 12 / 400, 600 when playing | |
| Playlist numbers | Geist Mono | 11 / 400 | Index, duration, summary |
| EQ numbers | Geist Mono | 9 / 400 | Frequencies and dB scale |

Small sizes are deliberate. A skin scale setting (1x, 1.5x, 2x) enlarges the whole skin,
independent of the OS scale.

### Metrics (logical px at 1x)

- Panels: player and EQ 520 wide fixed; playlist free width from 520; radius 8; 1 px
  `frame_edge` border; title bar 24; menu 22; shade mode height 24 for every panel.
- Player, 520 x 274: title 24, menu 22, padding 2, LCD 118, gap 8, seek 16, gap 8, transport 38,
  gap 8, toggles 18, padding 10. Body padding 2 / 10 / 10.
- Equalizer, 520 x 207: title 24, padding 10, curve 40, gap 8, sliders 96, labels 15, padding 12.
  Left column 84 wide, gap 12. In row layouts it matches its neighbour's height by stretching
  the sliders.
- Playlist, default 600 x 487 (layout C): title 24, padding 8, tabs and filter 22, gap 6, rows,
  gap 6, footer 20, padding 10. Row height 22 (17 rows visible at 487). In other layouts the
  height snaps to whole rows.
- LCD: radius 6, padding 10 / 12 / 7, spectrum 24 bars, gap 2, height 66. EQ curve 40, radius 4.
- Buttons: transport 30 round, play 38 round, open 26 x 22 radius 5, window buttons 16 x 14
  radius 3, LED toggle height 18 radius 9 with 6 px LED, EQ buttons height 22 radius 5 width 84,
  playlist buttons height 20 radius 4.
- Sliders: seek track 6, thumb 28 x 12; volume 86 wide, track 4, thumb 14 x 12; balance 46
  wide, thumb 12 x 12 with a centre tick; EQ band column 26, track 4 x 96, thumb 20 x 9 radius 3;
  preamp column 28.
- Playlist: index column 22 right aligned, row padding 0 / 10, gap 8; tab height 22 radius
  5 5 0 0; filter height 22 radius 11, input 120; new playlist button 20 x 20 radius 4.
- Focus ring 2 px `focus`, offset 2.

### Components and states

Every control has normal, hover, pressed, active (accent ink or LED on), disabled (45 %
opacity) and focus states, drawn in `Components*.dc.html`: window buttons, transport buttons,
LED toggles, text buttons, EQ buttons, presets dropdown and menu, seek, volume, balance and EQ
sliders, LCD with badges and state glyphs, EQ curve on and off, playlist rows (normal, hover,
selected, playing, playing and selected), tabs, filter field, empty list message, shade bars for
all three panels, and 14 hand-drawn icons on a 16 grid (fill or 1.4 to 1.8 stroke,
`currentColor`): previous, play, pause, stop, next, open, volume, search, plus, chevron, minimize,
shade, close, logo.

## 6. Window layout and docking

- Default layout: **C**, player and equalizer stacked on the left, playlist on the right with the
  height of that column (1126 x 487 at 1x). Presets A (column), B (EQ right of the player,
  playlist full width below), C, D (single row strip) under View, Layout.
- Docking rules: snap distance 12 px, docked gap 6 px, edge and corner alignment, dragging the
  player moves the group, dragging another panel detaches it.
- Wayland does not let a client know or set its window position (core xdg-shell). The
  `ext-placement` protocol is a draft and would only suggest a position before mapping. So
  independent magnetic top-level windows are impossible there.
- gpui-pre 0.3.7 facts (checked in its source): no programmatic window move on any platform;
  it has initial bounds at open, `resize`, `on_moved`, `start_window_move` (interactive drag),
  transparent window backgrounds and `Window::set_input_region` (Wayland only: clicks outside
  the region pass through).
- Plan: one transparent window holds the whole group; panels snap and detach inside it; the
  window grows and shrinks to fit; gaps are transparent and click-through via
  `set_input_region`; dragging the player calls `start_window_move`. Same approach on every
  platform for version 1. Separate OS windows on X11, macOS and Windows later, which needs a
  `set_position` added to gpui (fork or upstream PR).
- The snapping and layout logic lives in `nastro-dock` as plain geometry with unit tests.

## 7. Audio

| Piece | Crate (version checked 2026-10-14) | Notes |
| --- | --- | --- |
| Decoding | `symphonia` 0.6.1 (MPL-2.0) | MP3, FLAC, Vorbis, ALAC, AAC-LC, PCM, ADPCM; WAV, AIFF, CAF, MP4, OGG, MKV. Gapless for MP3, FLAC, Vorbis, ALAC. MSRV 1.85. |
| Output | `cpal` 0.18.2 (Apache-2.0) | WASAPI (ASIO, JACK optional), CoreAudio, ALSA (PulseAudio, PipeWire, JACK optional). |
| Resampling | `rubato` 5.0.1 | When track and device rates differ. |
| Analysis | `realfft` 3.5.0 | Spectrum for the visualizer. |
| Equalizer | own code | 10 peaking biquads plus preamp, unit tested. |
| Opus (until Symphonia has it) | `opus` 0.4.0 | Bindings to native libopus. |
| Optional fallback | `ffmpeg-next` 9.0.0 | Behind a cargo feature, off by default, for exotic formats. |

- Put a `Decoder` trait in `nastro-audio`; Symphonia is the default implementation.
- Own the pipeline (decode, resample, EQ, analysis, output) instead of `rodio` 0.23, so EQ,
  visualizer and gapless playback are under our control. `rodio` is fine for a first spike.
- Symphonia gaps today: Opus and WavPack in progress, AAC without gapless, APE (Monkey's Audio)
  only as a tag format.
- The visualizer and marquee animate only while playing; paused or stopped means zero frames.
- Rejected: BASS (un4seen). Closed source; free only for products that make no money; commercial
  licences are per platform (950 EUR single commercial, 125 EUR shareware, 40 % off each extra
  platform). AIMP was initially based on BASS; since version 3 it has its own engine.

## 8. App icon

Silver colorway is the official icon; the other three themes have matching colorways
(`AppIcon.dc.html`, `Brand.dc.html`). Concept: a cassette (window with two reels, bottom
trapezoid with two holes); the tape between the reels is a sound wave; the left reel holds more
tape, so something is playing. At 32 px and below use the simplified version. Sources for
`packaging/icons/`:

`nastro.svg`

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <title>Nastro</title>
  <defs>
    <linearGradient id="metal" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f3f5f7"/>
      <stop offset="0.4" stop-color="#d6dbe0"/>
      <stop offset="1" stop-color="#b3bac2"/>
    </linearGradient>
    <linearGradient id="lcd" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#0d1d2f"/>
      <stop offset="1" stop-color="#12314f"/>
    </linearGradient>
    <radialGradient id="pack" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0.3" stop-color="#24578a"/>
      <stop offset="1" stop-color="#14365c"/>
    </radialGradient>
    <filter id="glow" x="-30%" y="-60%" width="160%" height="220%">
      <feGaussianBlur stdDeviation="14"/>
    </filter>
  </defs>
  <rect x="64" y="64" width="896" height="896" rx="200" fill="url(#metal)" stroke="#7b838d" stroke-width="6"/>
  <path d="M264 72H760" stroke="#ffffff" stroke-opacity="0.9" stroke-width="6" stroke-linecap="round"/>
  <rect x="168" y="232" width="688" height="432" rx="80" fill="url(#lcd)" stroke="#0a121b" stroke-width="8"/>
  <path d="M232 236H792" stroke="#ffffff" stroke-opacity="0.45" stroke-width="3" stroke-linecap="round"/>
  <circle cx="376" cy="468" r="138" fill="url(#pack)" stroke="#a6e6ff" stroke-opacity="0.35" stroke-width="4"/>
  <circle cx="664" cy="468" r="100" fill="url(#pack)" stroke="#a6e6ff" stroke-opacity="0.35" stroke-width="4"/>
  <circle cx="376" cy="468" r="40" fill="none" stroke="#a6e6ff" stroke-width="22"/>
  <circle cx="664" cy="468" r="40" fill="none" stroke="#a6e6ff" stroke-width="22"/>
  <path d="M376 330C436 252 476 422 520 350C562 280 612 406 664 368" fill="none" stroke="#36b4ff" stroke-width="30" stroke-linecap="round" opacity="0.7" filter="url(#glow)"/>
  <path d="M376 330C436 252 476 422 520 350C562 280 612 406 664 368" fill="none" stroke="#d8f4ff" stroke-width="16" stroke-linecap="round"/>
  <path d="M300 957L356 776H668L724 957" fill="#d6dbe0" stroke="#7b838d" stroke-width="6" stroke-linejoin="round"/>
  <path d="M362 782H662" stroke="#ffffff" stroke-opacity="0.9" stroke-width="4" stroke-linecap="round" opacity="0.8"/>
  <circle cx="436" cy="872" r="22" fill="#12314f" stroke="#7b838d" stroke-width="4"/>
  <circle cx="588" cy="872" r="22" fill="#12314f" stroke="#7b838d" stroke-width="4"/>
</svg>
```

`nastro-small.svg` (32 px and below)

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 1024 1024">
  <title>Nastro</title>
  <defs>
    <linearGradient id="metal" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f3f5f7"/>
      <stop offset="1" stop-color="#b3bac2"/>
    </linearGradient>
  </defs>
  <rect x="48" y="48" width="928" height="928" rx="208" fill="url(#metal)" stroke="#7b838d" stroke-width="16"/>
  <rect x="128" y="168" width="768" height="512" rx="128" fill="#12314f"/>
  <circle cx="352" cy="440" r="112" fill="none" stroke="#a6e6ff" stroke-width="64"/>
  <circle cx="672" cy="440" r="80" fill="none" stroke="#a6e6ff" stroke-width="64"/>
  <path d="M352 296C424 196 472 436 512 350C552 264 608 400 672 328" fill="none" stroke="#d8f4ff" stroke-width="72" stroke-linecap="round"/>
  <path d="M280 968L352 768H672L744 968" fill="#b3bac2" stroke="#7b838d" stroke-width="16" stroke-linejoin="round"/>
</svg>
```

`nastro-symbolic.svg` (tray and symbolic icons, `currentColor`)

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">
  <title>Nastro</title>
  <rect x="1.5" y="3.5" width="21" height="17" rx="3" stroke-width="1.6"/>
  <circle cx="7.5" cy="10.5" r="3" stroke-width="2"/>
  <circle cx="16.5" cy="10.5" r="2.2" stroke-width="2"/>
  <path d="M7.5 7.5C9.3 4.6 10.7 10 12 8C13.2 6.2 14.6 9 16.5 8.3" stroke-width="2"/>
  <path d="M6.5 20.5L8 16.5H16L17.5 20.5" stroke-width="2"/>
</svg>
```

## 9. Artboards in this export

| File | Content |
| --- | --- |
| `Main.dc.html`, `Graphite.dc.html`, `Porcelain.dc.html`, `Nebula.dc.html` | Interactive player in layout C, one per theme (same file, different default theme). |
| `Layouts.dc.html` | Layout presets A to D at 1:2. |
| `Foundations.dc.html` | Color tokens for the four themes, gradients, shadows, rules. |
| `TypeMetrics.dc.html` | Typography and metrics. |
| `Components.dc.html` and `Components{Graphite,Porcelain,Nebula}.dc.html` | Component sheets with all states. |
| `DesignSystem.dc.html` | The two-layer analysis, skin formats, classic mapping, gpui risks. |
| `Brand.dc.html`, `AppIcon.dc.html` | App icon, colorways, sizes, symbolic glyph, lockup. |

Known mockup-only detail: the focus ring in the player artboards reads the theme's `focus` token
through a CSS variable; the real app reads the token directly.

## 10. Open decisions (ask before choosing)

- GitHub owner and repository name.
- Trademark check for "Nastro".
- Language of the docs (Postino uses English, also pending there).
- Whether the app chrome design system becomes a crate shared with Postino.
- Contributor terms (CLA or DCO), as in Postino.
