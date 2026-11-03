# Contributing

Thanks for helping with the GTK /dev/null client. This guide is for people changing code, docs, icons, or packaging. Product and API ownership live on [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh); this repo is the Relm4 client that consumes `/api/v1`.

## Before you start

1. Read [README](README.md) for install and run.
2. Skim [Architecture](docs/architecture.md) so crate boundaries stay clear.
3. Check [Changelog parity](docs/changelog-parity.md) before porting (capability matrix + day-by-day from Oct 15: done / waiting / skipped).
4. Use [Development](docs/development.md) for day-to-day build, check, icons, and GTK CSS pitfalls.
5. Use [Packaging](docs/packaging.md) when you touch Flatpak, AppImage, macOS packaging scripts, desktop files, or install layout.

Linux is the primary target. macOS (Homebrew GTK) must keep working; it is not a substitute for the first-party Swift app.

## Set up

Requirements match the README: Rust **1.93+**, GTK 4.14+, libadwaita 1.5+, and a Secret Service (Linux) or the login Keychain (macOS).

```bash
git clone https://github.com/dev-null-sh/dev-null-linux.git
cd dev-null-linux
cargo run -p devnull
```

Against a local API:

```bash
DEVNULL_URL=http://127.0.0.1:8000 cargo run -p devnull
```

On macOS prefer `cargo run -p devnull --release` for day-to-day UI work; debug GTK on Quartz is slow.

## Where to change things

| Goal | Start here |
| --- | --- |
| Navigation, messages, loading, composers | `crates/devnull/src/app.rs` |
| Widgets, aside, profiles, remotes UI | `crates/devnull/src/ui.rs` |
| Styles / tokens | `crates/devnull/src/style.css` (+ dark overrides in `prefs.rs`) |
| Avatars, uploads, galleries | `crates/devnull/src/images.rs` |
| Commit body / markdown | `crates/devnull/src/body.rs` |
| New or changed `/api/v1` types and calls | `crates/devnull-api` |
| Sign-in, device key, `devnull://` | `crates/devnull-auth` |
| Symbolic icons | `resources/icons/` (see Development → Icons) |

Keep signed HTTP in `devnull-api`. Do not scrape HTML or embed a WebView for GitHub OAuth.

## Porting from the website

The web changelog ([dev-null.sh/changelog](https://dev-null.sh/changelog)) is the usual source of user-facing work. Keep the ledger in [Changelog parity](docs/changelog-parity.md) current when you catch up a day.

When porting:

- Prefer features that already have a **signed** `/api/v1` route. Probe carefully; do not invent endpoints that 404.
- Skip web-only surfaces (Inertia-only props, moderation tools, traffic charts, terms CMS, headline APIs that are not on `/api/v1` yet). Empty UI that waits for an API is fine; fake data is not.
- Mark each item **Done**, **Partial**, **Waiting**, or **Skip** in Changelog parity, with a one-line why for anything not fully Done.
- Match shell layout: 256px / 80px nav, 620px feed (940px Remotes), 320px aside. Breakpoints and lane-color rules are in Development and Architecture.
- Follow [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md) for visual language. Lane color is for the graph, avatar ring, and handle dot — not a general accent. Primary actions use Commit Blue.
- Avatars with photos should use `GtkPicture` in a clipped face (same path as org avatars). `AdwAvatar::set_custom_image` is unreliable on macOS Quartz.
- Composer lane ring fills solid at **200** characters; the body limit remains **4000**.

## Checks before a pull request

CI runs on Ubuntu (fmt, clippy, tests) and macOS (clippy, tests), always `--locked`:

```bash
cargo fmt --all
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Optional live API decode (ignored in CI):

```bash
cargo test -p devnull-api -- --ignored
```

Please:

- Keep diffs focused; one concern per PR when you can.
- Update docs when behavior or setup steps change.
- Do not commit device keys, prefs with secrets, or `.env` files.
- Do not skip hooks or force-push to `main` / `production`.
- Say how you tested (Linux, macOS, Flatpak, or API against production / local).

## Pull request tip

Link the web changelog commit or issue when you port a feature. Update [Changelog parity](docs/changelog-parity.md) for that day, and note anything left web-only and why (missing API, Linux layout, etc.).

## Questions

Open an issue on this repo for client bugs and GTK packaging. Server / API / product questions belong on [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh).
