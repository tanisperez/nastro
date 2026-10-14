#!/bin/bash
# Regenerates the website and README screenshots (site/img/player-<skin>.webp): the main window
# in each built-in skin, playing the synthetic samples, captured on native Wayland with
# Spectacle. Needs a KDE Plasma Wayland session, spectacle, magick and the samples
# (`make samples`). Keep the pointer away from where the window opens (the centre of the
# screen): a hovered playlist row would show in the capture.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/release/nastro"
OUT="$ROOT/site/img"
SAMPLES="$ROOT/samples/audio"
WORK="$(mktemp -d)"

cleanup() {
    pkill -x nastro 2>/dev/null
    rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

die() { echo "capture.sh: $*" >&2; exit 1; }

for tool in spectacle magick; do
    command -v "$tool" >/dev/null || die "missing required tool: $tool"
done
[ -n "${WAYLAND_DISPLAY:-}" ] || die "run it from a Wayland session"
[ -d "$SAMPLES" ] || die "no samples, run 'make samples' first"
pgrep -x nastro >/dev/null && die "a nastro process is already running, close it first"
if [ ! -x "$BIN" ]; then
    (cd "$ROOT" && cargo build --release) || die "cargo build failed"
fi
mkdir -p "$OUT"

for skin in silver graphite porcelain nebula; do
    # A fresh, isolated home per skin: no real settings, session or playlists are touched.
    home="$WORK/$skin"
    mkdir -p "$home/.config/nastro" "$home/.local/state" "$home/.local/share"
    printf 'skin = "%s"\nlanguage = "en"\n' "$skin" > "$home/.config/nastro/settings.toml"
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_STATE_HOME="$home/.local/state" \
        XDG_DATA_HOME="$home/.local/share" "$BIN" "$SAMPLES" >"$WORK/$skin.log" 2>&1 &
    # Let the first track play long enough for a lively visualizer and a scrolled marquee.
    sleep 6
    spectacle -b -n -a -o "$WORK/$skin.png" || die "spectacle failed"
    pkill -x nastro
    sleep 1
    magick "$WORK/$skin.png" -quality 88 -define webp:method=6 -define webp:alpha-quality=95 \
        "$OUT/player-$skin.webp" || die "magick failed (no WebP support?)"
    echo "site/img/player-$skin.webp"
done
