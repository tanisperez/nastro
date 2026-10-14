# Releasing Nastro

Publishing a version means pushing a tag. The `Release` workflow
(`.github/workflows/release.yml`) builds Nastro for every platform, attaches the packages to the
GitHub release of that tag and publishes it. Packages, distribution channels and desktop
integration are described further down.

> Provisional values: the GitHub repository `tanisperez/nastro` and the website
> `nastro.tanis.codes` are pending decisions (see `AGENTS.md`). Every URL below uses them. When
> they are settled, update the README, `site/`, `packaging/` and `.github/workflows/website.yml`
> together, and do it before the first public release: the APT and DNF repository URLs end up in
> users' configuration.

## Versions and tags

- The version lives in one place: `version` under `[workspace.package]` in `Cargo.toml`.
  Versions follow SemVer.
- The tag is `v` plus that version: `v0.1.0`. The workflow fails if they differ.
- A tag with a pre-release suffix (`v0.1.0-rc.1`, `v0.2.0-beta.2`) is accepted when the part
  before the suffix matches the version (or when the version itself has the same suffix). It is
  published as a GitHub pre-release: built and attached like any release, but never marked as
  the latest one.

## Steps

The work of a version is tracked by its GitHub milestone (`gh issue list --milestone X.Y.Z`).

1. Check that every issue of the milestone is closed and `make format`, `make lint` and
   `make test` pass on `main`.
2. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, and commit.
3. Tag and push:

   ```
   git tag v0.1.0
   git push origin main v0.1.0
   ```

4. Create the release as a **draft**, with the notes in English Markdown (what changes and which
   milestone issues went in):

   ```
   gh release create v0.1.0 --draft --verify-tag --title v0.1.0 --notes-file notes.md
   ```

5. Wait for the `Release` workflow (`gh run watch`). It uploads the assets to the draft and then
   publishes it. If the draft does not exist yet when the builds finish, the workflow creates one
   with placeholder notes; edit them afterwards with `gh release edit v0.1.0 --notes-file notes.md`.

6. For a stable release, run the `Website` workflow by hand (`gh workflow run website.yml`) once
   the release is published. It rebuilds the APT and DNF repositories with the new packages. Its
   `release: published` trigger does not fire here, because the release workflow publishes with
   `GITHUB_TOKEN`, and events caused by that token never start other workflows.

The release stays a draft until every asset is uploaded, so nobody sees a half-built release.

## Assets

| Asset | Content |
|---|---|
| `nastro-X.Y.Z-linux-x86_64.tar.gz` | Linux binary plus desktop entry and icons in a `/usr`-like layout |
| `nastro_X.Y.Z_amd64.deb` | Debian and Ubuntu package |
| `nastro-X.Y.Z-1.x86_64.rpm` | Fedora and openSUSE package |
| `Nastro-X.Y.Z-macos-arm64.dmg` | macOS app, Apple silicon, ad-hoc signed |
| `Nastro-X.Y.Z-macos-arm64.app.tar.gz` | The same app as an archive |
| `Nastro-X.Y.Z-windows-x86_64-setup.exe` | Per-user Windows installer |
| `Nastro-X.Y.Z-windows-x86_64-portable.zip` | Portable Windows build for machines where running an installer is not allowed: `nastro.exe`, `LICENSE` and `NOTICE` at the top level of the zip, nothing to install |
| `SHA256SUMS` | SHA-256 of every asset above |

The portable zip is built by the Windows job right after the installer, with `Compress-Archive`,
from the same `nastro.exe`. It lands in `SHA256SUMS` like every other file in `dist`.

Linux binaries are built on Ubuntu 22.04 (glibc 2.35). Inside the deb and rpm a pre-release
version uses a tilde (`0.1.0~rc.1`) so it sorts before the final release. Every file name keeps
the dash, since GitHub renames a `~` in an asset name to `.`.

The packaging scripts live in `packaging/` (one README per platform) and run in the workflow.
`make dist` builds the packages for the current OS locally into `target/dist` (Linux needs
`nfpm` on `PATH`, macOS the Xcode command line tools). The Windows installer is compiled with
Inno Setup, see `packaging/windows/README.md`.

