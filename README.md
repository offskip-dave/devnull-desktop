# /dev/null for Linux

A native GTK4 / libadwaita client for [dev-null.sh](https://dev-null.sh), written in Rust.

It talks to the same signed JSON API as the [Mac app](https://github.com/dev-null-sh/mac-os-app). It does not scrape the website or embed a WebView for sign-in.

## Features

- Sign in with GitHub in the system browser (`devnull://linux/auth`)
- Following, Latest, Trending, and Starred logs
- Composer: text, images, polls, scheduling, preview, and emoji
- Commit threads: replies, quotes, stars, pastéis, share, report
- Profiles: activity, achievements, follow, share, Go live, GitHub edit/sync
- Notifications with filters
- Remotes inbox (1:1 and org threads)
- Search (`Ctrl+K` or `/`)
- Settings: Profile, Appearance, Terminal, Safety, Devices
- Keyboard shortcuts (`?`)

The first commit on an account still has to be made on the web. Following and followers lists, legal pages, and a few profile actions open in the browser.

## Docs

| Page | Covers |
| --- | --- |
| [Architecture](docs/architecture.md) | Crates, signed API, auth, UI layout |
| [Development](docs/development.md) | Build, test, icons, local prefs |
| [Packaging](docs/packaging.md) | Flatpak, desktop file, `devnull://` |

Visual language follows the website’s [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md).

## Requirements

- Rust 1.93+
- GTK 4.14+ and libadwaita 1.5+
- A Secret Service daemon (GNOME Keyring, KWallet, or KeePassXC)

On Arch / Omarchy:

```bash
sudo pacman -S gtk4 libadwaita
```

On Debian / Ubuntu:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev
```

On Fedora:

```bash
sudo dnf install gtk4-devel libadwaita-devel
```

## Run

```bash
cargo run -p devnull
```

Optional: `DEVNULL_URL` (default `https://dev-null.sh`).

The UI prefers Geist (SIL OFL) and falls back to Adwaita Sans / Adwaita Mono. On Arch: `pacman -S ttf-geist-font` if packaged, or install Geist from [vercel/geist-font](https://github.com/vercel/geist-font).

The app registers itself for `devnull://` on launch so the browser can return after you approve the device. If a browser still swallows the redirect, copy the callback URL and paste it on the sign-in page.

## Package

Flatpak (runs across distributions):

```bash
flatpak-builder --user --install --force-clean build-dir flatpak/sh.devnull.Client.yml
flatpak run sh.devnull.Client
```

See [Packaging](docs/packaging.md) for portals, the desktop file, and the URL handler.

## Architecture

| Crate | Role |
| --- | --- |
| `devnull-api` | Signed `/api/v1` client, SSHSIG, types, 110 req/min budget |
| `devnull-auth` | Ed25519 device key, PKCE, Secret Service, `devnull://` handler |
| `devnull` | Relm4 + libadwaita UI |

The device key never leaves the Secret Service. Revoke it on the web under Settings → Devices.

## License

MIT. The /dev/null name and product belong to the [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh) project.
