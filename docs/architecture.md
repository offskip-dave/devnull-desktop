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

`devnull-api` maps resources under `/api/v1`: logs, starred commits, commits (create, star, replies, quotes, related, reports, pastéis, previews), users, following, suggested, search, notifications (including `GET notifications/{id}/group`), remotes, settings, mutes/blocks, export, and account deletion.

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
| `ui` | Widgets: commits, rails, profiles, remotes inbox/dock, settings, dialogs, aside |
| `body` | Markdown (emphasis, lists, autolinked URLs), `/dev/null` logo, syntax highlighting |
| `images` | Avatars, uploads, galleries, lightbox |
| `emoji` | `:` suggestions in composers |
| `prefs` | Theme, who-to-follow, highlight-on-hover, chat dock, pins, reactions, last-visit markers |

Layout matches the website shell: 256px nav (80px collapsed), 620px feed (940px on Remotes), 320px aside. Below 768px the feed goes fluid. The feed scrollbar is drawn at the window edge so the pointer can scroll from either sidebar.

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

Local prefs: `~/.config/sh.devnull.Client/prefs.json` (theme, who-to-follow, highlight-on-hover, chat dock, pinned remotes and messages, reactions, log visit times). The device key is not stored there.
