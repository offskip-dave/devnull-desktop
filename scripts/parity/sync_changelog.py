#!/usr/bin/env python3
"""Sync web production feats into changelog-parity.md and open port PRs.

Source: public GitHub commits on dev-null-sh/dev-null.sh@production.
Missing non-Skip feats become draft port PRs (gh) and, when CURSOR_API_KEY
is set, Cursor Cloud Agents with autoCreatePR.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[2]
PARITY_PATH = ROOT / "docs" / "changelog-parity.md"
WEB_REPO = "dev-null-sh/dev-null.sh"
WEB_BRANCH = "production"
CHANGELOG_URL = "https://dev-null.sh/changelog"
DEFAULT_LOOKBACK_DAYS = 3
DEFAULT_MAX_PORT_PRS = 3
DEFAULT_MAX_AGENTS = 3

# Titles matching these (case-insensitive) land as Skip in the ledger.
SKIP_PATTERNS = [
    r"\bcli\b",
    r"\btui\b",
    r"\bpwa\b",
    r"\bsitemap\b",
    r"\bseo\b",
    r"\batom\b",
    r"\brss\b",
    r"\bfavicon\b",
    r"\bsplash\b",
    r"\bonboarding\b",
    r"\bmobile app\b",
    r"\bios\b",
    r"\bandroid\b",
    r"\bswift\b",
    r"\bmac app\b",
    r"\bapps page\b",
    r"\bpartners\b",
    r"\bpotd\b",
    r"\badmin\b",
    r"\bmoderators?\b",
    r"\bmoderation\b",
    r"\bautomod\b",
    r"\bcsae\b",
    r"\btom\b",
    r"\btraffic\b",
    r"\bretention\b",
    r"\bgrowth\b",
    r"\bfeature flag\b",
    r"\blockfile\b",
    r"\bdependenc",
    r"\bci\b",
    r"\bwindows\b",
    r"\bparallel tests\b",
    r"\bguidelines on models\b",
    r"\bchangelog\b",
    r"\binertia\b",
    r"\bssr\b",
    r"\bmarketing\b",
    r"\bguest join\b",
    r"\bauto-login\b",
    r"\bkeyboard moderation\b",
]

FEAT_RE = re.compile(r"^feat(?:\([^)]*\))?:\s*(.+)$", re.I)
PR_RE = re.compile(r"#(\d+)\b")
SHA_RE = re.compile(r"\b([0-9a-f]{7,40})\b", re.I)
DAY_HEADING_RE = re.compile(r"^### (\d{4}-\d{2}-\d{2})\s*$", re.M)
TABLE_ROW_RE = re.compile(
    r"^\|\s*(.+?)\s*\|\s*(Done|Partial|Waiting|Skip(?:\s*/\s*Waiting)?(?:\s*\([^)]*\))?)\s*\|\s*(.*?)\s*\|$",
    re.M | re.I,
)


@dataclass(frozen=True)
class Feat:
    sha: str
    title: str  # conventional subject without "feat:" prefix
    date: date
    pr: int | None
    url: str

    @property
    def item_cell(self) -> str:
        base = self.title.strip()
        if self.pr is not None and f"#{self.pr}" not in base:
            return f"{base} (#{self.pr})"
        return base

    @property
    def slug(self) -> str:
        key = f"{self.pr}" if self.pr is not None else self.sha[:10]
        rough = re.sub(r"[^a-z0-9]+", "-", self.title.lower()).strip("-")[:40]
        return f"{key}-{rough}".strip("-")


def normalize_title(text: str) -> str:
    text = text.lower().strip()
    text = re.sub(r"^feat(?:\([^)]*\))?:\s*", "", text)
    text = PR_RE.sub("", text)
    text = re.sub(r"[^\w\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    return text


def classify(title: str) -> str:
    low = title.lower()
    for pat in SKIP_PATTERNS:
        if re.search(pat, low):
            return "Skip"
    return "Waiting"


def github_get(url: str, token: str | None = None) -> object:
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "devnull-parity-sync",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.load(resp)


def fetch_feats(lookback_days: int, token: str | None) -> list[Feat]:
    since = datetime.now(timezone.utc).date().toordinal() - lookback_days
    since_date = date.fromordinal(max(since, date(2026, 10, 15).toordinal()))
    feats: list[Feat] = []
    page = 1
    while page <= 10:
        qs = urllib.parse.urlencode(
            {
                "sha": WEB_BRANCH,
                "per_page": 100,
                "page": page,
                "since": f"{since_date.isoformat()}T00:00:00Z",
            }
        )
        url = f"https://api.github.com/repos/{WEB_REPO}/commits?{qs}"
        batch = github_get(url, token)
        if not isinstance(batch, list) or not batch:
            break
        for item in batch:
            msg = (item.get("commit") or {}).get("message") or ""
            subject = msg.split("\n", 1)[0].strip()
            m = FEAT_RE.match(subject)
            if not m:
                continue
            title = m.group(1).strip()
            # Drop trailing PR marker from title body for cleaner cells; keep pr field.
            prs = [int(x) for x in PR_RE.findall(subject)]
            pr = prs[-1] if prs else None
            title_clean = PR_RE.sub("", title).strip().rstrip("()").strip()
            title_clean = re.sub(r"\s+", " ", title_clean)
            authored = (item.get("commit") or {}).get("author") or {}
            day = datetime.fromisoformat(
                authored["date"].replace("Z", "+00:00")
            ).date()
            sha = item["sha"]
            feats.append(
                Feat(
                    sha=sha,
                    title=title_clean,
                    date=day,
                    pr=pr,
                    url=item.get("html_url")
                    or f"https://github.com/{WEB_REPO}/commit/{sha}",
                )
            )
        if len(batch) < 100:
            break
        page += 1

    # Newest first from API; keep stable unique by sha.
    seen: set[str] = set()
    unique: list[Feat] = []
    for feat in feats:
        if feat.sha in seen:
            continue
        seen.add(feat.sha)
        unique.append(feat)
    return unique


def parse_parity(text: str) -> tuple[set[int], set[str], set[str]]:
    """Return known PR numbers, normalized titles, and short shas mentioned."""
    prs: set[int] = set()
    titles: set[str] = set()
    shas: set[str] = set()
    for row in TABLE_ROW_RE.finditer(text):
        item = row.group(1)
        for n in PR_RE.findall(item):
            prs.add(int(n))
        titles.add(normalize_title(item))
        for sha in SHA_RE.findall(item):
            shas.add(sha[:7].lower())
    # Also catch PR refs elsewhere in notes columns.
    for n in PR_RE.findall(text):
        prs.add(int(n))
    return prs, titles, shas


def is_tracked(feat: Feat, prs: set[int], titles: set[str], shas: set[str]) -> bool:
    if feat.pr is not None and feat.pr in prs:
        return True
    if feat.sha[:7].lower() in shas:
        return True
    return normalize_title(feat.title) in titles


def ensure_day_section(text: str, day: date) -> str:
    heading = f"### {day.isoformat()}"
    if re.search(rf"^### {day.isoformat()}\s*$", text, re.M):
        return text
    # Insert after "## Day by day" block: before the first older day heading that
    # is chronologically after? Days are oldest-first in the doc. New days go at
    # the end of the day-by-day section (before Standing gaps).
    marker = "\n## Standing gaps"
    block = (
        f"\n{heading}\n\n"
        f"Auto-imported by the changelog parity cron.\n\n"
        f"| Item | Status | Notes |\n"
        f"| --- | --- | --- |\n"
    )
    if marker in text:
        return text.replace(marker, block + marker, 1)
    return text + block


def insert_row(text: str, day: date, item: str, status: str, notes: str) -> str:
    text = ensure_day_section(text, day)
    heading = f"### {day.isoformat()}"
    # Find the table under this heading and append a row before the next ### or ##.
    pattern = re.compile(
        rf"({re.escape(heading)}\n(?:.*?\n)*?\| --- \| --- \| --- \|\n)"
        rf"((?:\|.*\|\n)*)",
        re.M,
    )
    m = pattern.search(text)
    if not m:
        raise RuntimeError(f"could not find day table for {day}")
    rows = m.group(2)
    new_row = f"| {item} | {status} | {notes} |\n"
    # Avoid duplicate rows for the same item cell.
    if new_row in rows or f"| {item} |" in rows:
        return text
    return text[: m.end(1)] + rows + new_row + text[m.end() :]


def existing_port_slugs(token: str | None, repo: str) -> set[str]:
    """Open PR titles/heads that already cover a port slug."""
    found: set[str] = set()
    try:
        qs = urllib.parse.urlencode({"state": "open", "per_page": 100})
        data = github_get(f"https://api.github.com/repos/{repo}/pulls?{qs}", token)
    except urllib.error.HTTPError as err:
        print(f"warning: could not list open PRs: {err}", file=sys.stderr)
        return found
    if not isinstance(data, list):
        return found
    for pr in data:
        title = (pr.get("title") or "").lower()
        head = ((pr.get("head") or {}).get("ref") or "").lower()
        blob = f"{title} {head}"
        for m in re.finditer(r"parity-port/([a-z0-9-]+)", blob):
            found.add(m.group(1))
        for m in re.finditer(r"port:\s*([^\n(]+)", title):
            found.add(normalize_title(m.group(1)).replace(" ", "-")[:40])
    return found


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kwargs)


def gh_create_pr(
    *,
    branch: str,
    title: str,
    body: str,
    base: str,
    draft: bool,
) -> str:
    args = [
        "gh",
        "pr",
        "create",
        "--base",
        base,
        "--head",
        branch,
        "--title",
        title,
        "--body",
        body,
    ]
    if draft:
        args.append("--draft")
    out = run(args, cwd=ROOT)
    return out.stdout.strip()


def launch_cursor_agent(*, prompt: str, repo_url: str, starting_ref: str) -> dict:
    api_key = os.environ.get("CURSOR_API_KEY", "").strip()
    if not api_key:
        raise RuntimeError("CURSOR_API_KEY is not set")
    payload = {
        "prompt": {"text": prompt},
        "repos": [{"url": repo_url, "startingRef": starting_ref}],
        "autoCreatePR": True,
        "skipReviewerRequest": True,
    }
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        "https://api.cursor.com/v1/agents",
        data=data,
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
    )
    # Cloud Agents also accept basic auth with key as username.
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            return json.load(resp)
    except urllib.error.HTTPError as err:
        # Retry with basic auth form used in docs (-u KEY:).
        if err.code not in (401, 403):
            raise
        import base64

        basic = base64.b64encode(f"{api_key}:".encode()).decode()
        req2 = urllib.request.Request(
            "https://api.cursor.com/v1/agents",
            data=data,
            method="POST",
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Basic {basic}",
            },
        )
        with urllib.request.urlopen(req2, timeout=120) as resp:
            return json.load(resp)


def port_prompt(feat: Feat) -> str:
    return f"""Port this website production feature into the GTK /dev/null client.

