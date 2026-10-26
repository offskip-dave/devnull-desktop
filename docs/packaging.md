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

A cargo-run session also writes `~/.local/share/applications/sh.devnull.Client.desktop` pointing at the current binary, then tries `update-desktop-database`, `xdg-mime`, and `xdg-settings` so a browser can return after GitHub approval.

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

Runtime dependencies: GTK 4, libadwaita, a Secret Service implementation, and optional Geist fonts.
