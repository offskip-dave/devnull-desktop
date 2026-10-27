# Architecture

The Linux client is a Relm4 app over GTK4 and libadwaita. It is a first-party API client, not a wrapper around the website.

## Crates

```
crates/devnull          Relm4 UI (bin: devnull)
crates/devnull-api      Signed /api/v1 HTTP client
crates/devnull-auth     Device key, PKCE, Secret Service, URL handler
```

`devnull` depends on the other two. `devnull-auth` depends on `devnull-api` for `DeviceKey` and the base URL.

Application id: `sh.devnull.Client`. User agent: `DevNull-Linux/0.1`.

## Signed API

Every authenticated request is signed the same way as the Mac app and the server’s `SignedRequests` service.

1. The device holds an Ed25519 key (OpenSSH `ssh-ed25519` wire format).
2. The payload is hashed and wrapped as an SSHSIG for namespace `devnull-api` (JSON) or `devnull-api-images` (multipart).
3. Headers carry the public key, fingerprint (`SHA256:…`), nonce, and signature.

Unsigned calls exist for public profiles and commits. Signed traffic is capped at 110 requests per minute (55 unsigned) so the client stays under the API’s 120/min limit.

Point the client at another origin with `DEVNULL_URL`. Default is `https://dev-null.sh`.

`devnull-api` maps resources under `/api/v1`: logs, starred commits, commits (create, star, replies, quotes, reports, pastéis, previews), users, search, notifications, remotes, settings, mutes/blocks, export, and account deletion.

## Sign-in

There is no embedded browser for GitHub OAuth.

1. `devnull-auth` creates or loads the device key from Secret Service (`oo7`).
2. It opens `/mobile/authorize` in the system browser with PKCE (`S256`) and `redirect_uri=devnull://linux/auth`.
3. After you approve the device, the browser should hand the callback to this app.
4. The client POSTs the code and verifier to `/api/v1/devices` and stores the session as the signed key.

If the browser swallows `devnull://`, paste the callback URL on the sign-in screen. Revoke a device on the website under Settings → Devices.

On a normal install, launch registers a user desktop file for `x-scheme-handler/devnull`. Flatpak ships the handler in the app metadata instead.

## UI

`crates/devnull/src/app.rs` is the Relm4 `SimpleComponent`. Supporting modules:

| Module | Role |
| --- | --- |
| `ui` | Widgets: commits, rails, profiles, settings, dialogs, aside |
| `body` | Markup, `/dev/null` logo, syntax highlighting |
| `images` | Avatars, uploads, galleries |
| `emoji` | `:` suggestions in composers |
| `prefs` | Theme, who-to-follow, last-visit markers |

Layout matches the website shell: 256px nav (80px collapsed), 620px feed (940px on Remotes), 320px aside. The feed scrollbar is drawn at the window edge so the pointer can scroll from either sidebar.

Lane color is used on the graph rail, avatar ring, and the handle dot — not as a general accent. Action buttons use Commit Blue (`#3b6fe0` / `#7aa2f7` in dark). Tokens live in `crates/devnull/src/style.css` and the dark overrides in `prefs.rs`, following [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md).

Local prefs: `~/.config/sh.devnull.Client/prefs.json` (theme, who-to-follow, log visit times). The device key is not stored there.
