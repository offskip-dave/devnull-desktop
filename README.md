<p align="center">
  <img src="resources/icons/hicolor/scalable/apps/sh.devnull.Client.svg" alt="/dev/null" width="128" height="128">
</p>

# /dev/null GTK client

A native GTK4 / libadwaita client for [dev-null.sh](https://dev-null.sh), written in Rust. It builds on **Linux** and **macOS**.

It talks to the same signed JSON API as the [first-party Mac app](https://github.com/dev-null-sh/mac-os-app). It does not scrape the website or embed a WebView for GitHub sign-in.

On a Mac this is the Homebrew GTK build, not a replacement for the Swift app at [dev-null.sh/apps](https://dev-null.sh/apps).

## Features

- Sign in with GitHub in the system browser (`devnull://linux/auth` on both Linux and macOS); device redeem records terms acceptance
- Following, Latest, Trending, and Starred logs
- Composer: text, images, polls, scheduling, preview, emoji picker, `:` shortcodes, `@` mentions, and `#hashtags`
- Commit threads: replies, quotes, related commits, stars, pastéis, poll voting; more menu for copy link, Send in Remotes, pin / unpin, follow / mute / block, report, and delete (with 30s undo)
- Profiles: activity graph, achievements, XP, follow / follow back, following and followers lists (Follows you chips), share link or ID, Go live / End stream, GitHub edit/sync; a profile photo opens in the viewer; hover-card Remote / mute / block
- Organization pages: commits, mentions, follow, claim, lane color, share
- Notifications with filters, grouped stars, named pairs, and mention avatars
- Remotes: Open / Stashed inbox, search, dock, settings, new org / new remote; Back to present when scrolled far up a thread
- Aside: hot topics (with recap when the API sends one), live following, who to follow
- Search (`Ctrl+K` or `/`): hot topics when the box is empty, then Most relevant and Latest
- Settings: Profile, Appearance (including Halloween, Christmas, and New Year), Terminal, Safety, Devices
- Keyboard shortcuts (`?`)

The first commit on an account still has to be made on the web. Legal pages, headline discussion pages, and a few profile actions open in the browser.

What is ported from the website changelog — every production day from **2026-10-15** (day one) through today — lives in [Changelog parity](docs/changelog-parity.md).

## Docs

| Page | Covers |
| --- | --- |
| [Contributing](CONTRIBUTING.md) | Setup, where to edit, porting from the web, PR checks |
| [Architecture](docs/architecture.md) | Crates, signed API, auth, layout, notifications, remotes |
| [Changelog parity](docs/changelog-parity.md) | Web changelog items: done, partial, waiting, skipped |
| [Development](docs/development.md) | Build, test, icons, GTK CSS, local prefs |
| [Packaging](docs/packaging.md) | Flatpak, AppImage, macOS binary, desktop file, CI scripts |

Visual language follows the website’s [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md).

## Contributing

Bug fixes, features, docs, and packaging help are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md): how to run the app, which crate to touch, how we port changelog items from the website, and the `fmt` / clippy / test gate CI enforces.

Linux is the primary platform; keep macOS (Homebrew GTK) green. Do not add a WebView for sign-in or invent `/api/v1` routes that do not exist yet.

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

```bash
scripts/build-flatpak.sh --install   # Linux; needs flatpak-builder
scripts/build-appimage.sh            # Linux; host GTK 4.14+ / libadwaita 1.5+
scripts/build-macos.sh               # macOS Homebrew GTK binary → dist/
```

Artifacts land in `dist/`. CI also builds them on `main` / `production`, `v*` tags, and manual dispatch. See [Packaging](docs/packaging.md) for portals, the desktop file, the URL handler, AppImage host requirements, and Homebrew notes.

## Architecture

| Crate | Role |
| --- | --- |
| `devnull-api` | Signed `/api/v1` client, SSHSIG, types, 110 req/min budget |
| `devnull-auth` | Ed25519 device key, PKCE, Secret Service or Keychain, `devnull://` handler |
| `devnull` | Relm4 + libadwaita UI |

GTK application id: `sh.devnull.Client`. Authorize `app_id` is `linux:sh.devnull.Client` or `macos:sh.devnull.Client`. Redirect URI is `devnull://linux/auth` on both platforms.

## License

MIT. See [LICENSE](LICENSE). The /dev/null name and product belong to the [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh) project.
