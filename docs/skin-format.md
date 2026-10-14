# Skin format

Reference for people who write Nastro skins. This is format version 1, the native `skin.toml`
format. The reader is the `nastro-skin` crate.

## Where skins go

User skins live in the `nastro/skins/` folder of the user's config directory. These are the
planned locations, the app does not read them yet:

| System | Folder |
| --- | --- |
| Linux | `~/.config/nastro/skins/` |
| macOS | `~/Library/Application Support/nastro/skins/` |
| Windows | `%APPDATA%\nastro\skins\` |

## File layout

A skin is either a single file or a folder:

- `skins/<id>.toml`: the skin id is the file name without `.toml`.
- `skins/<id>/skin.toml`: the skin id is the folder name. The folder is meant for skins that
  will carry extra files later (images, fonts).

The id must not be one of the built-in ids (`silver`, `graphite`, `porcelain`, `nebula`) and
must be unique: a clash is reported as an error and that skin is skipped. Skins are listed
sorted by id.

## Sections and keys

The file has four sections: `[skin]`, `[font]`, `[color]` and `[playlist]`.

### `[skin]`

| Key | Meaning |
| --- | --- |
| `name` | Display name. Required. |
| `format` | Format version, an integer. Must be `1`. |

### `[font]`

Font family names. They are looked up in the system fonts, falling back to the app's own.

| Key | Default | Use |
| --- | --- | --- |
| `ui` | `Geist` | Proportional UI font |
| `mono` | `Geist Mono` | Monospace font |
| `lcd` | `Share Tech Mono` | Clock, marquee and badges |

### `[color]`

Every value is a string, except `frame_brush`, which is a number from 0 to 0.2 (opacity of the
brushed texture on the metal; Silver uses `0.05`).

| Key | Use |
| --- | --- |
| `frame_hi` | Metal, top |
| `frame_mid` | Metal, middle (at 40 %) |
| `frame_lo` | Metal, bottom |
| `frame_edge` | Panel and control border |
| `bevel_hi` | 1 px top highlight |
| `groove` | Engraved grooves |
| `text` | Text and icons on metal |
| `text_muted` | Secondary labels, inactive title |
| `control_hi` | Button, top |
| `control_lo` | Button, bottom |
| `control_hover_hi` | Button hover, top |
| `control_hover_lo` | Button hover, bottom |
| `slot` | Slider track |
| `field_bg` | Search field background |
| `focus` | 2 px focus ring |
| `lcd_top` | LCD, top |
| `lcd_bottom` | LCD, bottom |
| `lcd_edge` | LCD recessed border |
| `lcd_fg` | Clock, marquee and badges |
| `lcd_dim` | Badge borders, EQ curve when off |
| `lcd_glow` | Clock glow |
| `lcd_rule` | Rule above the marquee (`lcd_fg` at 16 %, alpha `29`) |
| `vis_bar` | Spectrum bars |
| `vis_osc` | Oscilloscope |
| `vis_peak` | Peaks |
| `accent` | LED on, progress, volume fill |
| `accent_ink` | Active icon on metal |
| `led_off` | LED off |

### `[playlist]`

The same fields as `pledit.txt` in a classic skin.

| Key | Classic name | Use |
| --- | --- | --- |
| `bg` | `NormalBG` | Playlist background |
| `fg` | `Normal` | Track title |
| `current` | `Current` | Playing track and active tab |
| `selected_bg` | `SelectedBG` | Selected row |
| `index` | | Index and duration |

## Colors and alpha

A color is `"#rgb"`, `"#rrggbb"` or `"#rrggbbaa"`, case-insensitive. The optional last pair
is the alpha: `ff` is opaque, `00` is transparent. For example `"#ffffffd9"` is white at 85 %,
and `"#a6e6ff29"` is `#a6e6ff` at 16 %. Alpha is meant for the tokens that are painted over
something (`bevel_hi`, `groove`, `lcd_glow`, `lcd_rule`). Use opaque colors elsewhere.

Some values are derived by the app and cannot be set: disabled text is `text` at 45 %, the
playlist row hover is white at 7 %, and a pressed control is the control gradient inverted.

## Missing and unknown keys

- A missing key in `[color]` or `[playlist]` (or `[font]`) takes the value from the Silver skin
  and produces a warning that names the key. A skin written for an older version keeps working
  when new tokens are added.
- An unknown key or section is ignored with a warning.
- These are errors, and the skin is not loaded: invalid TOML, a color that is not in one of
  the forms above, a value of the wrong type, `format` other than `1`, a missing `[skin] name`,
  and a `frame_brush` outside 0 to 0.2. The message names the key.

## Contrast rule

Text on metal at least 4.5:1, icons and focus at least 3:1 (WCAG 2.x contrast ratio, with
translucent colors composited over their background first). Light metals use `accent` for
fills and the darker `accent_ink` for strokes on metal. The checked pairs are:

| Pair | Minimum |
| --- | --- |
| `text` on `frame_hi`, `frame_mid`, `frame_lo`, `control_hi`, `control_lo` | 4.5 |
| `text_muted` on `frame_mid` | 4.5 |
| `accent_ink` on `control_hi`, `control_lo` | 3.0 |
| `focus` on `frame_mid` | 3.0 |
| `lcd_fg` on `lcd_top`, `lcd_bottom` | 4.5 |
| playlist `fg` on `bg` and on `selected_bg` | 4.5 |
| playlist `current` on `bg` and on `selected_bg` | 3.0 |

The `nastro-skin` crate exposes the check as `contrast_issues`. A skin that misses a pair still
loads, the check is advice for authors.

## Example

This is the built-in Silver skin, complete.

```toml
# Silver: light brushed aluminium, ice blue LCD, navy playlist.
# Built-in skin, format 1. Every token is documented in docs/skin-format.md.

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

[playlist]
bg = "#163a62"
fg = "#cfe0f2"
current = "#ffffff"
selected_bg = "#2f6aa5"
index = "#8fb0d2"
```

## Classic skins

Importing classic `.wsz` skins (a ZIP of bitmap sprites plus `pledit.txt`, `viscolor.txt` and
`region.txt`) is planned. It is not available yet.
