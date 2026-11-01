# Packaging

## Flatpak

Manifest: `flatpak/sh.devnull.Client.yml`.

```bash
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

## Desktop entry

`resources/sh.devnull.Client.desktop` is the installable launcher:

- `Exec=devnull %u` so `devnull://` callbacks are passed as arguments
- `MimeType=x-scheme-handler/devnull;`
- `StartupWMClass=sh.devnull.Client`

A cargo-run session on Linux also writes `~/.local/share/applications/sh.devnull.Client.desktop` pointing at the current binary, then tries `update-desktop-database`, `xdg-mime`, and `xdg-settings` so a browser can return after GitHub approval. The desktop file passes `%u` into the same binary; GApplication uniqueness forwards that URI to the waiting window. A second process also writes `~/.local/share/sh.devnull.Client/callback.uri` so redeem still works if the session bus is missing.

On macOS, launch compiles `~/Library/Application Support/sh.devnull.Client/DevNull Callback.app` (AppleScript + `devnull://` URL types) and registers it with Launch Services. The helper writes the same callback file, then starts the binary. If the browser still swallows the redirect, paste the callback URL on the sign-in page.

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
cargo build --release -p devnull
cargo run -p devnull
```

If `pkg-config` cannot find GTK after install:

```bash
export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix libffi)/lib/pkgconfig"
```

The device key is stored in the login Keychain as a generic password (`service=sh.devnull.Client`, `account=device-key`). Prefs live under `~/Library/Application Support/sh.devnull.Client/`.