## Web feature
- Title: {feat.title}
- Commit: {feat.url}
- PR: {f'#{feat.pr}' if feat.pr else 'none'}
- Changelog: {CHANGELOG_URL}

## Rules (must follow)
1. Read docs/changelog-parity.md, CONTRIBUTING.md, and docs/architecture.md first.
2. Prefer signed `/api/v1` only. Probe carefully; do not invent endpoints that 404.
3. Do not scrape HTML or embed a WebView for OAuth.
4. If there is no usable signed API yet, update docs/changelog-parity.md to Waiting with a one-line why, and stop without fake UI data.
5. If the feature is web/CLI/admin/CMS/other-app only, mark Skip in the parity ledger and stop.
6. When you implement something, mark the day-by-day row and capability matrix Done or Partial with notes.
7. Keep diffs focused. Match existing Relm4 / GTK patterns in crates/devnull.
8. Run `cargo fmt --all` and `cargo clippy --locked --workspace --all-targets -- -D warnings` before finishing.
9. Open a PR whose description links the web commit/PR and lists how you tested.

Implement the port (or correctly mark Waiting/Skip) and leave the tree ready for review.
"""


def write_port_task(feat: Feat, status: str) -> Path:
    tasks = ROOT / ".github" / "parity-tasks"
    tasks.mkdir(parents=True, exist_ok=True)
    path = tasks / f"{feat.slug}.md"
    path.write_text(
        f"""# Port task: {feat.title}

