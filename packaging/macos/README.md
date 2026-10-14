# macOS packaging

- `Info.plist`: template for `Nastro.app/Contents/Info.plist`. `bundle.sh` replaces
  `@SHORT_VERSION@` and `@BUILD_VERSION@` (both the numeric `X.Y.Z`, pre-release suffix dropped).
  Declares `CFBundleDocumentTypes` for audio files (MP3, AAC/M4A, FLAC, Ogg, WAV, AIFF) and M3U
  playlists, as an alternate Viewer, so Nastro appears in "Open With" without taking over the
  defaults. Category: music. Minimum macOS is 11.0 (the arm64 floor; gpui targets 10.15).
- `bundle.sh`: builds the app, the `.dmg` and the `.app.tar.gz`.

```sh
packaging/macos/bundle.sh 0.1.0 target/aarch64-apple-darwin/release/nastro dist/
```

Output in `dist/`: `Nastro-<version>-macos-arm64.dmg` (UDZO, with an `/Applications` link) and
`Nastro-<version>-macos-arm64.app.tar.gz`. The bundle is signed ad hoc (`codesign --sign -`), not
with a Developer ID. The `.icns` is built from `packaging/icons/png/` (16 to 1024 px).

Needs macOS tools: `iconutil`, `codesign`, `hdiutil`.

Adapted from Postino's script. Not run on macOS (nor at all) for Nastro yet; the Info.plist
parses with Python `plistlib`.
