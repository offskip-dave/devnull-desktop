#!/usr/bin/env bash
# Build a release GTK binary for macOS (Homebrew), copy to dist/.
# This is not the first-party Swift .app / .dmg.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DIST_DIR="${DIST_DIR:-dist}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS packaging requires Darwin" >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required (gtk4, libadwaita, pkgconf)" >&2
  exit 1
fi

export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix libffi)/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export DYLD_FALLBACK_LIBRARY_PATH="$(brew --prefix)/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"

cargo build --release --locked -p devnull

ARCH="$(uname -m)"
mkdir -p "$DIST_DIR"
OUT="$DIST_DIR/devnull-macos-${ARCH}"
cp target/release/devnull "$OUT"
chmod +x "$OUT"
echo "wrote $OUT"
