# Nastro

A lightweight, skinnable desktop music player, without accounts and without telemetry. It plays
the audio files on your computer. The player, a 10-band equalizer and the playlist dock together
with magnetic snapping, and the look is a skin: four are built in (Silver, Graphite, Porcelain
and Nebula), you can write your own as a plain `skin.toml`, and importing classic skins is
planned.

[Website](https://nastro.tanis.codes) · [Download](https://nastro.tanis.codes/install.html) ·
[Documentation](#documentation) (the repository and the website addresses are provisional)

Screenshots coming soon.

> Status: early development. Linux comes first, and the Windows and macOS builds have not been
> verified yet.

## Goals

- Cross-platform desktop app: Linux, Windows and macOS.
- Written in Rust, with the UI built on [`gpui`](https://crates.io/crates/gpui), the
  GPU-accelerated framework behind the Zed editor.
- A classic layout that you can restyle: player, equalizer and playlist panels, docked together
  with magnetic snapping.
- **No login, no telemetry, no online sync.** It plays local files and nothing leaves your
  machine.
- Its own audio pipeline (decoding, resampling, equalizer, analysis and output), so the
  equalizer, the visualizer and gapless playback stay under our control.
- Light: an idle window does no work.
- Fully open source.

## Building from source

Install Rust with [`rustup`](https://rustup.rs). The toolchain version is pinned in
`rust-toolchain.toml` and installed on the first build. Then install the system dependencies for
your platform:

- **Linux** (Debian and Ubuntu package names; on Fedora use `alsa-lib-devel` instead of
  `libasound2-dev`):

  ```
  sudo apt install build-essential pkg-config cmake libfontconfig-dev libfreetype-dev \
      libwayland-dev wayland-protocols libxkbcommon-dev libxkbcommon-x11-dev \
      libx11-xcb-dev libxcb1-dev libvulkan-dev libasound2-dev
  ```

- **macOS**: Xcode, with its command line tools.
- **Windows**: the Visual Studio Build Tools with the "Desktop development with C++" workload.

Build and run:

```
git clone https://github.com/tanisperez/nastro.git
cd nastro
cargo run                    # debug build
cargo build --release        # optimized binary in target/release/nastro
cargo test --all-targets     # run the tests
```

On Linux and macOS the `Makefile` wraps the usual commands (`make run`, `make release`,
`make test`, `make lint`, ...). `make samples` generates a set of synthetic test tracks (needs
`ffmpeg`). Run `make help` for the full list.

## Documentation

The user guide is at <https://nastro.tanis.codes/docs/>. Reference documents in this repository:

- [Skin format](docs/skin-format.md): how to write a `skin.toml`.
- [Audio](docs/audio.md): the audio pipeline.
- [Releasing](docs/releasing.md): how versions are built and published.
- [Architecture](docs/architecture.md): the crates and how they fit together.

## License

Copyright 2026 Estanislao Pérez Nartallo.

Nastro is licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE).
