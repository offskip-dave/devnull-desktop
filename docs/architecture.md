# Architecture

The GTK client is a Relm4 app over GTK4 and libadwaita. It builds on Linux and macOS. It is a first-party API client, not a wrapper around the website. On a Mac it is this GTK binary, distinct from the first-party Swift app.

For contributor workflow (setup, porting rules, PR checks), see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Crates

```
crates/devnull          Relm4 UI (bin: devnull)
crates/devnull-api      Signed /api/v1 HTTP client
crates/devnull-auth     Device key, PKCE, Secret Service / Keychain, URL handler
```

`devnull` depends on the other two. `devnull-auth` depends on `devnull-api` for `DeviceKey` and the base URL.

Application id: `sh.devnull.Client` (GTK / desktop / keyring). Authorize `app_id` is `linux:sh.devnull.Client` or `macos:sh.devnull.Client`. User agent: `DevNull-Linux/0.1` on Linux, `DevNull-macOS/0.1` on macOS. Redirect URI is `devnull://linux/auth` on both so the server still recognizes this GTK client.

## Signed API

Every authenticated request is signed the same way as the Mac app and the server’s `SignedRequests` service.

1. The device holds an Ed25519 key (OpenSSH `ssh-ed25519` wire format).
2. The payload is hashed and wrapped as an SSHSIG for namespace `devnull-api` (JSON) or `devnull-api-images` (multipart).
3. Headers carry the public key, fingerprint (`SHA256:…`), nonce, and signature.

Unsigned calls exist for public profiles and commits. Signed traffic is capped at 110 requests per minute (55 unsigned) so the client stays under the API’s 120/min limit.

Point the client at another origin with `DEVNULL_URL`. Default is `https://dev-null.sh`.

`devnull-api` maps resources under `/api/v1`: logs, starred commits, commits (create, star, list who starred your own commit, replies, quotes, related, reports, pastéis, previews, poll votes), users, following, followers, followed organizations, live start/stop, organizations (show, commits, mentions, follow, claim), suggested, search, mention suggestions, notifications (including `GET notifications/{id}/group`), remotes, settings, mutes/blocks, export, and account deletion.

When the website gains a feature that has no `/api/v1` route yet (for example Inertia-only hot topics), the GTK client may reserve UI that stays empty until the API ships. Do not call invented paths that only 404, and do not scrape HTML as a substitute.

## Sign-in

There is no embedded browser for GitHub OAuth.

1. `devnull-auth` creates or loads the device key from Secret Service (`oo7`) on Linux, or the login Keychain (`security-framework`) on macOS.
2. It opens `/mobile/authorize` in the system browser with PKCE (`S256`), `redirect_uri=devnull://linux/auth`, and `app_id` `linux:sh.devnull.Client` or `macos:sh.devnull.Client` (the server requires `platform:bundle`).
3. After you approve the device, the browser should hand the callback to this app.
4. The client POSTs the code and verifier to `/api/v1/devices` and stores the session as the signed key.

If the browser swallows `devnull://`, paste the callback URL on the sign-in screen. Revoke a device on the website under Settings → Devices.

On Linux, launch registers a user desktop file (`Exec=… %u`) for `x-scheme-handler/devnull`. Flatpak ships the handler in the app metadata instead. GApplication uniqueness on the session bus is the usual return path: the waiting window gets the callback as a command-line URI.

Both platforms also write the in-flight PKCE verifier (and a copy of the device key) under the app data dir, and any process that sees `devnull://` writes `callback.uri` for the waiting window to poll. That covers a missing session bus (typical on macOS, possible on Linux). On macOS, launch compiles a helper `.app` under Application Support, registers it, and claims `devnull://` so the first-party Mac app does not swallow `devnull://linux/auth`. The helper only writes the callback file; the waiting window polls it. Paste still works if the browser keeps the URL.

## UI

`crates/devnull/src/app.rs` is the Relm4 `SimpleComponent`. Supporting modules:

