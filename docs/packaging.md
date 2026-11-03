# Packaging

Install layout, Flatpak, AppImage, desktop entry, and macOS Homebrew notes. Contributor workflow lives in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Scripts and CI

From the repo root:

| Script | Output |
| --- | --- |
| `scripts/build-flatpak.sh` | `dist/devnull.flatpak` (pass `--install` to install for the user) |
| `scripts/build-appimage.sh` | `dist/devnull-x86_64.AppImage` (or `aarch64`) |
| `scripts/build-macos.sh` | `dist/devnull-macos-$(uname -m)` |

The **Packaging** GitHub Actions workflow (`.github/workflows/packaging.yml`) runs those targets on `workflow_dispatch`, pushes to `main` / `production`, and `v*` tags. It uploads artifacts for ~14 days. PR CI stays on the lighter [CI](../.github/workflows/ci.yml) workflow (fmt / clippy / tests only).

Dependabot (`.github/dependabot.yml`) opens weekly PRs for Cargo and GitHub Actions.

## Flatpak

Manifest: `flatpak/sh.devnull.Client.yml`.

```bash
scripts/build-flatpak.sh --install
# or manually:
flatpak-builder --user --install --force-clean build-dir flatpak/sh.devnull.Client.yml
flatpak run sh.devnull.Client
```

It uses `org.gnome.Platform` 48, the Rust SDK extension, and `cargo build --release --locked -p devnull`. Finish args:

- Network (API)
- Wayland / fallback X11
- DRI
- `org.freedesktop.secrets` (device key)
- `org.freedesktop.Notifications`
- `org.freedesktop.portal.Desktop` (file/open)

The Flatpak install copies:

- `/app/bin/devnull`
- `resources/sh.devnull.Client.desktop`
- `resources/sh.devnull.Client.metainfo.xml`
- `resources/icons/hicolor`

When `FLATPAK_ID` is set, the app skips writing a user desktop file; the packaged one already owns `x-scheme-handler/devnull`.

Flatpak is the portable Linux path: any distro that can install Flatpak and the GNOME 48 runtime.

## AppImage

```bash
scripts/build-appimage.sh
```

The AppImage **does not bundle GTK or libadwaita**. It links against the host’s libraries so themes stay consistent. The host needs:

- GTK **4.14+** and libadwaita **1.5+**
- glibc at least as new as the build host (CI builds on Ubuntu 24.04)
- a Secret Service daemon

That roughly means GNOME 46+ (Ubuntu 24.04+, Fedora 40+, recent Arch / openSUSE). Older hosts should use Flatpak instead.

## Desktop entry

`resources/sh.devnull.Client.desktop` is the installable launcher:

- `Exec=devnull %u` so `devnull://` callbacks are passed as arguments
- `MimeType=x-scheme-handler/devnull;`
- `StartupWMClass=sh.devnull.Client`

A cargo-run session on Linux also writes `~/.local/share/applications/sh.devnull.Client.desktop` pointing at the current binary, then tries `update-desktop-database`, `xdg-mime`, and `xdg-settings` so a browser can return after GitHub approval. The desktop file passes `%u` into the same binary; GApplication uniqueness forwards that URI to the waiting window. A second process also writes `~/.local/share/sh.devnull.Client/callback.uri` so redeem still works if the session bus is missing.

On macOS, launch compiles `~/Library/Application Support/sh.devnull.Client/DevNull Callback.app` (AppleScript + `devnull://` URL types), registers it with Launch Services, and sets it as the default handler for `devnull://`. The first-party Mac app (`/Applications/DevNull.app`) claims the same scheme for `devnull://mac/auth`; without claiming it, GitHub's `devnull://linux/auth` callback never reaches this GTK client. The helper only writes the callback file — the waiting window polls it. If the browser still swallows the redirect, paste the callback URL on the sign-in page.

## AppStream

`resources/sh.devnull.Client.metainfo.xml` is the Flathub / GNOME Software metadata (`sh.devnull.Client`).

## Distro packages

A native package should install:

| File | Destination |
| --- | --- |
| `target/release/devnull` | `/usr/bin/devnull` |
| `resources/sh.devnull.Client.desktop` | `/usr/share/applications/` |
| `resources/sh.devnull.Client.metainfo.xml` | `/usr/share/metainfo/` |
| `resources/icons/hicolor/` | `/usr/share/icons/hicolor/` |

Runtime dependencies: GTK 4, libadwaita, a Secret Service implementation, and optional Geist fonts. The binary needs Rust 1.93+ to build.

## macOS (Homebrew)

This is a GTK build, not a `.app` / `.dmg` of the first-party Mac client.

```bash
brew install gtk4 libadwaita pkgconf
scripts/build-macos.sh
# or:
cargo build --release -p devnull
cargo run -p devnull
```

If `pkg-config` cannot find GTK after install:

```bash
export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix libffi)/lib/pkgconfig"
```

The device key is stored in the login Keychain as a generic password (`service=sh.devnull.Client`, `account=device-key`). Prefs live under `~/Library/Application Support/sh.devnull.Client/`.