The Linux build needs the ALSA development files (`libasound2-dev` on Debian and Ubuntu) and the
packages depend on `libasound2` (deb) and `alsa-lib` (rpm).

## When something fails

- **Version mismatch**: delete the tag (`git push origin :refs/tags/v0.1.0`, `git tag -d v0.1.0`),
  fix `Cargo.toml`, commit and tag again.
- **A build fails**: fix it on `main`, then move the tag to the fixed commit (delete and push it
  again). The draft keeps its notes; re-running the workflow replaces the assets (`--clobber`).
- **Only the publish job failed**: re-run the failed jobs from the Actions page.

## Why releases work this way

- **No backend.** Only free GitHub features (Actions on a public repo, Releases, Pages) plus the
  free community registries (a Homebrew tap, winget-pkgs).
- **Pre-releases** (`v0.3.0-beta.1`) are built and attached but not distributed: the APT and DNF
  repositories only take stable releases.
- **Trigger:** `release.yml` runs on `push: tags: ["v*"]`. `build.yml` is the per-push CI.
- **Draft first.** The release is created as a draft and the workflow publishes it only when
  every asset is there, so neither users nor the package repos see a half-built release.
- **Targets:** Linux x86_64, Windows x86_64 and macOS arm64. ARM Linux and ARM Windows can come
  later (free `ubuntu-22.04-arm` and `windows-11-arm` runners for public repos).
- **App id:** `codes.tanis.nastro`, used for the `.desktop` file, the Wayland `app_id`, the macOS
  bundle id and the Windows AppUserModelID.
- **nfpm** builds the deb and the rpm from one YAML file, so there is no per-distro packaging
  toolchain to learn.
- Linux packages declare these dependencies: `libxkbcommon`, `libxkbcommon-x11`,
  `libwayland-client`, `libxcb`, `libasound2` (`alsa-lib` on rpm), EGL, and recommend the Vulkan
  loader. Check them with `ldd` against the first real binary.

The workflow, in short:

```
tag v0.3.0 pushed
  +- check: tag == Cargo version, prerelease = tag contains "-"
      +- build-linux   (ubuntu-22.04)   cargo build --release, tar.gz, nfpm deb + rpm
      +- build-macos   (macos-latest)   cargo build --release, .app bundle, ad-hoc codesign, dmg, app.tar.gz
      +- build-windows (windows-latest) cargo build --release, Inno Setup installer, portable zip
          +- publish: SHA256SUMS, upload to the draft, mark it published
              +- website (by hand, stable only): website + APT + DNF repos on Pages
```

Packaging files live in `packaging/`: `linux/` (nfpm, desktop file, repo scripts), `macos/`
(`Info.plist`, bundle script), `windows/` (`nastro.iss`) and `icons/` (the SVG sources and the
PNG sizes).

## Desktop integration

Nastro defines no file type of its own. It registers as a handler for audio files and M3U
playlists, so it shows up in "Open with" menus.

| OS | What the package installs |
| --- | --- |
| Linux (deb, rpm) | `/usr/bin/nastro`, `/usr/share/applications/codes.tanis.nastro.desktop` (with the audio and playlist `MimeType=` list), icons in `/usr/share/icons/hicolor/*/apps/`. The distro triggers refresh the desktop database and icon caches, nothing to script. |
| macOS | `Nastro.app` with `Info.plist`: `CFBundleIdentifier`, `CFBundleDocumentTypes` for audio files and playlists (alternate Viewer, so it never takes over the defaults) and an `.icns` icon. |
| Windows | Inno Setup, **per user** install (`%LOCALAPPDATA%\Programs\Nastro`, no admin). Start menu shortcut, optional desktop shortcut, an optional and unchecked-by-default task that registers "Open with" entries for audio files and playlists under `HKCU\Software\Classes`, uninstaller in "Apps", and `CloseApplications=yes`. The portable zip registers nothing. |

The Wayland `app_id` is the app id, so the window matches the `.desktop` file (right icon and
name in the dock and alt-tab).

## Distribution channels

### Linux