- Status hint: `{status}`
- Web commit: {feat.url}
- Web PR: {f'#{feat.pr}' if feat.pr else 'n/a'}
- Date: {feat.date.isoformat()}
- Changelog: {CHANGELOG_URL}

## Checklist

- [ ] Confirm a signed `/api/v1` route exists (or mark Waiting / Skip in `docs/changelog-parity.md`)
- [ ] Implement in the GTK client without inventing endpoints
- [ ] Update capability matrix + day-by-day rows
- [ ] `cargo fmt` / clippy / tests
""",
        encoding="utf-8",
    )
    return path


def git_configured() -> None:
    # Actions provides identity via the runner; set local fallbacks for dry runs.
    def get(key: str) -> str:
        try:
            return run(["git", "config", "--get", key], cwd=ROOT).stdout.strip()
        except subprocess.CalledProcessError:
            return ""

    if not get("user.email"):
        run(
            ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
            cwd=ROOT,
        )
    if not get("user.name"):
        run(["git", "config", "user.name", "github-actions[bot]"], cwd=ROOT)


def reset_to_base(base: str) -> None:
    run(["git", "checkout", base], cwd=ROOT)
    run(["git", "reset", "--hard", f"origin/{base}"], cwd=ROOT)
    run(["git", "clean", "-fd"], cwd=ROOT)


def create_branch_commit_push(branch: str, paths: Iterable[Path], message: str) -> bool:
    """Commit and force-push. Returns False when there was nothing to commit."""
    git_configured()
    run(["git", "checkout", "-B", branch], cwd=ROOT)
    run(["git", "add", *[str(p) for p in paths]], cwd=ROOT)
    status = run(["git", "status", "--porcelain"], cwd=ROOT).stdout.strip()
    if not status:
        return False
    run(["git", "commit", "-m", message], cwd=ROOT)
    run(["git", "push", "-u", "origin", branch, "--force"], cwd=ROOT)
    return True


def pr_exists(repo: str, head_branch: str, token: str | None) -> bool:
    owner, _, name = repo.partition("/")
    if not owner or not name:
        return False
    qs = urllib.parse.urlencode(
        {"head": f"{owner}:{head_branch}", "state": "open", "per_page": 5}
    )
    try:
        data = github_get(f"https://api.github.com/repos/{repo}/pulls?{qs}", token)
    except urllib.error.HTTPError:
        return False
    return isinstance(data, list) and bool(data)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lookback-days", type=int, default=DEFAULT_LOOKBACK_DAYS)
    parser.add_argument("--max-port-prs", type=int, default=DEFAULT_MAX_PORT_PRS)
    parser.add_argument("--max-agents", type=int, default=DEFAULT_MAX_AGENTS)
    parser.add_argument(
        "--base-branch",
        default=os.environ.get("PARITY_BASE_BRANCH", "main"),
    )
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY", ""),
        help="owner/name of this GTK client repo",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print actions only; do not push, open PRs, or launch agents",
    )
    parser.add_argument(
        "--no-agents",
        action="store_true",
        help="never call Cursor Cloud Agents even if CURSOR_API_KEY is set",
    )
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    parity_text = PARITY_PATH.read_text(encoding="utf-8")
    known_prs, known_titles, known_shas = parse_parity(parity_text)

    print(f"fetching feats from {WEB_REPO}@{WEB_BRANCH}…", flush=True)
    feats = fetch_feats(args.lookback_days, token)
    missing = [f for f in feats if not is_tracked(f, known_prs, known_titles, known_shas)]
    print(f"found {len(feats)} feat(s) in lookback; {len(missing)} missing from parity", flush=True)

    if not missing:
        print("nothing to do")
        return 0

    # Apply ledger rows for all missing (Skip and Waiting).
    updated = parity_text
    ledger_feats: list[tuple[Feat, str]] = []
    for feat in sorted(missing, key=lambda f: (f.date, f.sha)):
        status = classify(feat.title)
        notes = (
            "Out of scope for GTK (auto)"
            if status == "Skip"
            else f"Auto-imported from [{feat.sha[:7]}]({feat.url}); needs GTK port or API probe"
        )
        updated = insert_row(updated, feat.date, feat.item_cell, status, notes)
        ledger_feats.append((feat, status))
        print(f"  + {feat.date} [{status}] {feat.item_cell}", flush=True)

    if args.dry_run:
        print("--- dry-run parity diff preview (first 4k) ---")
        # Show only changed length hint.
        print(f"parity would grow by {len(updated) - len(parity_text)} bytes")
        waiting = [f for f, s in ledger_feats if s == "Waiting"]
        print(f"would open up to {min(len(waiting), args.max_port_prs)} port PR(s)")
        return 0

    if not args.repo:
        print("error: --repo or GITHUB_REPOSITORY is required to open PRs", file=sys.stderr)
        return 2

    repo_url = f"https://github.com/{args.repo}"
    open_slugs = existing_port_slugs(token, args.repo)

    # Ensure origin base is current before branching.
    run(["git", "fetch", "origin", args.base_branch], cwd=ROOT)
    reset_to_base(args.base_branch)

    # 1) Ledger PR with all new rows.
    ledger_branch = f"parity/ledger-{date.today().isoformat()}"
    PARITY_PATH.write_text(updated, encoding="utf-8")
    committed = create_branch_commit_push(
        ledger_branch,
        [PARITY_PATH],
        f"chore(parity): import web feats from {date.today().isoformat()}",
    )
    reset_to_base(args.base_branch)

    ledger_body = f"""## Summary
