# Architecture

How Nastro is put together and why. For the project rules (working style, performance,
logging) read `AGENTS.md`; for the look, `docs/design-system.md`; for the skin file,
`docs/skin-format.md`; for the audio engine, `docs/audio.md`; for gpui pitfalls,
`docs/gpui-notes.md`. The design handoff that started the project, with its artboards, is kept
under the git tag `design-mockups`; what it decided lives in these documents.

## Principles

- **A thin UI.** Everything except `nastro-app` is plain Rust, testable with `cargo test`: no
  window, no GPU, no audio device. UI logic that can be plain Rust lives under
  `crates/nastro-app/src/state/` with unit tests; views render and forward input.
- **Skins are data.** The player, the equalizer and the playlist are drawn from the tokens of
  the active `skin.toml`. The app chrome (menus, Preferences, About) has its own fixed palette.
- **An idle player does no work.** Nothing animates unless music plays, and the audio engine
  sleeps while paused or stopped.
- **Readable code over clever code.** Small modules, doc comments on public items, no `unsafe`
  (one Win32 module aside, see "Windows and docking").

## Crates

```
nastro-app ──> nastro-audio ──> nastro-core
     ├──────> nastro-skin
     ├──────> nastro-dock
     └──────> nastro-core
```

No cycles, and no crate depends on `nastro-app`. The handoff proposed that `nastro-skin` and
`nastro-dock` depend on `nastro-core`; they ended up needing nothing from it, so they stand
alone.

| Crate | Responsibility |
| --- | --- |
| `nastro-core` | Tracks and their tags, playlists with selection and a current track that survives every edit, the play order (shuffle and repeat), extended M3U read and write, equalizer settings and presets, time formatting. No IO. |
| `nastro-skin` | The skin model, the `skin.toml` parser (missing tokens filled from Silver with a warning), the four built-in skins, user skins from a folder, and the contrast checks of the design rules. Its own `Color` type, no gpui. |
| `nastro-dock` | Panel geometry: the four layout presets, magnetic snapping, shade and resize moving the docked panels, docked groups. Pure functions over rectangles. |
| `nastro-audio` | The `Decoder` trait and its Symphonia implementation, tag probing, the equalizer, the analyzer and the `Player` engine with its cpal output. See `docs/audio.md`. |
| `nastro-app` | The gpui binary (`nastro`): models as gpui entities, views, settings, session, logging, translations. |

## The app

`crates/nastro-app/src/`:

| Path | What |
| --- | --- |
| `main.rs` | Logging, settings, language, the session, the models, the main window. |
| `app/` | The models, as gpui entities, and the commands that involve several of them. |
| `app/transport.rs` | The engine handle, its status, volume and balance, and the per-frame data. |
| `app/library.rs` | The playlist tabs, the loaded track, the play order, each tab's filter. |
| `app/equalizer.rs` | The equalizer settings, AUTO and the last preset. |
| `app/shell.rs` | Settings, the skins on offer, the panel layout, the window size. |
| `app/commands.rs` | Play, pause, next, the gapless queue, opening files, probing tags. |
| `app/persist.rs` | Saving the session a second after the last change, and on quit. |
| `state/` | Plain Rust with tests: settings and session files, language, numbers, display texts, slider arithmetic, file collection. |
| `theme/` | Fonts, the chrome palette, the skin metrics, the active skin as gpui colors. |
| `views/` | The main window, the three panels, the display, the menus, Preferences, About. |
| `views/components/` | The skin's building blocks: frame, title bar, groove, buttons, sliders, menu trigger, tracked text. |
| `actions.rs`, `keymap.rs` | Every action, and the key bindings. |
| `locales/` | Translations, one YAML file per area (`docs/i18n.md`). |

### Models and views

The four models (`Transport`, `Library`, `EqModel`, `Shell`) are gpui entities stored in the
`Models` global, so any view or command reaches them from a `cx`. Views observe the models they
show and repaint when those notify.

Rendering cost is kept proportional to what changes:

- Each panel is a cached view (`AnyView::cached`): it is rebuilt only when it notifies.
- While playing, the frame task updates the transport's status and notifies only the `Frame`
  entity, which the `Display` view (clock, visualizer, marquee, seek bar) observes. The
  transport itself notifies when the play state, the track or the volume change, so the rest of
  the player is not rebuilt 30 times per second.
- The playlist renders only its visible rows (`uniform_list`); the filter result is cached
  under the playlist's generation counter.

Measured on 2026-10-14 (release build, Linux, Intel Iris Xe, the default layout): about 7 % of
one core while playing, no measurable main thread work while paused, RSS about 130 MB, like
Postino. Most of the playing cost is gpui drawing the whole window on every frame, so it grows a
little with each panel shown; the brushed metal is one image per panel for that reason (a quad
per stripe cost about 2 % of a core on its own).

### One instance

On Linux and macOS a second launch (a file opened with Nastro from the file manager) connects to
the first one's local socket (`$XDG_RUNTIME_DIR/nastro.sock` on Linux), hands it its files and
exits (`instance.rs`); a launch without files brings the window to the front. A stale socket
left by a crash is replaced. Windows has no Unix sockets in `std`, so every launch there is its
own instance for now.

