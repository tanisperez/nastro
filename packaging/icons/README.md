# Nastro icons

A cassette in the Silver skin's colors: a metal body, a display window with two reels, and the
bottom trapezoid with its two holes. The tape between the reels is a sound wave, and the left
reel holds more tape than the right one, so something is playing. The design session also drew
colorways for the other three skins (Graphite, Porcelain, Nebula), but they were not exported:
only the Silver icon exists as files.

| File | Use |
|---|---|
| `nastro.svg` | The official icon, Silver colorway. Master for every other size, the hicolor `scalable` icon on Linux and the logo of the website. |
| `nastro-small.svg` | Simplified variant for 32 px and below (less detail, heavier strokes). The first entries (16, 24, 32 px) of `nastro.ico` are rendered from it. |
| `nastro-symbolic.svg` | Monochrome glyph for the tray and any monochrome context. It uses `currentColor`. |

The PNG sizes under `png/` are committed and feed the Linux hicolor icons and the `.icns`.

`crates/nastro-app/assets/nastro.ico` (Windows, 16 to 256 px) is built with
ImageMagick (16, 24 and 32 px from the small variant, the rest from the PNGs):

```
for s in 16 24 32; do
  magick -background none -density 1200 nastro-small.svg -resize ${s}x${s} /tmp/s$s.png
done
magick /tmp/s16.png /tmp/s24.png /tmp/s32.png png/nastro-48.png png/nastro-64.png \
  png/nastro-128.png png/nastro-256.png ../../crates/nastro-app/assets/nastro.ico
```

The macOS `.icns` is built from the PNGs at bundle time (`iconutil`, see `packaging/macos`).
