# /dev/null GTK client

A native GTK4 / libadwaita client for [dev-null.sh](https://dev-null.sh), written in Rust. It builds on **Linux** and **macOS**.

It talks to the same signed JSON API as the [first-party Mac app](https://github.com/dev-null-sh/mac-os-app). It does not scrape the website or embed a WebView for GitHub sign-in.

On a Mac this is the Homebrew GTK build, not a replacement for the Swift app at [dev-null.sh/apps](https://dev-null.sh/apps).

## Features

- Sign in with GitHub in the system browser (`devnull://linux/auth` on both Linux and macOS)
- Following, Latest, Trending, and Starred logs
- Composer: text, images, polls, scheduling, preview, emoji picker, `:` shortcodes, and `@` mention suggestions
- Commit threads: replies, quotes, related commits, stars, pastéis, share, report, poll voting
- Profiles: activity graph, achievements, XP, follow / follow back, following and followers lists, share link or ID, Go live / End stream, GitHub edit/sync
- Organization pages: mentions, follow, claim, lane color, share
- Notifications with filters, grouped stars, named pairs, and mention avatars
- Remotes: Open / Stashed inbox, search, dock, settings, new org / new remote
- Search (`Ctrl+K` or `/`)
- Settings: Profile, Appearance, Terminal, Safety, Devices
- Keyboard shortcuts (`?`)

The first commit on an account still has to be made on the web. Legal pages and a few profile actions open in the browser.

## Docs

| Page | Covers |
| --- | --- |
| [Architecture](docs/architecture.md) | Crates, signed API, auth, layout, notifications, remotes |
| [Development](docs/development.md) | Build, test, icons, local prefs |
| [Packaging](docs/packaging.md) | Flatpak, desktop file, `devnull://`, Homebrew |

Visual language follows the website’s [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md).

## Requirements

- Rust **1.93+** (`rustup update stable` if `cargo` refuses to build)
- GTK 4.14+ and libadwaita 1.5+
- On Linux: a Secret Service daemon (GNOME Keyring, KWallet, or KeePassXC)
- On macOS: the login Keychain (used automatically)

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

On macOS (Homebrew):

```bash
brew install gtk4 libadwaita pkgconf
```

If `pkg-config` cannot find GTK after Homebrew:

```bash
export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix libffi)/lib/pkgconfig"
```

## Run

```bash
cargo run -p devnull
```

Optional: `DEVNULL_URL` (default `https://dev-null.sh`).

The UI prefers Geist (SIL OFL) and falls back to Adwaita Sans / Adwaita Mono. On Arch: `pacman -S ttf-geist-font` if packaged, or install Geist from [vercel/geist-font](https://github.com/vercel/geist-font).

The app registers itself for `devnull://` on launch so the browser can return after you approve the device. If a browser still swallows the redirect, copy the callback URL and paste it on the sign-in page.

The device key never leaves Secret Service (Linux) or the Keychain (macOS). Revoke it on the web under Settings → Devices.

## Package

Flatpak (Linux, runs across distributions):

```bash
flatpak-builder --user --install --force-clean build-dir flatpak/sh.devnull.Client.yml
flatpak run sh.devnull.Client
```

See [Packaging](docs/packaging.md) for portals, the desktop file, the URL handler, and Homebrew on macOS.

## Architecture

| Crate | Role |
| --- | --- |
| `devnull-api` | Signed `/api/v1` client, SSHSIG, types, 110 req/min budget |
| `devnull-auth` | Ed25519 device key, PKCE, Secret Service or Keychain, `devnull://` handler |
| `devnull` | Relm4 + libadwaita UI |

GTK application id: `sh.devnull.Client`. Authorize `app_id` is `linux:sh.devnull.Client` or `macos:sh.devnull.Client`. Redirect URI is `devnull://linux/auth` on both platforms.

## License

MIT. See [LICENSE](LICENSE). The /dev/null name and product belong to the [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh) project.