### Engine events

The engine sends `PlayerEvent`s through an async channel that a task of the transport awaits:
no polling. `Started` queues the next track in the play order for gapless playback
(`commands::requeue`), `Advanced` moves the current track to the queued one, `Finished` plays
the next one or stops at the end, `Error` skips the track unless every track of the playlist
failed in a row. Editing the playing playlist, or toggling shuffle or repeat, queues again.

## Windows and docking

The handoff's plan, implemented for version 1 on every platform:

- One transparent window (`WindowBackgroundAppearance::Transparent`, no title bar, client side
  decorations) holds the whole group. The panels are positioned inside it from the
  `nastro-dock` layout, in window coordinates scaled by the skin scale, with a 16 px margin
  (`WINDOW_MARGIN`) for their shadows. The panel shadow must end inside that margin
  (`SkinTheme::panel_shadow_reach`, checked by a test): the window edge would cut it in a
  straight line around the whole group.
- gpui-pre 0.3.7 cannot move a window programmatically, and Wayland does not let a client know
  or set its position (the `ext-placement` protocol is a draft, and would only suggest a
  position before the window is mapped). So the layout never goes negative: the window's top
  left corner is fixed, and the window size is the bottom right corner of the displayed panels
  plus the margin. The window is resized (`Window::resize`) whenever the layout changes.
- Dragging the player's title bar or menu bar moves the window (`start_window_move`). Dragging
  the equalizer's or the playlist's title bar moves that panel inside the window, snapping
  magnetically (`Layout::move_panel`). The playlist's corner grip resizes it.
- Clicks outside the panels go to the windows behind on Wayland and on Windows
  (`platform::limit_input`). On Wayland it is `Window::set_input_region`. gpui implements that
  only on Wayland, so on Windows Nastro sets a window region itself (`SetWindowRgn`, in
  `platform/win32.rs`, the only `unsafe` code). A window region also clips drawing, so there it
  covers each panel plus its shadow, and it is in device pixels, so it is set again when the
  scale factor changes. On X11 and macOS the whole window still takes input. The region is
  set only once the window size has settled for a frame (`NastroWindow::settle_region`):
  setting it during a resize on Wayland shows a stretched frame (`docs/gpui-notes.md`).
- Menus are drawn inside the window, so while one is open the window grows downwards to
  `MENU_ROOM` and takes input everywhere; it shrinks back when the menu closes.
- Separate OS windows per panel on X11, macOS and Windows are left for later: they need a
  `set_position` in gpui (a fork or an upstream change).

The main window does not use gpui-kit's `Root`: on Linux with client decorations it draws a
window border, a shadow inset and resize edges, and paints the theme background, all of which
would show around the panels. Text fields still work without it; tooltips and gpui-kit dialogs
do not, so the main window has none. Preferences and About are ordinary chrome windows with
`Root` and `TitleBar`, like Postino's.

## Files Nastro writes

| File | Content |
| --- | --- |
| `<config dir>/nastro/settings.toml` | Settings, written on every change. `~/.config/nastro/` on Linux. |
| `<config dir>/nastro/skins/` | The user's own skins (read only). |
| `<state dir>/nastro/session.toml` | Volume, balance, shuffle, repeat, equalizer, panel layout, playlist order, the loaded track, the window position (used on macOS and Windows only: Wayland never reports it and KWin under X11 places new windows itself). `~/.local/state/nastro/` on Linux; the local data folder on macOS and Windows. |
| `<data dir>/nastro/playlists/playlist-<n>.m3u8` | One extended M3U file per playlist tab, named with `#PLAYLIST`. `~/.local/share/nastro/playlists/` on Linux. |
| Logs | See `AGENTS.md`, "Logging". |

## Decisions and their reasons

| Decision | Why |
| --- | --- |
| Our own audio pipeline instead of `rodio` | The equalizer, the visualizer tap and gapless playback need control of every stage. See `docs/audio.md`. |
| One transparent window for the group | gpui cannot place windows and Wayland forbids it; the handoff's plan. |
| Menus as gpui-kit popup menus in the chrome palette | Menus are app chrome by the design's two layers, and gpui-kit gives keyboard navigation, checks, submenus and shortcut hints. |
| Session separate from settings | Settings are choices; the session changes while listening and is written often, in the state folder. |
| Playlists as `.m3u8` files | Any other player can open them, and they diff cleanly. |
| Letter spacing by one box per character | gpui text has no letter spacing; only for short uppercase labels. |
| The marquee offset follows the playing position | It scrolls only while music plays and freezes on pause with no timer of its own. |

## Not done yet

Tracked in the release milestone once the repository exists on GitHub (see `AGENTS.md`,
"Pending decisions"):

- Classic `.wsz` skins (sprites, `pledit.txt`, `viscolor.txt`, `region.txt`).
- Keyboard focus rings on the skin's controls (the `focus` token is converted but unused).
- The clock glow and the title text shadow of the design: gpui has no text shadow.
- Reordering playlist rows by dragging (Alt+Up and Alt+Down move the selection), and a context
  menu on rows.
- A single instance on Windows (named pipes).
- Opus, WavPack and APE decoding; the in-app updater (`docs/releasing.md`).
