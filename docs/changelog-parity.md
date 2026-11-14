# Changelog parity

Track of [dev-null.sh/changelog](https://dev-null.sh/changelog) against this GTK client, from the platform’s first production commit on **2026-10-15** (`chore: first` — empty repo in the morning, live by lunch; see [About](https://dev-null.sh/about)) through **2026-11-13**. Prefer signed `/api/v1` surfaces. Skip Inertia-only, CLI, PWA, admin/CMS, and first-party Swift/mobile app work unless a client route exists.

The [capability matrix](#capability-matrix-launch--present) is the durable inventory. [Day by day](#day-by-day-oct-15--present) lists every non-merge `feat` on `production` since day one. Update both when you catch up a changelog day. Link the web PR or commit in your PR description.

## Status key

| Status | Meaning |
| --- | --- |
| **Done** | Present in this client |
| **Partial** | Some of the web behavior; gaps noted |
| **Waiting** | Wanted; blocked on `/api/v1`, recording/WebRTC, or a larger port |
| **Skip** | Out of scope (web/CLI/admin/CMS/other apps) |

## Capability matrix (launch → present)

Durable inventory by product area. Every production `feat` from day one is also listed under [Day by day](#day-by-day-oct-15--present).

### Auth, devices, shell

| Capability | Status | Notes |
| --- | --- | --- |
| GitHub sign-in via system browser + PKCE / `devnull://` | Done | No embedded WebView |
| Device list / revoke in Settings | Done | Revoke also possible on the web |
| Terms acceptance at device redeem | Done | `terms_accepted: true` on `POST devices` (#1385) |
| First-run onboarding / guest join sheet | Skip | Web / mobile marketing |
| PWA, splash, favicon, SEO, SSR public pages | Skip | Website |
| Opt-in local auto-login cookie | Skip | Browser-only |
| Official CLI / GitHub Actions commit / TUI / Mac Swift / mobile betas | Skip | Other clients; this repo is the GTK client |

### Logs and commits

| Capability | Status | Notes |
| --- | --- | --- |
| Following / Latest / Trending / Starred logs | Done | |
| Mark where you left off in the log | Done | Visit markers in prefs |
| Dim other commits on hover | Done | Prefs toggle |
| Collapse nav / responsive shell | Done | 256→80 nav, aside hide, compact |
| Composer: text, images, polls, schedule, preview | Done | Lane ring fills at 200 chars |
| Hashtags in commit text | Done | `#tag` opens search; hex colors stay swatches |
| Emoji shortcodes + picker; `@` mention suggestions | Done | |
| Replies, quotes, related commits, ancestors | Done | |
| Stars (incl. stargazers of your own commit) | Done | API #1310; stargazer hover cards (#1204) |
| Pastéis mode | Done | |
| Report / mute / block / delete own commit | Done | Delete via more menu |
| Commit more menu (follow / mute / block / pin / send) | Done | #1371 Send in Remotes via share picker |
| Translate commits; translation language / automatic | Done | Settings |
| Code blocks, titles/filenames, fold fade | Done | `body.rs` |
| Link previews + in-place embeds (YouTube, etc.) | Done | Opens / plays via embed widgets |
| Author hover cards | Done | Follow, mute, block, Remote |
| Polls vote / results | Done | |
| Soft-deleted commits keep replies | Done | Tombstones from API |
| Series of commits | Partial | Thread shows `series` when the API sends it; no series composer (web dropped multi-part stash) |
| Pin a commit to your profile | Done | More menu → pin / unpin via `/api/v1` |
| Save / bookmark commits (pre-Starred merge) | Done | Consolidated into **Starred** on the web and here |
| Undo delete for 30 seconds | Done | Toast Undo for 30s after delete |
| Sensitive-text warn before publish | Skip | Removed on the web (#999); not a GTK gap |
| Draft stash across reloads | Partial | Session composer state only; not the web’s persisted drafts |
| Choose link card in composer | Waiting | Remotes Done; signed create-commit API has no `link_preview_url` yet |
| First commit on a new account | Skip (by design) | Still created on the web |

### Profiles and connections

| Capability | Status | Notes |
| --- | --- | --- |
| Profile page: bio, lane, graph, streaks, achievements / XP | Done | Photo opens in the image viewer |
| Seasonal event themes | Done | Auto / Default / Halloween / Christmas / New Year in Appearance |
| Commits / Replies / README tabs | Done | README via API then GitHub |
| Follow / Follow back / Follows you chips | Done | Following lists too (#1434) |
| Following / Followers lists (+ orgs on Following) | Done | |
| Mutuals / Followers you know tabs | Waiting | Web-only connection types so far |
| Share profile link or ID | Done | |
| Go live / End stream | Done | |
| Sync / edit via GitHub | Done | Sync in-app; edit opens GitHub |
| GitHub Sponsors link when listed | Done | |
| Hover-card Remote @handle | Done | #1308 |
| Activity graph embed for READMEs | Skip | External embed for GitHub READMEs |
| Unclaimed GitHub profiles (#1406) | Waiting | Inertia lookup; no stable `/api/v1` show yet |
| Wait to follow not-yet-members (#1407) | Waiting | Web pending-follow routes |
| Mentions of anyone → GitHub lookup page (#1409) | Waiting | Depends on #1406 |

### Organizations

| Capability | Status | Notes |
| --- | --- | --- |
| Org page: follow, claim, lane color, share | Done | |
| Org commits tab (API) | Done | #1386 |
| Org mentions tab | Done | |
| Post / schedule as organization | Done | Composer “post as”; feature flag removed on the web (6e71709) |
| Org remotes (groups), pin messages, leave | Done | |
| Org admin moderation (timeout / ban posters) | Skip | Moderator tools, Inertia |
| Org calls | Waiting | Same as remotes calls (WebRTC) |

### Remotes

| Capability | Status | Notes |
| --- | --- | --- |
| Inbox Open / Stashed / Fetched; search; dock | Done | |
| Direct + org threads; push text/images; reply; react | Done | Link-card picker on push when the body has URLs |
| Pin / stash / mute / block / pull receipts | Done | |
| Account-synced remote pins | Done | Via API prefs |
| Back to present (≥25 msgs below) | Done | #1250 |
| New remote / new org pickers | Done | |
| Org Mentions bell (#1493) | Partial | Composer bell after Call; lists thread `mentions`. Jump when loaded. Clear / auto-seen Waiting on signed DELETE |
| Voice notes (record / play / speed) | Waiting | UI stub; toast — no Opus pipeline |
| One-to-one / org calls | Waiting | Button opens web thread URL; no WebRTC in GTK |
| Send a commit link into Remotes | Done | More menu → share picker → push permalink |

### Notifications

| Capability | Status | Notes |
| --- | --- | --- |
| Inbox with filters, groups, expand | Done | |
| Grouped stars, mention avatars, lane-colored actors | Done | |
| Follow back from notifications | Done | Via follow actions |

### Search and aside

| Capability | Status | Notes |
| --- | --- | --- |
| People / commit search (`Ctrl+K`, `/`) | Done | Idle omnibox lists hot topics; results have Most relevant and Latest |
| Who to follow (+ refresh, hide, not interested) | Done | |
| Live following | Done | |
| Hot topics + optional summary | Done | Aside, omnibox, and in-app headline pages via `GET /api/v1/headlines` |
| Legal / about / apps / changelog links | Done | Open in browser |
| Remember last three searches | Done | Prefs + omnibox recent chips / clear |

### Settings and safety

| Capability | Status | Notes |
| --- | --- | --- |
| Profile / Appearance / Terminal / Safety / Devices | Done | |
| Timezone, remotes prefs, pastéis, translation | Done | |
| Event theme (Auto, Default, Halloween, Christmas, New Year) | Done | Calendar windows match the website; Christmas graph is green with a red peak |
| Mutes / blocks lists; unmute / unblock | Done | |
| Export data; delete account | Done | |
| Keyboard shortcuts dialog | Done | |
| Admin moderation, timeouts, ops / growth / traffic charts | Skip | Admin Inertia |
| Terms / privacy CMS | Skip | Opened in browser |

### Headlines

| Capability | Status | Notes |
| --- | --- | --- |
| Hot-topic aside list | Done | `GET /api/v1/headlines` |
| Headline discussion pages / recap pipeline | Done | `GET /api/v1/headlines/{id}`; each commit ends on its own dot |

## Day by day (Oct 15 → present)

Every non-merge `feat` on the website `production` branch, oldest day first. Statuses match the matrix; **Skip** rows are kept so the day isn’t incomplete when you cross-check the changelog.

### 2026-10-15

46 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| pin a commit to your profile (#90) | Done | More menu pin / unpin |
| stash unsent commit and reply drafts | Partial | Session composer only; not persisted like the web |
| followers page (#55) | Done |  |
| notifications show improvement | Done |  |
| scale the activity graph to the user's history (#68) | Done |  |
| link previews for profile links (#57) | Done |  |
| add commits and replies tabs to profiles (#76) | Done |  |
| notifications | Done |  |
| improve commit star placement, animation, and row layout | Done |  |
| commit pages with ancestors and replies | Done |  |
| commit graph frontend | Done |  |
| soft delete commits and keep their replies (#70) | Done |  |
| inline code and highlighted code blocks in commits | Done |  |
| allowing multiple approve commits & refactoring user card to user preview | Partial | Hover cards Done; batch approve is admin |
| base timeline, creating of posts, and users | Done |  |
| threaded replies to commits | Done |  |
| commit activity graph on profiles | Done |  |
| choose your lane color (#65) | Done |  |
| export your personal data from settings (#74) | Done |  |
| mark where you left off in the log (#66) | Done |  |
| merge description into body | Done |  |
| let people star commits | Done |  |
| people search with cmd+k | Done |  |
| link sidebar stats to commits and connections | Done |  |
| link previews for commit links (#69) | Done |  |
| show longest and current commit streaks on profiles (#61) | Done |  |
| emoji shortcodes in commits and replies (#67) | Done |  |
| user hover cards with follow | Done |  |
| mentions & image uploads | Done |  |
| show the star next to reply and copy link | Done |  |
| save commits privately (#81) | Done |  |
| mentions & image uploads | Done |  |
| suggest who to follow (#44) | Done |  |
| keyboard moderation with a and x | Skip | |
| new logo, a solid slash and an outlined orange slash | Skip | |
| polish the log, composer, profile, and landing | Done | Core log / composer / profile shipped here |
| add notification indicator for unmoderated commits | Skip | |
| add opt-in local auto-login | Skip | |
| allow to approve multiple commits | Skip | |
| notify authors about moderation updates (#77) | Skip | |
| reply to a commit from the terminal with --reply <hash or link> | Skip | |
| octane | Skip | |
| web app manifest and legible favicon fallbacks (#63) | Skip | |
| custom error pages in git's voice | Skip | |
| commit from the terminal, signed with your GitHub SSH key | Skip | |
| moderation | Skip | |

### 2026-10-16

55 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| undo commit deletion for 30 seconds (#94) | Done | Toast Undo for 30s |
| warn before publishing sensitive text (#146) | Skip | Removed on the web (#999) |
| let authors choose the link card (#166) | Waiting | Remotes picker Done; create-commit API omits `link_preview_url` |
| keep attached images in composer drafts (#161) | Partial | Session composer only; not persisted like the web |
| post a series of commits (#112) | Partial | Thread renders series when present; composer multi-part not ported |
| link to GitHub Sponsors from profiles (#129) | Done |  |
| refresh who to follow | Done |  |
| X-style photo viewer for commit images (#144) | Done |  |
| autocomplete emoji shortcodes | Done |  |
| long commits | Done |  |
| show a "follows you" badge on profiles (#149) | Done |  |
| refresh notifications design | Done |  |
| shorten long urls in commits like x | Done |  |
| rank the Everyone log by stars and replies | Done |  |
| color notification icons with the actor's lane | Done |  |
| follow new followers back from the notifications list | Done |  |
| polls on commits (#114) | Done |  |
| notify authors about stars, grouped per commit (#107) | Done |  |
| mute and block people (#111) | Done |  |
| group and filter the notification inbox (#147) | Done |  |
| poll visible commit stars (#143) | Done |  |
| copy highlighted code blocks (#137) | Done |  |
| play YouTube, Suno, Spotify and other links in place (#243) | Done |  |
| developer achievements and unix ranks (#113) | Done |  |
| show profile previews for mentions | Done |  |
| improve log system | Done |  |
| live preview in the commit and reply composers (#164) | Done |  |
| sync profile from github | Done |  |
| Enhance personal data export functionality with additional user data and throttling (#120) | Done |  |
| draw lane-coloured thread lines between replies (#121) | Done |  |
| quote commits (#96) | Done |  |
| schedule commits for later submission (#85) | Done |  |
| count stars on replies and favor recent commits in trending | Done |  |
| tuck code block copy into the corner (#190) | Done |  |
| put author avatars on the timeline rail (#105) | Done |  |
| pgsql + 3 new timelines | Skip | |
| remove landing page | Skip | |
| embeddable activity graph for READMEs (#95) | Skip | |
| SEO | Skip | |
| moderations | Skip | |
| commit from github actions (#165) | Skip | |
| public read API and Atom feeds (#98) | Skip | |
| add auto-approval (#284) | Skip | |
| first-run onboarding | Skip | |
| rate limit social actions, people lookups, and exports (#162) | Skip | |
| report published commits for moderation (#93) | Skip | |
| operations dashboard for admins (#169) | Skip | |
| spring the user preview in and out | Skip | |
| remove youtube livestream banner | Skip | |
| finish ssr (public pages) | Skip | |
| smooth transitions between pages, tabs and settings (#148) | Skip | |
| auto-follows | Skip | |
| reject posts that are just bad | Skip | |
| auto-follows (missing tests) | Skip | |
| have @world answer hello world commits (#191) | Skip | |

### 2026-10-17

10 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| schedule the chosen link card | Waiting | Same create-commit API gap as #166 |
| schedule commits with images and polls | Done |  |
| label quotes listed under the commit they quote | Done |  |
| timezone setting and zoned timestamp tooltips (#155) | Done |  |
| allow committing images without a message | Done |  |
| drop images anywhere on a composer | Done |  |
| suggest emoji in a menu while typing a shortcode (#305) | Done |  |
| show dev/null commit links as the author and short hash (#349) | Done |  |
| allow scheduling images without a message | Done |  |
| terms and privacy | Skip | |

### 2026-10-18

34 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| mentions on orgs just displays users from orgs | Done |  |
| schedule commits with a calendar picker from a composer toolbar icon (#382) | Done |  |
| open chat images in the image viewer (#405) | Done |  |
| add remotes, direct and group messages that feel instant without websockets (#338) | Done |  |
| add share button to user profiles (#255) | Done |  |
| notifications per day | Done |  |
| short numbers where needed (#410) | Done |  |
| react to remote messages with emoji (#452) | Done |  |
| dim other commits on mouse hover (#310) | Done |  |
| read a thread an author writes as replies to themselves in order (#449) | Done |  |
| reply in remotes, with links, mentions, code, link cards, images and kept drafts (#381) | Done |  |
| turn emoji shortcodes into emoji in remotes (#377) | Done |  |
| go live from settings, with the stream on the profile and a live marker on the avatar | Done |  |
| improve notification filtering (#409) | Done |  |
| add a changelog of pull requests merged into production (#379) | Skip | |
| more analytics improvements | Skip | |
| add image attachments to CLI commits (#339) | Skip | |
| floating commit button on mobile (#435) | Skip | |
| improve automod to take recent reports into account (#412) | Skip | |
| small mobile improvements (#387) | Skip | |
| wallpapers, pwa improvements, pms emodjis imrpovements | Skip | |
| auto-unfocus remote chat on mobile after sending a message (#439) | Skip | |
| mark the source code link as leaving /dev/null (#375) | Skip | |
| screen new GitHub accounts at sign up (#369) | Skip | |
| brand | Skip | |
| show the mobile commit button only once the composer scrolls away (#444) | Skip | |
| improve automod for trusted users (#385) | Skip | |
| analytics | Skip | |
| swipe between log lanes on touch screens (#404) | Skip | |
| load older days in the changelog and color authors with their own lanes (#380) | Skip | |
| analytics | Skip | |
| login locally, about, animation if commit being reviewed | Skip | |
| add source code link to sidebar (#257) | Skip | |
| links on about | Skip | |

### 2026-10-19

49 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| choose how many replies to show at a time (#599) | Partial | Pagination exists; no exact web control |
| show a check mark on the commit badge once it is approved (#548) | Partial | Moderation badge polish |
| fade solid thread rails toward the next author's lane (#707) | Done |  |
| one char commits | Done |  |
| clicks on commits open commit | Done |  |
| let orgs hold up to 100 people (#647) | Done |  |
| fade reply lane connectors toward the next author's colour (#691) | Done |  |
| show GitHub repository links as owner/repo | Done |  |
| report commits from the API (#552) | Done |  |
| collapse the navigation (#549) | Done |  |
| quote commits from the API (#553) | Done |  |
| allows users to give live | Done |  |
| switch pastéis mode and give pastéis from the API (#557) | Done |  |
| delete the signed-in account from the API (#551) | Done |  |
| list a profile's followers and following in the API (#556) | Done |  |
| adds profile preview to who to follow (#527) | Done |  |
| pick the stream length with a slider when going live (#689) | Done |  |
| offer follow back in who to follow (#416) | Done |  |
| suggest emoji in poll options (#432) | Done |  |
| make notification types easier to tell apart (#489) | Done |  |
| show GitHub issue and pull request links as owner/repo#number | Done |  |
| sign in to the API from a mobile app with a device key (#130) | Done |  |
| make lane dimming opt-in (#596) | Done |  |
| allow to disable translations from panel | Done |  |
| mute and block people from the API (#555) | Done |  |
| star and save commits from the API (#550) | Done |  |
| show a one-line bio in who to follow (#398) | Done |  |
| shared commit translations (#577) | Done |  |
| sync from GitHub and save the lane color from the API (#554) | Done |  |
| feat(operations): count everyone and everything since the first signup | Skip | |
| feat(operations): format every count the way the rest of the app does | Skip | |
| drop confetti when an achievement unlocks (#287) | Skip | |
| product metrics tab with the AARRR funnel | Skip | |
| feat(operations): preview the busiest authors' profiles on hover | Skip | |
| attach browser and screen details to issues opened with /feature and /bug (#544) | Skip | |
| adds poteto event | Skip | |
| feat(operations): break the window's commits down by status and author | Skip | |
| feat(operations): drop the streak strip under the activity graph | Skip | |
| feat(operations): draw everyone's commits on an activity graph | Skip | |
| sync from GitHub during onboarding after editing the profile (#682) | Skip | |
| feat(operations): split the page into Health, Activity and Traffic tabs | Skip | |
| tom's a real boy now :) (#524) | Skip | |
| add sweet tooth and luddite achievement badges (#621) | Skip | |
| add repository insights and community statistics for Tom (#702) | Skip | |
| add markdown templates for issue commands (#719) | Skip | |
| feat(operations): replace the volume table with today's numbers | Skip | |
| double trending scores every three hours instead of six | Skip | |
| feat(operations): switch the activity graph between commits, replies and signups | Skip | |
| feat(operations): give the feature switches their own tab | Skip | |

### 2026-10-20

28 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| remember the three most recent searches (#841) | Done | Omnibox recent + prefs |
| consolidate saved and bookmarks into one (#811) | Done |  |
| show live users you follow in the sidebar (#775) | Done |  |
| preview and open profiles from notification actors (#570) | Done |  |
| link the composer avatar to the user's profile (#675) | Done |  |
| let users choose a translation language (#753) | Done |  |
| fill the avatar ring as a post grows toward its limit (#659) | Done |  |
| reshuffle the trending log on each visit (#727) | Done |  |
| prioritize mention suggestions by conversation (#750) | Done |  |
| show tagged GitHub accounts in bios as avatar pills (#720) | Done |  |
| render poll options like a commit body (#837) | Done |  |
| link upstream issue and pull request references (#801) | Done |  |
| show authors who starred their commit from its star count (#365) | Done |  |
| open changelog as a full page on desktop (#770) | Skip | |
| notify users when they reach a new level (#486) | Skip | |
| only allow to suggest 10 issues a day | Skip | |
| let Tom search upstream repository history (#778) | Skip | |
| banner | Skip | |
| optimistic commit deletion (#126) | Skip | |
| route Tom tools through the Decisions API (#878) | Skip | |
| allows admins to timeout and bans users (#898) | Skip | |
| chart the activation rate of each signup day (#779) | Skip | |
| allows to flush dailed jobs | Skip | |
| measure retention from day 1 while the network is young (#812) | Skip | |
| updates about | Skip | |
| ignore recent reports on moderation | Skip | |
| improve Tom history search and local mentions (#788) | Skip | |
| actually delete commits | Skip | |

### 2026-10-21

9 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| audio in remotes | Waiting | No Opus pipeline |
| add shell shocked achievement (#926) | Done |  |
| allow users to go live on Twitch (#902) | Done |  |
| dismiss people from who to follow (#880) | Done |  |
| improve notification center styling (#911) | Done |  |
| show a dot on the favicon while something is unread | Skip | |
| add CVE maintenance page (#916) | Skip | |
| serves static page while DNS changes | Skip | |
| we back baby | Skip | |

### 2026-10-22

22 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| set the volume of voice notes (#946) | Waiting | No Opus pipeline |
| play voice notes at 1.5x and 2x (#965) | Waiting | No Opus pipeline |
| say you are typing in Remotes from the API (#658) | Partial | No typing indicators in GTK yet |
| sync from GitHub on your own profile (#966) | Done |  |
| vote on polls from the API (#663) | Done |  |
| stash conversations and merge or drop requests in Remotes from the API (#657) | Done |  |
| attach images to commits from the API (#660) | Done |  |
| push messages in Remotes from the API (#665) | Done |  |
| add account-level remote mute preferences | Done |  |
| turn Remotes on and off and change their settings from the API (#649) | Done |  |
| preview a commit from the API (#664) | Done |  |
| start polls from the API (#662) | Done |  |
| short formats for big numbers, thousands separators for smaller ones (#933) | Done |  |
| start remotes and orgs from the api (#734) | Done |  |
| list and open conversations in Remotes from the API (#654) | Done |  |
| show local time on profiles, opt-in (#803) | Done |  |
| react to messages in Remotes from the API (#656) | Done |  |
| add browser notifications and mute controls for remotes | Skip | |
| add Product of the Day and new joins to the about page | Skip | |
| chart acquisition by day on the metrics page (#963) | Skip | |
| make authors have a hovercard in the changelog (#860) | Skip | |
| draw the graph in a single canvas (#783) | Skip | |

### 2026-10-23

10 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| add per-user automatic translation preference (#1073) | Done |  |
| show back to present when scrolling through remotes (#1086) | Done |  |
| show hex colors as swatches (#1064) | Done |  |
| load new commits on the Latest log by themselves (#1060) | Done |  |
| widen activation to 2 starred authors or 10 follows (#1032) | Skip | |
| add eMudhra certificate hover easter egg | Skip | |
| list the accounts acting in bulk in moderation (#1034) | Skip | |
| split daily active users into returning and new, by activation (#1018) | Skip | |
| shadow each day onto the next on the daily active users chart (#1025) | Skip | |
| replace remote notification sound | Skip | |

### 2026-10-24

14 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| choose the link card when quoting (#1094) | Waiting | Same API gap as #166 for create-commit |
| new commit author avatars (#796) | Done |  |
| add account-synced remote pins (#1088) | Done |  |
| hang every reply off its parent's column (#1019) | Done |  |
| make authors have a hovercard in remotes (#1020) | Done |  |
| preview draft images in lightbox before posting (#945) | Done |  |
| suggest developers from profile and discussion context (#1021) | Done |  |
| add a preview to the quote composer (#1029) | Done |  |
| show loaded remotes instantly while searching (#1093) | Done |  |
| build the contributors list on /about from GitHub (#488) | Skip | |
| add super-saiyan achievement | Skip | |
| improves the algorithm | Skip | |
| rebuild the operations console (#1109) | Skip | |
| email users | Skip | |

### 2026-10-25

7 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| add an experimental page gathering every commit about a link (#1134) | Waiting |  |
| link the commit toast to the commit just posted (#1152) | Done |  |
| mac app | Skip | |
| new mac os build | Skip | |
| simplify | Skip | |
| new app image | Skip | |
| preview url | Skip | |

### 2026-10-26

20 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| add a mutuals tab next to followers and following (#1006) | Waiting | Connections tabs |
| show followers you know on profiles (#733) | Waiting | Connections tabs |
| pin messages in orgs (#1101) | Done |  |
| suggest who to follow from the API (#990) | Done |  |
| show more GitHub links as GitHub writes them (#1016) | Done |  |
| show the GitHub profile README on profiles (#886) | Done |  |
| record terms acceptance from the app, and meet Apple's rules for user-generated content… | Done |  |
| show copy link and report only on the hovered commit or reply (#940) | Done |  |
| commit a scheduled commit now (#1136) | Done |  |
| show the local time in profile previews (#1014) | Done |  |
| show GitLab and Codeberg links the way GitHub ones are shown (#951) | Done |  |
| pings | Done |  |
| show GitHub-imported followers in the onboarding (#1173) | Skip | |
| add opt-in Remotes web push notifications (#1138) | Skip | |
| write commits, open issues and go live from search (#738) | Skip | |
| invite pull requests to the Mac app on the apps page (#1155) | Skip | |
| link about page contributors to their /dev/null profile (#870) | Skip | |
| show the conversion rate by device in growth metrics (#1170) | Skip | |
| explain /dev/null to signed-out phone visitors in a join sheet (#1192) | Skip | |
| rebuilds app | Skip | |

### 2026-10-27

3 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| list related commits under every thread (#1069) | Done |  |
| count taps to sign in for each type of device (#1217) | Skip | |
| adds octane | Skip | |

### 2026-10-28

14 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| add star and go-to keyboard shortcuts (#1247) | Done |  |
| @organization mentions (#1262) | Done |  |
| label code blocks with their filename (#1210) | Done |  |
| link bio mentions to their dev/null profile (#1266) | Done |  |
| mentions tab on organization pages (#1272) | Done |  |
| tell guests how many developers are on /dev/null (#1264) | Skip | |
| mac os 0.3.1 | Skip | |
| completes api | Skip | |
| og on orgs | Skip | |
| adds apps link | Skip | |
| removes commits from github actions | Skip | |
| offer to open the page in the phone's browser from an app's browser (#1218) | Skip | |
| orgs on the native mac os app | Skip | |
| mac os app 0.2 | Skip | |

### 2026-10-29

12 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| feat(one-to-one calls): missing sounds | Waiting | Button opens web thread |
| direct one-to-one calls | Waiting | Button opens web thread |
| add calls on orgs | Waiting | Button opens web thread |
| let organization members claim their page (#1273) | Done |  |
| let organization admins choose its color (#1303) | Done |  |
| tell followers about someone's first commit (#1189) | Done |  |
| tell API clients which users are live (#1274) | Done |  |
| split the sitemaps by the day commits were written and the month profiles signed up (#1… | Skip | |
| let only the first github claim make an organization admin (#1302) | Skip | |
| list organization pages in the sitemap (#1289) | Skip | |
| date every sitemap in the index by the last change of what it lists (#1285) | Skip | |
| let guests on a phone get a link by email to join from their computer (#1263) | Skip | |

### 2026-10-30

10 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| share commits in remotes (#1164) | Done | More menu → share picker |
| add participant selection to commit replies (#1238) | Done | Replying-to picker + `reply_recipients` |
| attach images to commit reports (#1112) | Done | Report dialog attaches up to 3 images |
| keep who follows whom and commit search for members (#1328) | Done |  |
| let users follow organizations (#1305) | Done |  |
| include the days a person was seen in their data export (#1330) | Done |  |
| record each day a person is seen, for retention by signup cohort (#1324) | Skip | |
| claims orgs based on public information | Skip | |
| allows search for users on organizations team managers | Skip | |
| let organization admins manage members in the organization's settings (#1304) | Skip | |

### 2026-10-31

8 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| calls, going live, and organizations api parity | Partial | Live Done; calls open web URL |
| let apps say their name when they ask to be approved, and show it with the device (#1339) | Done |  |
| name both people when two follow you or star your commit (#1332) | Done |  |
| bring the avatar and handle into the profile's sticky bar once the avatar scrolls away … | Done |  |
| pick emoji from a button in the commit and reply composers (#252) | Done |  |
| search for profiles | Done |  |
| show day-N retention by signup day and by weekly cohort on the Growth page (#1337) | Skip | |
| release 0.4.0 mac os app | Skip | |

### 2026-11-01

14 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| add shared commit headlines (#1355) | Done | Signed headline routes (#1531) |
| let members post commits as their organization (#1361) | Done |  |
| let members schedule commits as their organization, canceled if they left it by then (#… | Done |  |
| show commits from organizations, under their handle and to their followers (#1359) | Done |  |
| tell the members of an organization about its commits and its mentions (#1368) | Skip | |
| prefetch on tabs | Skip | |
| name the mobile apps and prohibit CSAE in the terms (#1364) | Skip | |
| terms and conditions + front-end optimizations | Skip | |
| show moderators who posted an organization's commit, so they can time out or ban them (… | Skip | |
| show the chart's traffic metric on every breakdown row (#1378) | Skip | |
| follow the organizations users follow on GitHub, now or once they show up (#1360) | Skip | |
| chart the traffic over time and filter it by page, referrer, country, device or browser… | Skip | |
| commits on changelog | Skip | |
| notify whoever an organization replies to, quotes or mentions, from the organization (#… | Skip | |

### 2026-11-02

8 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| show hot topics and headline discussions | Done | Aside + in-app headline feed (#1531) |
| list an organization's own commits in the API, as its page's Commits tab (#1386) | Done |  |
| list who starred your own commit in the API (#1310) | Done |  |
| split daily active users into regulars and people back after a week away (#1382) | Done |  |
| refresh hot-topic recaps with concise event-focused briefs | Skip | |
| aggregate hot headlines with capped author contributions | Skip | |
| seed GitHub organizations with threads that mention them (#1366) | Skip | |
| generate short-lived recaps for hot headlines | Skip | |

### 2026-11-03

11 non-merge `feat`s on production.

| Item | Status | Notes |
| --- | --- | --- |
| give anyone on GitHub a page, looked up when a member first opens it (#1406) | Waiting | No stable `/api/v1` yet |
| link mentions of anyone to their page, which looks them up on GitHub (#1409) | Waiting | No stable `/api/v1` yet |
| let members wait to follow developers who aren't on /dev/null yet (#1407) | Waiting | No stable `/api/v1` yet |
| flicker the hot topics flame every five seconds | Done | Aside list; discussion pages in-app |
| preview recaps below bold hot topic headlines | Done | Aside + headline intro |
| move commit actions into a menu, with follow, mute, block and delete (#1371) | Done | Includes Send in Remotes |
| glide back to the present in conversations (#1250) | Done |  |
| offer a remote from the profile preview menu (#1308) | Done |  |
| add the TUI app to the apps page (#1445) | Skip | |
| suggest mobile app | Skip | |
| allow reclassifying commit headlines in batches | Skip | |
| hashtags | Done | `#tag` in commit text opens search |
| open profile pictures in the image viewer | Done | Zoom cursor; initials-only avatars stay plain |
| use a pointer cursor for profile pictures | Done | Zoom-in cursor on photos |
| show pastéis icon for Sweet Tooth achievement | Done | `pastel-de-nata` |
| add instant latest tab to semantic search | Done | Search results: Most relevant / Latest (past week) |
| draw the graph back out when folding replies away (#1449) | Partial | Replies leave immediately; the graph redraws. The website's reverse 500ms line animation is not ported |
| expand Tom community statistics with aggregates | Skip | |
| read public GitHub organization membership | Skip | Server-side org claims |

### 2026-11-04

| Item | Status | Notes |
| --- | --- | --- |
| Deduplicate changelog authors (#1411) | Skip | |
| Guidelines on models | Skip | |
| Point the CLI to Sync from GitHub (#1444) | Skip | |
| Keep follow suggestions on GitHub sync (#1358) | Done | |
| Follows-you on following lists (#1434) | Done | |
| Parallel tests on Windows (#1441) | Skip | |
| Hide mutuals on own connections (#1442) | Skip | No Mutuals tab yet |
| Code block language vs title (#1306) | Done | |
| Terms yes at app sign-in (#1385) | Done | Listed under Nov 3 stream of work; redeem sends `terms_accepted` |
| scroll to the top when reselecting a log (#1480) | Done | Ease to the top; a wheel or drag cancels it |
| replace suggested searches with hot topics | Done | Idle omnibox lists recent searches and hot topics |
| show latest hot topic matches | Done | Topics open the in-app headline page |
| add whoopsie achievement badge | Done | Mushroom-cloud icon |
| add event themes for Halloween, Christmas and New Year | Done | Appearance picker; palette follows this computer's date |
| Halloween 404, Christmas graph, Santa hat and New Year snippet | Done | Graph peak, hat on the mark, aside greeting. The website's Halloween 404 stays on the web |
| batch incoming log commits adaptively | Skip | Website realtime batching |
| keep live card counts and follow suggestions in sync (#1477) | Skip | Cross-device websocket; this client updates the counts it changes itself |
| keep the web app live across tabs and devices (#1458) | Skip | Browser tabs / Reverb |
| polish the UI on mobile (#1475) | Skip | |
| remote mobile styling fixes (#1462) | Skip | |

### 2026-11-05

| Item | Status | Notes |
| --- | --- | --- |
| prefetch the navigation's Log again (#1491) | Skip | Inertia prefetch |
| store phone push tokens and push preferences for the mobile app (#1489) | Skip | Phone app |

### 2026-11-06

| Item | Status | Notes |
| --- | --- | --- |
| add a mentions bell to org remotes (#1493) | Partial | Bell lists `mentions` / `mentions_count` from the org thread payload and jumps when the message is loaded. Clear / mark-all / load-older wait on signed `DELETE remotes/orgs/{org}/mentions` (web session only today) |
| clear org mentions once they are seen (#1498) | Waiting | Same signed clear route; web uses `message_ids` (1–99) |
| only count mentions as seen inside the zoomed view (#1499) | Waiting | Depends on #1498 |
| index graph parents and successors for reply paths (#1417) | Skip | Server graph perf |
| record which apps people use each day (#1505) | Skip | Admin Growth chart; this client stays `other_app` via UA / `app_id` |
| add the lite mobile app shell in apps/mobile (#1502) | Skip | Lite mobile |
| drive the lite mobile app's tab bar from the site (#1503) | Skip | Lite mobile |
| give the lite mobile app's web view a session from its device (#1510) | Skip | Lite mobile |
| count the lite mobile app separately in the app usage metrics (#1511) | Skip | Metrics / lite mobile |
| show the replied-to commit in the moderation queue and reports (#1509) | Skip | Moderator tools |
| require multiple root commits for hot topics | Skip | Server hot-topic rule |
| show linked commits for hot topics | Done | Headline page via signed API |
| remove the WebView experiment (#1501) | Skip | Website revert |
| describe how the Mac app updates itself (#1461) | Skip | Swift Mac app docs |

### 2026-11-07

| Item | Status | Notes |
| --- | --- | --- |
| make stargazers have a hovercard (#1204) | Done | Connections stargazers list binds the same hover card as the log |
| end each related commit with its own dot (#1459) | Done | Related list no longer chains a solid rail |
| try topics on the experimental links pages (#1513) | Skip | Inertia `/links` |
| rank links and their commits by stars of all time on the links pages (#1515) | Skip | Inertia `/links` ranking |
| use the page's title and favicon for GitHub links too (#1517) | Skip | Inertia `/links` chrome |
| draw the lite mobile app's launch and sign-in screens like the site's (#1514) | Skip | Lite mobile |
| sign in to the lite mobile app with a device key (#1512) | Skip | Lite mobile |
| update the TUI listing for themes and remotes (#1478) | Skip | TUI |

### 2026-11-08

| Item | Status | Notes |
| --- | --- | --- |
| removes feature flag around org commits (6e71709) | Done | Post-as follows `/me` organizations; no client feature gate |
| end each search result with its own dot (#1455) | Done | Search feed already uses disconnected cards (`connected: false`) |
| adjusts partners page | Skip | Website partners |
| keep formatting out of unselected lines (#1412) | Skip | Website editor |
| fit the replies picker on small screens (#1500) | Skip | Website layout |
| let Tom search repository history when asked who worked on a feature (#1487) | Skip | Tom / admin |

### 2026-11-09

| Item | Status | Notes |
| --- | --- | --- |
| end each hot topic commit with its own dot (#1454) | Done | Headline feed uses `connected: false` |

### 2026-11-10

| Item | Status | Notes |
| --- | --- | --- |
| fixes ci (7973e75) | Skip | Website CI |
| follow a theme another tab picks (#1524) | Skip | Multi-tab web theme sync |
| keep sidebar focus outlines whole (#1527) | Skip | Website CSS |

### 2026-11-11

| Item | Status | Notes |
| --- | --- | --- |
| translate binary and Morse code posts (5911a76) | Done | Server translation; client already shows translations |
| cloudflare `turnstile` / cloudflare on sign in | Skip | Web OAuth bot checks |
| warn before visiting external links (#1536) | Done | Leave-site dialog + trusted domains in prefs |
| read hot topics and headlines through the API (#1531) | Done | `GET headlines` + `GET headlines/{id}`; aside and omnibox open in-app |
| show XP only once in the profile achievements shelf (#1353) | Done | Header keeps Lv/rank; XP stays on the progress row |
| let API clients request features and report bugs (#1543) | Waiting | Signed `POST issues` exists; composer `/feature` `/bug` UI not ported |
| match link statistics to commits visible to the viewer (#1416) | Skip | Inertia `/links` |
| let API clients ask for more reply previews (#1528) | Done | Lists send `replies_per_page=3`; seeds nested previews from `reply_preview` |
| list an org's maintainers first, then everyone by name (#1234) | Done | Preserves API member order in remotes header |
| add emoji picker to quote composer (#1468) | Done | Smile button on quote window |
| let an org hold up to 256 people (#1233) | Done | Org picker cap raised to 256 |
| git blame achievement for contributors (#1271) | Done | `git-commit-horizontal` badge icon |
| let people pick a GitHub account after signing out (#1079) | Skip | Web OAuth account picker |
| centre the end dot on the actions row on desktop (#1544) | Done | `RAIL_TERMINATOR_PAD` 14px to match the 28px actions row |
| sponsors (847aaf1) | Skip | Website partners / sponsors page |
| restore offline GitHub and deterministic org tests (#1551) | Skip | Website tests |

### 2026-11-12

| Item | Status | Notes |
| --- | --- | --- |
| choose nostalgic remotes notification sounds (dcd7c54) | Waiting | Prefs + MSN/ICQ/AIM assets; GTK still uses the system message sound |
| leave Navi sounds out pending permission (78daafc) | Skip | Website sound asset gating |
| dont appear moderations that are not yet done (17bc041) | Skip | Server / admin moderation list |
| og for partners (7a1c746) | Skip | Website partners OG |
| count the commits that mention developers who aren't on /dev/null yet (#1548) | Waiting | Depends on unclaimed GitHub profiles (#1406) |

### 2026-11-13

| Item | Status | Notes |
| --- | --- | --- |
| notify users about timeouts and show moderation history (#1561) | Partial | `timed_out` notifications Done; own history Waiting on signed `GET me/moderation-history` (#1575 open) |
| align moderation history and show it to moderators (#1562) | Skip | Moderator / admin web |
| align moderation history with profile stats (bf8975e) | Skip | Website profile chrome |
| let members quote a commit as their organization (#1568) | Done | Quote dialog AuthorSwitcher; submit already sent `author` |
| show the author error in the main and reply composers (#1574) | Done | Clears refused org and toasts `errors.author` |
| open timezone and language pickers on first click (#1556) | Skip | Website dropdown focus |
| show the images a profile readme keeps beside it (#1555) | Done | Server resolves relative README images |
| give every minute limit its own counter (#1573) | Skip | Server rate-limit counters |
| show a timestamp's times on tap on touch screens (#1549) | Done | Commit time opens absolute-time popover |
| add the milestones since the stream to the about timeline (#1546) | Skip | Website about |
| list your organizations and Post as in the API's current user (#1492) | Done | `/me` organizations (+ role); Post as already wired |
| add public guilds with memberships and bulletin boards (#1576) | Waiting | Admin-default preview; no signed guild routes yet |
| removes vet in favour of `cooldown/period` (6500841) | Skip | Server chore |
| fix ci (a7a1582) | Skip | Website CI |


## Standing gaps (priority)

Blocked on API or hard infra — not same-day launch work:

1. **Mutuals / Followers you know** — connections tabs when list endpoints exist on `/api/v1`  
2. **Unclaimed GitHub profiles + wait-to-follow + mention lookup** — #1406 / #1407 / #1409 / #1548  
3. **Org Mentions clear / auto-seen** — signed `DELETE remotes/orgs/{org}/mentions` with `message_ids` (#1493 / #1498 / #1499); load older history when jumping  
4. **Voice notes** — record / upload / playback (Opus)  
5. **Calls** — WebRTC (button currently opens the web thread)  
6. **Commit link-card picker** — API create-commit does not accept `link_preview_url` yet  
7. **Feature / bug report from composer** — signed `POST /api/v1/issues` (#1543); slash-command UI  
8. **Own moderation history** — when `GET /api/v1/me/moderation-history` (#1575) merges  
9. **Guilds** — when signed routes exist and the feature leaves admin preview (#1576)  
10. **Remotes nostalgic sounds** — prefs + bundled MSN/ICQ/AIM cues  
11. **Recent search chips** — local prefs polish (web uses localStorage); optional  

## Out of scope (recurring Skip)

Do not track these as GTK work unless product direction changes:

- Website changelog / about / partners / apps / PotD chrome  
- Official CLI, TUI, Mac Swift app, iOS/Android betas, PWA  
- Admin ops, growth, traffic, retention, feature flags, Tom tools  
- Automod / keyboard moderation / CSAE policy CMS  
- SEO, sitemaps, Atom public feeds (consumers use HTTP; this app uses signed JSON)  
- Server-only refactors, CI, lockfiles, dependency bumps  

## How to update

1. Open [dev-null.sh/changelog](https://dev-null.sh/changelog) (or `git log` on `production` from [dev-null.sh](https://github.com/dev-null-sh/dev-null.sh)) for the days you are catching up.  
2. Add or adjust rows in [Day by day](#day-by-day-oct-15--present); update the [capability matrix](#capability-matrix-launch--present) when a capability’s status changes.  
3. Prefer implementing **Waiting** once a signed route exists; probe production carefully — do not invent paths.  
4. Keep [README](../README.md) Features aligned with what users can see; this file stays the ledger.

## Daily automation

`.github/workflows/changelog-parity.yml` runs every 24 hours (and on `workflow_dispatch`). It:

1. Reads recent `feat:` commits on `dev-null-sh/dev-null.sh@production` via the public GitHub API.  
2. Diffs them against this ledger.  
3. Opens a **ledger PR** that appends missing rows (`Skip` when the title matches out-of-scope heuristics, otherwise `Waiting`).  
4. For up to a few `Waiting` items, opens **port** work: Cursor Cloud Agents with `autoCreatePR` when the `CURSOR_API_KEY` repo secret is set; otherwise draft tracking PRs under `parity-port/…` with a task file in `.github/parity-tasks/`.

Local dry run:

```bash
python3 scripts/parity/sync_changelog.py --dry-run --repo "$GITHUB_REPOSITORY"
```
