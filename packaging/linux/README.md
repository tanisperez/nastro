# Linux packaging

| File | Purpose |
|---|---|
| `codes.tanis.nastro.desktop` | Desktop entry (`Exec=nastro %U`, audio and playlist MIME types) |
| `nfpm.yaml` | nfpm config shared by the deb and the rpm |
| `package.sh` | Builds the tarball, the deb and the rpm |
| `install-latest.sh` | Installs the latest release's tarball into a prefix (`make install-latest`) |
| `repo.sh` | Builds the signed APT and DNF repositories published on the website |
| `test-repo.sh` | Tests `repo.sh` end to end with podman (Ubuntu and Fedora containers) |

Nastro defines no MIME type of its own: it opens the system audio types and M3U playlists, so
there is no shared-mime-info file here.

## Usage

```sh
packaging/linux/package.sh <version> <path-to-nastro-binary> <out-dir> [x86_64|aarch64]
# example
packaging/linux/package.sh 0.1.0-rc.1 target/release/nastro dist
```

`<version>` has no leading `v`. A pre-release such as `0.1.0-rc.1` becomes `0.1.0~rc.1` in the
deb and rpm (nfpm semver handling). The optional arch defaults to `x86_64`; `aarch64` maps to
`arm64` for the deb (untested, there is no ARM build yet). Output in `<out-dir>`:

- `nastro-<version>-linux-<arch>.tar.gz`: a `/usr`-like prefix (`bin/`, `share/`) plus `LICENSE`,
  `NOTICE` and `README.md`, meant to be copied into `/usr` or `~/.local`.
- `nastro_<version>_amd64.deb` and `nastro-<version>-1.x86_64.rpm`. The file names keep the
  `-` of a pre-release (`0.1.0-rc.1`), only the version inside the packages uses `~`. GitHub
  renames `~` to `.` in uploaded assets, which would break `SHA256SUMS`.

`nfpm` must be on `PATH`. Postino uses nfpm 2.47.0, and so does the release workflow.

## Notes

- No maintainer scripts: dpkg triggers (desktop-file-utils, icon caches) and rpm file triggers
  refresh the desktop and icon caches on modern distributions.
- Dependencies are the runtime libraries the binary links (xcb, xkbcommon, xkbcommon-x11, ALSA)
  or loads with dlopen (wayland-client, wayland-egl, EGL). Verify them with `ldd` on the first real
  binary. The Vulkan loader is only recommended. Package names for deb and rpm are set in
  `nfpm.yaml` (`libasound2` and `alsa-lib` for ALSA).
- nfpm only expands environment variables in content entries that set `expand: true`.
- The scripts are adapted from Postino's and were not run for Nastro yet (there is no binary).

## Repositories

`website.yml` runs `repo.sh` after each stable release (and when it changes) and uploads the
result with the site: `apt/`, `rpm/` and `nastro.asc` (the public key, shared by both). Only the
last 3 stable releases are kept. Secrets: `GPG_PRIVATE_KEY` (armored export) and
`GPG_PASSPHRASE`. Without `GPG_PRIVATE_KEY` the repository steps are skipped and the site still
deploys. Check a change with `packaging/linux/test-repo.sh <dir with a deb and an rpm>`. The base
URL (`nastro.tanis.codes`) is provisional.
