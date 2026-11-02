# Development

Day-to-day build, check, icons, and GTK CSS notes. For how to contribute (porting from the website, PR expectations, where to edit), see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Build

From the repo root:

```bash
cargo run -p devnull
```

Release:

```bash
cargo build --release -p devnull
```

Point at a local or staging API:

```bash
DEVNULL_URL=http://127.0.0.1:8000 cargo run -p devnull
```

You still need GTK 4 and libadwaita. Linux also needs a Secret Service daemon; macOS uses the login Keychain. See the [README](../README.md#requirements) for distro and Homebrew packages.

The workspace `rust-version` is **1.93** (Relm4 0.11). If cargo says your rustc is too old:

```bash
rustup update stable
```

On macOS, if `pkg-config` cannot find GTK after `brew install gtk4 libadwaita pkgconf`:

```bash
export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix libffi)/lib/pkgconfig"
```

Geist is optional. Without it the UI falls back to Adwaita Sans / Adwaita Mono.

On macOS, `cargo run -p devnull --release` is the usable build. Debug GTK on Quartz is slow. Cmd+Q quits; the titlebar traffic lights move, minimize, and zoom/fullscreen the window.

## Check

CI on `main` / `production` and pull requests runs format, clippy (`-D warnings`), and tests on Ubuntu (`test`) and clippy/tests on macOS (`macos`), both with `--locked`. Run the same locally before opening a PR (see [Contributing](../CONTRIBUTING.md#checks-before-a-pull-request)):

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

`devnull-api` includes a signing test (uses `ssh-keygen` when installed). A live decode against `https://dev-null.sh` is ignored in CI:

```bash
cargo test -p devnull-api -- --ignored
```

## Layout of the UI crate

```
crates/devnull/src/
  main.rs      GTK app, devnull:// ingest
  app.rs       Relm4 component: navigation, messages, composers
  ui.rs        Widget builders
  style.css    Injected as global CSS
  prefs.rs     Theme, hover, chat dock, pins, reactions, local JSON
  body.rs      Commit body rendering
  images.rs    Avatars and attachments
  emoji.rs     Composer emoji picker
```

Icons are loaded from `resources/icons` at runtime via `gtk::IconTheme`. The desktop file and AppStream metainfo live next to them under `resources/`.

## Icons

Action icons are Lucide drawings, converted to **filled** symbolic paths so GTK does not paint strokes as solid blobs.

- Source: Lucide (ISC), same set as the website’s `lucide-react`
- Fill: `#2e3436` (GTK recolors symbolic icons)
- Conversion: Inkscape `object-stroke-to-path` with `INKSCAPE_PROFILE_DIR` set to a writable directory

```bash
export INKSCAPE_PROFILE_DIR="$PWD/.inkscape-profile"
mkdir -p "$INKSCAPE_PROFILE_DIR"
inkscape --actions="select-all;object-stroke-to-path" \
  -o "$INKSCAPE_PROFILE_DIR/out.svg" \
  resources/icons/hicolor/scalable/actions/dn-example-symbolic.svg
```

Then keep only the filled `<path>` elements in a 24×24 `viewBox` SVG. See `resources/icons/ATTRIBUTION.md`.

Do not drop a stroke-only Lucide file into `actions/` and expect it to render correctly.

## Design constraints

Keep the GTK UI aligned with the website. In particular:

- Feed column 620px (Remotes 940px, fluid below 768px); nav 256 / 80; aside 320
- Lane color only on the graph, avatar ring, and handle dot — except quoted remotes authors (website ReplyGhost) and notification actor rails / rings / icons
- Commit Blue for primary actions
- No card chrome on the timeline
- No WebView for GitHub OAuth

## GTK CSS

The stylesheet is GTK CSS, not the web. These properties are ignored or invalid: `max-width`, `max-height`, `color-scheme`, `cursor`, `overflow`, `height`, `margin-start`, `margin-end`. Use widget APIs instead (`set_overflow`, size requests, `EventControllerMotion` for hover). Dark tokens are injected from `prefs.rs` because `@media (prefers-color-scheme)` follows the OS, not the app theme. `ListBoxRow :hover` is unreliable; log and notification rows use `.dn-pointed` from a motion controller.

## Local state

| Path | Contents |
| --- | --- |
| `~/.config/sh.devnull.Client/prefs.json` (Linux) | Theme, who-to-follow, highlight-on-hover, chat dock, pins, reactions, last-visit markers |
| `~/Library/Application Support/sh.devnull.Client/prefs.json` (macOS) | Same prefs file, via GTK’s config dir |
| `~/.local/share/sh.devnull.Client/pending-auth` (Linux) | In-flight PKCE + key during sign-in (mode 600, deleted after redeem) |
| `~/Library/Application Support/sh.devnull.Client/pending-auth` (macOS) | Same pending file |
| Secret Service item `app=sh.devnull.Client` (Linux) | Ed25519 device key (raw 32 bytes) |
| Keychain generic password `sh.devnull.Client` / `device-key` (macOS) | Ed25519 device key (base64) |

Delete the prefs file to reset theme, the chat dock, pins, reactions, and visit markers. Revoke the device on the website to invalidate the key.
