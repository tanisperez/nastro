# AGENTS.md

Guidance for AI agents working on Nastro. See `README.md` for the project overview.

## Project

Nastro is an open source, lightweight, native desktop music player with a classic, skinnable
look: a player window, a 10-band equalizer and a playlist, docked together with magnetic
snapping. Written in Rust with the `gpui` crate. Target platforms: Linux, Windows and macOS. No
account, no cloud, no telemetry: it plays local files.

It is a desktop project, unrelated to the Expo mobile apps in `~/dev`. Do not reuse the
Expo/React Native conventions here. Its sister project is Postino (`~/dev/postino`), a desktop
HTTP client with the same stack: when in doubt about a convention, Postino's `AGENTS.md` and
`docs/` are the reference, and lessons learned there apply here too.

## Decisions taken

- Name: **Nastro** (Italian for "tape", as in cassette tape). Never use Winamp, AIMP or any
  other player's name, logo or imagery. Mentioning that Nastro can import classic skins is fine.
- Stack: Rust (edition 2024, stable toolchain) + `gpui` through
  [`gpui-kit`](https://crates.io/crates/gpui-kit) 0.7.0, which pins `gpui-pre` 0.3.7, the same
  versions as Postino (the `Cargo.lock` started from Postino's). `cargo tree -d --depth 0 | grep
  gpui` must stay empty.
- Two visual layers. The **skin** (player, equalizer, playlist and their shade bars) is data
  loaded at runtime from a `skin.toml`; the app never hardcodes a skin color. The **app chrome**
  (menus, Preferences, About) has one fixed design system, Postino's palette and metrics. See
  `docs/design-system.md` and `docs/skin-format.md`.
- Skins: the native `skin.toml` format first, four built-in skins (Silver, the default,
  Graphite, Porcelain, Nebula). Classic `.wsz` skins are planned; modern `.wal` skins are out of
  scope.
- Windows: one transparent window holds the docked group (the plan and the reasons are in
  `docs/architecture.md`, "Windows and docking"). The geometry lives in `nastro-dock`.
- Audio: our own pipeline (Symphonia decode, rubato resample, our equalizer, realfft analysis,
  cpal output), not `rodio`, so the equalizer, the visualizer and gapless playback are under our
  control. Details in `docs/audio.md`.
- License: Apache-2.0 (`LICENSE`, `license` in `[workspace.package]`), like Postino.
- GitHub repository: `tanisperez/nastro`, confirmed on 2026-10-14. Release work is tracked
  with a milestone and issues there once the repository exists.

## Pending decisions (ask before choosing)

Taken provisionally with Postino's values so the files have something to say. Do not change them
silently, and do not settle them without asking.

- The website (provisionally `nastro.tanis.codes`).
- Trademark search for "Nastro" (EUIPO and OEPM, class 9). Known association: the beer brand
  "Nastro Azzurro", a different class.
- Language of the docs (English for now, as in Postino).
- Whether the app chrome design system becomes a crate shared with Postino.
- Contributor terms (CLA or DCO). Ask before accepting external contributions.

## Architecture

A Cargo workspace of small crates, UI kept thin. Reasons and conventions in
`docs/architecture.md`; `gpui-kit` pitfalls in `docs/gpui-notes.md`. Dependency direction
(arrows mean "depends on"), no cycles, no crate depends on `nastro-app`:

```
nastro-app ──> nastro-audio ──> nastro-core
     ├──────> nastro-skin
     ├──────> nastro-dock
     └──────> nastro-core
```

- `nastro-core`: domain model (tracks, playlists, play order, play state, equalizer settings
  and presets), M3U playlists as text, time formatting. No IO.
- `nastro-skin`: the skin model, the `skin.toml` parser, the built-in skins, the contrast
  checks. No `gpui` types: colors are its own `Color`, converted by the app once per skin load.
- `nastro-dock`: panel geometry: layout presets, magnetic snapping, docked groups. Pure
  functions over rectangles, no window.
- `nastro-audio`: the `Decoder` trait and its Symphonia implementation, metadata probing,
  resampling, the equalizer, spectrum analysis and the player engine with its cpal output.
- `nastro-app`: the `gpui` binary (`nastro`). Only UI and glue.

Everything except `nastro-app` is testable with `cargo test`, with no window, no GPU and no
audio device. UI logic that can be plain Rust lives under `crates/nastro-app/src/state/` with
unit tests; views only render and forward input.

## Conventions

- Errors: `thiserror` enums in library crates, `anyhow` only in `nastro-app`. No `unwrap()` or
  `expect()` outside tests, except for truly impossible states, with a comment saying why.
- Lints: clippy `all`, `unwrap_used` and `expect_used` warn (`make lint` turns warnings into
  errors). Tests may allow the first two at module level. `unsafe_code` is denied; the one
  exception is `crates/nastro-app/src/platform/win32.rs` (the main window's region on Windows,
  which gpui does not offer), with a `SAFETY` comment on every block. Do not add another.
- Dependencies: versions live once in `[workspace.dependencies]`; crates use
  `dep.workspace = true`. Do not add a dependency without a clear reason.
- No `gpui`, `symphonia`, `cpal` or `toml` types in a public API outside their own crate:
  crossing crate boundaries is plain owned data from `nastro-core` (or the crate's own types).
- Doc comments on every public item. Small modules. Readable over clever.
- Formatting: `rustfmt.toml` and `.editorconfig` (4 spaces, LF, max width 100).
- Writing style: never use the em dash, in code, comments, docs, translations or commits. Use
  a comma or a period.

## Design system

Read `docs/design-system.md` before changing any UI. Skin colors only through the active skin
(`cx.skin()`), chrome colors only through `cx.palette()`, sizes only from `theme::metrics`
(scaled by the skin scale), never as literals in a view.

## Translations

Same system as Postino (`docs/i18n.md`): `rust-i18n` 4, `crates/nastro-app/locales/*.yml`,
`_version: 2`, every key with `en`, `es`, `gl` and `it` together, one file per area. Tests never
call `rust_i18n::set_locale`. Not translated: log messages, library errors, codec names and
units (`FLAC`, `kbps`, `kHz`), keyboard shortcut labels.

## Performance

The rules found in Postino apply unchanged. Keep them in every change.

- An idle window does no work. gpui re-renders the whole window whenever any entity calls
  `cx.notify()`. The visualizer, the clock and the marquee animate only while playing: paused
  or stopped means zero frames. Check it: with the app paused for 10 s, the main thread CPU
  ticks (fields 14 and 15 of `/proc/<pid>/task/<pid>/stat`) must barely move.
- `render` runs on every frame. Nothing in a render path may cost in proportion to the playlist
  size: the playlist renders only its visible rows (`uniform_list`), the filter result is cached
  under a generation counter, and labels are `SharedString` built once.
- No file IO and no decoding on the UI thread. Probing tags and reading playlists runs on the
  background executor.
- The audio thread never blocks on the UI and never allocates in the cpal callback.

## Logging

Same as Postino (`crates/nastro-app/src/logging.rs`): a small logger behind the `log` facade,
file in `~/.local/state/nastro/nastro.log` on Linux (`$XDG_STATE_HOME`),
`~/Library/Logs/Nastro/` on macOS, `%LOCALAPPDATA%\Nastro\logs\` on Windows, rotated at 5 MB,
10 files. `NASTRO_LOG` overrides the level at startup. Library crates only depend on `log`.
Never log anything per audio buffer above Trace.

## Testing

- Library crates have unit tests next to the code and fixtures under `tests/fixtures/`.
- Audio tests never open a device: the engine writes to an `AudioSink` trait, and tests use an
  in-memory sink. Small audio fixtures are generated in the test itself (WAV) or committed
  (a few KB of FLAC, MP3, Ogg made with ffmpeg from a sine tone).
- `make samples` generates a set of synthetic tracks with tags under `samples/audio/` (needs
  `ffmpeg`), for manual checks. Never commit real music.

## User documentation

The user guide sources are in `site/docs-src/pages/` (one HTML file per page, the sidebar order
in `NAV` in `site/docs-src/build.py`); `site/docs/` is generated. Every change users can notice
updates the guide in the same commit, written from the code (exact labels, defaults, ranges).
Run `make docs` before considering a task done.

## Working rules

- Git: commits are fine; never `git push` unless the user says so explicitly. Commit messages in
  English.
- Keep changes small and traceable to the request. Do not refactor unrelated code.
- Run `make format`, `make lint` and `make test` before considering a task done.
- GPUI evolves quickly. Read the real source of the pinned versions under
  `~/.cargo/registry/src/*/` (`gpui-pre-0.3.7`, `gpui-component-0.7.0`, `gpui-base-0.7.0`) instead
  of relying on memory of its API. The same goes for `symphonia` 0.6, `cpal` 0.18 and `rubato` 5,
  whose APIs changed a lot from older versions.
- Put alternate builds under a subdirectory of `target/`: a full gpui build takes several GB.

## Commands

See `make help`. Main targets: `run`, `build`, `release`, `install`, `install-latest`, `check`,
`lint`, `format`, `test`, `samples`, `docs`.
