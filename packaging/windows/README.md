# Windows packaging

`nastro.iss` is an Inno Setup 6 script. Defines, passed to `ISCC`:

- `AppVersion`: full version, may be `0.1.0-rc.1` (the numeric file version is derived).
- `SourceExe`: path to the release `nastro.exe`.
- `OutputDir`: output folder (default `Output`).

```bat
ISCC.exe /DAppVersion=0.1.0 /DSourceExe=..\..\target\release\nastro.exe /DOutputDir=..\..\dist nastro.iss
```

Produces `Nastro-<version>-windows-x86_64-setup.exe`. Per-user install in
`%LOCALAPPDATA%\Programs\Nastro`, no admin. Start menu shortcut (and optional desktop one) with
AppUserModelID `codes.tanis.nastro`. The "Open audio files and playlists with Nastro" task is
unchecked by default: when ticked it registers Nastro as an "Open with" handler for `.mp3`,
`.flac`, `.ogg`, `.oga`, `.wav`, `.aiff`, `.aif`, `.m4a`, `.aac`, `.m3u` and `.m3u8` under HKCU,
without taking over the current default. The `AppId` GUID must never change. Languages: English,
Spanish, Italian (Inno bundles no Galician).

Adapted from Postino's script. Not compiled nor tested for Nastro yet (not possible on Linux
without wine).

## Portable zip

There is no script for it: the `Package the portable zip for Windows` step of
`.github/workflows/release.yml` builds `Nastro-<version>-windows-x86_64-portable.zip` with
`Compress-Archive`, right after the installer, into the same `dist` folder. It holds `nastro.exe`,
`LICENSE` and `NOTICE` at the top level and is meant for machines where running an installer is
not allowed. Settings and logs stay where the installed app keeps them. To make one by hand:

```powershell
Compress-Archive -DestinationPath dist\Nastro-0.1.0-windows-x86_64-portable.zip `
  -Path target\release\nastro.exe, LICENSE, NOTICE
```