| Module | Role |
| --- | --- |
| `ui` | Widgets: commits, rails, profiles, remotes inbox/dock, settings, dialogs, aside |
| `body` | Markdown (emphasis, lists, autolinked URLs), `/dev/null` logo, syntax highlighting |
| `images` | Avatars, uploads, galleries, lightbox |
| `emoji` | `:` suggestions in composers |
| `prefs` | Theme, who-to-follow, highlight-on-hover, chat dock, pins, reactions, last-visit markers |

On macOS a `gtk::HeaderBar` (not `AdwToolbarView`) supplies close/minimize/zoom so Quartz can drag and go fullscreen without overflowing the 1280px shell. The dock icon is set from `sh.devnull.Client.svg`. Cmd+Q is `app.quit`.

Layout matches the website shell: 256px nav (80px collapsed), 620px feed (940px on Remotes), 320px aside. Below 768px the feed goes fluid. The feed scrollbar is drawn at the window edge so the pointer can scroll from either sidebar. A profile's sticky bar starts as a name and count, then brings in the avatar and `@handle` once the hero avatar scrolls away. Folded commits that start with a long code block fade the last lines instead of trailing `…`.

Breakpoints: the aside hides below 1280px — on Remotes, the org inbox collapses first when a thread is open — the nav rail collapses below 1024px, and the shell goes compact below 768px.

Lane color is used on the graph rail, avatar ring, and the handle dot — not as a general accent. Quoted remotes authors use it on their name, matching the website’s ReplyGhost. Notification rails, rings, and icons use the actor’s lane (the website keeps those rails gray). Action buttons use Commit Blue (`#3b6fe0` / `#7aa2f7` in dark). Tokens live in `crates/devnull/src/style.css` and the dark overrides in `prefs.rs`, following [DESIGN.md](https://github.com/dev-null-sh/dev-null.sh/blob/production/DESIGN.md).

Who to follow in the aside comes from `GET suggested` (also `suggested-users` / `me/suggested`), minus people already followed (`GET users/{me}/following`, then `me/following`, then authors on the Following log). When that payload is missing, unique authors from Trending then Latest are used, still excluding people already followed.

## Notifications

`GET notifications?type=&cursor=` and `GET notifications/{id}/group`. Filter pills choose which kinds to include. Rows highlight on hover like the log.

Follows and stars color the rail, ring, and icon with the newest actor’s lane (not the commit author’s). Mentions and replies put that author’s avatar on the rail. Grouped stars stack up to three avatars plus a `+N` disc and expand in place. A referenced commit with images shows a 40px thumbnail under the timestamp.

## Remotes

Signed routes under `/api/v1`: inbox (`GET remotes?tab=&q=`), candidates (`GET remotes/candidates`), create org (`POST remotes/orgs`), thread messages and push, mark read, archive, leave org, preferences (`PUT me/remote-preferences`), and enable (`PUT me/remotes`).

The Remotes page is a 320px inbox plus thread and composer (feed max 940px). **Open** and **Stashed** tabs split the list. Direct rows show the partner’s avatar, name, and `@handle`. Org rows stack member avatars and show the org name (or the other members’ first names) with a members count; the open-thread header hides that count. Search filters the inbox and can start a remote with a matching developer.

Threads use 75% bubbles, 12-hour timestamps, and hover pin / reply / react. Quoted authors use lane color; membership lines read `@username added @username`. Opening a thread jumps to the latest message. Voice notes and ping live next to the composer. Pull receipts sit under the latest message.

**Dock** pins a 360px overlay to the bottom-right of every page (`prefs.chat_docked`). Compact windows never dock. **New remote** and **New org** open a people picker (candidates, then search). Remote settings (who can push, share pulls, turn remotes off) live in the inbox header. Pin, stash, mute, and leave org sit on the thread menu.

Local prefs: `~/.config/sh.devnull.Client/prefs.json` on Linux, `~/Library/Application Support/sh.devnull.Client/prefs.json` on macOS (theme, who-to-follow, highlight-on-hover, chat dock, pinned remotes and messages, reactions, log visit times). The device key is not stored there.