| Channel | How |
| --- | --- |
| Debian and Ubuntu | A signed APT repo on GitHub Pages (`apt-ftparchive` and GPG). Users add the key and one `.sources` file once, then `apt upgrade` updates. |
| Fedora and openSUSE | A signed DNF repo on the same site (`createrepo_c` and GPG, `rpm --addsign`). One `.repo` file, then `dnf upgrade`. |
| Everyone else | The tarball, plus the deb and rpm as direct downloads. |

The APT and DNF repos share one GPG key (private key and passphrase as secrets
`GPG_PRIVATE_KEY` and `GPG_PASSPHRASE`, public key published on the site as `nastro.asc`). There
is no `gh-pages` branch: a branch holding the deb and rpm files would make every `git clone` of
Nastro download them too. The site is deployed from Actions (`actions/deploy-pages`) and rebuilt
from scratch each time: the job builds the website, downloads the deb and rpm of the last 3
stable releases from GitHub Releases, regenerates and signs the APT and DNF metadata, and
uploads the whole site. It runs on pushes to `main` that touch `site/`, and by hand after each
stable release (step 6 of "Steps"). GitHub Pages limits: 1 GB per site, 100 MB per file, 100 GB
of bandwidth per month (soft). Older versions stay downloadable from Releases. The repo URLs use
the website domain (provisional), so moving the hosting later never breaks users' apt and dnf
configuration.

Considered and left for later: Flathub (builds from source offline, reviewed submission, a good
second step once the app id and name are final), an AUR `nastro-bin` package (Postino's
`packaging/aur` is the template), AppImage, PPA and COPR.

### macOS

- **Homebrew:** an own tap with a cask pointing at the release dmg. The official `homebrew/cask`
  requires signed and notarized apps (see Signing). Not set up yet.
- **Direct:** the dmg from Releases or from the site.

### Windows

- **winget:** a PR to `microsoft/winget-pkgs` per release. The first submission is manual and
  reviewed by Microsoft. Not set up yet.
- **Direct:** the setup exe from Releases.
- **Portable:** `Nastro-X.Y.Z-windows-x86_64-portable.zip` from Releases, for machines where
  running an installer is not allowed. Unzip anywhere and run `nastro.exe`. Settings and logs
  stay where the installed app keeps them, so a portable copy still writes outside its folder.

## The website

The provisional `nastro.tanis.codes` is the landing page, the install page, the user
documentation and the APT and DNF repos, served by GitHub Pages.

- Plain static HTML, CSS and a little JavaScript under `site/`, no dependencies. The only build
  step is `site/docs-src/build.py` (Python standard library), which the workflow runs to
  generate `site/docs/` and which fails the deploy on a broken link or an em dash
  (`site/README.md`).
- Geist and Geist Mono are served from the site (SIL OFL 1.1).
- English only for now.

## Updates

There is no in-app updater in Nastro yet: package managers own updates on Linux, and macOS and
Windows users download a new version by hand. Postino has one (the `postino-update` crate, the
`latest.json` manifest written by its release workflow and the `POSTINO_UPDATER=github` build
switch, see Postino's `docs/releasing.md`). It can be ported later: it needs the crate, the glue
in the app, a `latest.json` step in the `publish` job and the `/relaunch=1` run entry in
`nastro.iss`. Until then the workflow writes no manifest.

## Signing

The builds are unsigned, which causes friction on first install only:

| OS | Without signing | Fix and cost |
| --- | --- | --- |
| macOS | Gatekeeper blocks the first launch of a downloaded app. On macOS 15 and later the user goes to System Settings, Privacy & Security, "Open Anyway" (or runs `xattr -dr com.apple.quarantine /Applications/Nastro.app`). | Apple Developer Program, 99 USD per year: Developer ID signing plus notarization in CI. Also unlocks the official `homebrew/cask`. |
| Windows | SmartScreen shows "Windows protected your PC" until the installer builds reputation: "More info, Run anyway". winget accepts unsigned installers. | [SignPath Foundation](https://signpath.org) gives free code signing to open source projects, or Azure Trusted Signing (about 10 USD per month). |
| Linux | Nothing; the APT and DNF repos are GPG signed for free. | None. |

## Secrets

Created as each channel is set up, all free: `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE`, and later
`HOMEBREW_TAP_TOKEN` and `WINGET_TOKEN`.