- Import missing production `feat`s from [`{WEB_REPO}`]({CHANGELOG_URL}) into `docs/changelog-parity.md`.
- Lookback: last {args.lookback_days} day(s).

## Imported
""" + "\n".join(
        f"- `{status}` {feat.item_cell} — {feat.url}" for feat, status in ledger_feats
    )
    if committed and not pr_exists(args.repo, ledger_branch, token):
        try:
            url = gh_create_pr(
                branch=ledger_branch,
                title=f"chore(parity): sync web changelog {date.today().isoformat()}",
                body=ledger_body,
                base=args.base_branch,
                draft=False,
            )
            print(f"ledger PR: {url}", flush=True)
        except subprocess.CalledProcessError as err:
            print(f"warning: ledger PR create failed: {err.stderr}", file=sys.stderr)
    else:
        print(f"ledger branch ready (committed={committed}); PR may already exist", flush=True)

    # 2) Port PRs / agents for Waiting items.
    waiting = [f for f, s in ledger_feats if s == "Waiting"]
    ports_opened = 0
    agents_launched = 0
    use_agents = (not args.no_agents) and bool(os.environ.get("CURSOR_API_KEY", "").strip())

    for feat in waiting:
        if ports_opened >= args.max_port_prs:
            break
        if feat.slug in open_slugs:
            print(f"skip existing port for {feat.slug}", flush=True)
            continue

        if use_agents and agents_launched < args.max_agents:
            try:
                result = launch_cursor_agent(
                    prompt=port_prompt(feat),
                    repo_url=repo_url,
                    starting_ref=args.base_branch,
                )
                agents_launched += 1
                ports_opened += 1
                print(f"cursor agent: {json.dumps(result)[:500]}", flush=True)
                continue
            except Exception as err:  # noqa: BLE001 - keep cron resilient
                print(f"warning: agent launch failed for {feat.slug}: {err}", file=sys.stderr)

        # Fallback: draft PR with task file (no fake implementation).
        branch = f"parity-port/{feat.slug}"
        if pr_exists(args.repo, branch, token):
            print(f"skip existing PR head {branch}", flush=True)
            continue
        reset_to_base(args.base_branch)
        text = PARITY_PATH.read_text(encoding="utf-8")
        text = insert_row(
            text,
            feat.date,
            feat.item_cell,
            "Waiting",
            f"Port task opened; web [{feat.sha[:7]}]({feat.url})",
        )
        PARITY_PATH.write_text(text, encoding="utf-8")
        task = write_port_task(feat, "Waiting")
        if not create_branch_commit_push(
            branch,
            [PARITY_PATH, task],
            f"parity: track port for {feat.title}",
        ):
            reset_to_base(args.base_branch)
            continue
        reset_to_base(args.base_branch)
        body = f"""## Summary
Draft port tracking PR for a website `feat` not yet in this GTK client.

- Web: {feat.url}
- Changelog: {CHANGELOG_URL}
- Task file: `{task.relative_to(ROOT)}`

## Rules
Follow CONTRIBUTING.md / Changelog parity: signed `/api/v1` only, no invented routes, mark Waiting/Skip if blocked.

## Test plan
- [ ] Confirm API surface
- [ ] Implement or correctly mark Waiting/Skip
- [ ] `cargo fmt` / clippy / tests
"""
        try:
            url = gh_create_pr(
                branch=branch,
                title=f"port: {feat.title}",
                body=body,
                base=args.base_branch,
                draft=True,
            )
            print(f"port PR: {url}", flush=True)
            ports_opened += 1
        except subprocess.CalledProcessError as err:
            print(f"warning: port PR failed: {err.stderr}", file=sys.stderr)

    print(
        f"done: ledger_feats={len(ledger_feats)} ports_opened={ports_opened} agents={agents_launched}",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except urllib.error.HTTPError as err:
        print(f"HTTP error: {err.code} {err.reason}", file=sys.stderr)
        body = err.read().decode("utf-8", "replace")
        if body:
            print(body[:1000], file=sys.stderr)
        raise SystemExit(1)
