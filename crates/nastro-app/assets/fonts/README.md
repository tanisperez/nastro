# Bundled fonts

Embedded in the binary with `include_bytes!` and registered at startup
(`crates/nastro-app/src/theme/mod.rs`), so the look does not depend on the fonts the OS has.
All three families are under the SIL Open Font License 1.1.

## Geist and Geist Mono

The UI and numbers font of the skins and the app chrome, the same files as Postino.

- Source: <https://github.com/vercel/geist-font>, release `v1.7.2`
  (<https://github.com/vercel/geist-font/releases/download/v1.7.2/geist-font-v1.7.2.zip>).
- License: `OFL-Geist.txt`, copied unmodified from the release archive.
- Files: `Geist-Regular.ttf`, `Geist-Medium.ttf`, `Geist-SemiBold.ttf`, `Geist-Bold.ttf`,
  `GeistMono-Regular.ttf`, `GeistMono-Medium.ttf`, `GeistMono-SemiBold.ttf`, the weights the type
  scale in `docs/design-system.md` uses.

## Share Tech Mono

The LCD font: the clock and the badges of the player's display, nowhere else.

- Source: <https://github.com/google/fonts/tree/main/ofl/sharetechmono>, by Carrois Type Design.
- License: `OFL-ShareTechMono.txt`, copied unmodified from that folder. "Share" is a Reserved
  Font Name, so the file is bundled as it is, never modified or renamed.
- File: `ShareTechMono-Regular.ttf`.
