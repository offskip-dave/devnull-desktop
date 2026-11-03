#!/usr/bin/env bash
# Build (and optionally install) the Flatpak from flatpak/sh.devnull.Client.yml.
# Usage: scripts/build-flatpak.sh [--install]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MANIFEST="flatpak/sh.devnull.Client.yml"
APP_ID="sh.devnull.Client"
BUILD_DIR="${FLATPAK_BUILD_DIR:-build-dir}"
REPO_DIR="${FLATPAK_REPO_DIR:-.flatpak-repo}"
DIST_DIR="${DIST_DIR:-dist}"
INSTALL=0

for arg in "$@"; do
  case "$arg" in
    --install) INSTALL=1 ;;
    -h | --help)
      echo "Usage: $0 [--install]"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

if ! command -v flatpak-builder >/dev/null 2>&1; then
  echo "flatpak-builder is required" >&2
  exit 1
fi

ARGS=(--user --force-clean)
if [[ "$INSTALL" -eq 1 ]]; then
  ARGS+=(--install)
fi

flatpak-builder "${ARGS[@]}" --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"

mkdir -p "$DIST_DIR"
BUNDLE="$DIST_DIR/devnull.flatpak"
flatpak build-bundle "$REPO_DIR" "$BUNDLE" "$APP_ID"
echo "wrote $BUNDLE"
