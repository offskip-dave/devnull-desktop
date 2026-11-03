#!/usr/bin/env bash
# Build a host-GTK AppImage into dist/. Does not bundle GTK/libadwaita.
# Requires: cargo, GTK 4.14+, libadwaita 1.5+, curl (to fetch appimagetool unless present).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DIST_DIR="${DIST_DIR:-dist}"
APPDIR="${APPDIR:-AppDir}"
ARCH="${ARCH:-$(uname -m)}"
case "$ARCH" in
  x86_64 | amd64) ARCH=x86_64 ;;
  aarch64 | arm64) ARCH=aarch64 ;;
esac

APP_ID="sh.devnull.Client"
ICON_SVG="resources/icons/hicolor/scalable/apps/${APP_ID}.svg"
DESKTOP="resources/${APP_ID}.desktop"
METAINFO="resources/${APP_ID}.metainfo.xml"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "AppImage builds are Linux-only" >&2
  exit 1
fi

cargo build --release --locked -p devnull

rm -rf "$APPDIR"
mkdir -p \
  "$APPDIR/usr/bin" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/metainfo" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps"

cp target/release/devnull "$APPDIR/usr/bin/devnull"
cp "$DESKTOP" "$APPDIR/usr/share/applications/${APP_ID}.desktop"
cp "$METAINFO" "$APPDIR/usr/share/metainfo/${APP_ID}.metainfo.xml"
cp "$ICON_SVG" "$APPDIR/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
cp -a resources/icons/hicolor/. "$APPDIR/usr/share/icons/hicolor/"

ln -sf "usr/share/applications/${APP_ID}.desktop" "$APPDIR/${APP_ID}.desktop"
ln -sf "usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg" "$APPDIR/${APP_ID}.svg"
ln -sf "usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg" "$APPDIR/.DirIcon"

cat >"$APPDIR/AppRun" <<'EOF'
#!/bin/bash
# Host-GTK AppRun: does not bundle GTK4/libadwaita (avoids theme breakage across distros).
# Needs GTK 4.14+, libadwaita 1.5+, and a Secret Service on the host.
HERE="$(dirname "$(readlink -f "$0")")"
export XDG_DATA_DIRS="${HERE}/usr/share:/usr/share:/usr/local/share:${XDG_DATA_DIRS:-/usr/share}"

LDCONFIG=""
for candidate in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
  if command -v "$candidate" >/dev/null 2>&1; then
    LDCONFIG="$candidate"
    break
  fi
done

if [ -n "$LDCONFIG" ]; then
  for lib in libadwaita-1.so.0 libgtk-4.so.1; do
    if ! "$LDCONFIG" -p 2>/dev/null | grep -q "$lib"; then
      echo "/dev/null: $lib is missing." >&2
      echo " Needs GTK 4.14+, libadwaita 1.5+ (GNOME 46+ / Ubuntu 24.04+, Fedora 40+)." >&2
      echo " Prefer Flatpak on older hosts: flatpak install / run sh.devnull.Client" >&2
      exit 1
    fi
  done
fi

exec "${HERE}/usr/bin/devnull" "$@"
EOF
chmod +x "$APPDIR/AppRun"

TOOL="${APPIMAGETOOL:-}"
if [[ -z "$TOOL" ]]; then
  TOOL="$ROOT/.cache/appimagetool-${ARCH}.AppImage"
  mkdir -p "$ROOT/.cache"
  if [[ ! -x "$TOOL" ]]; then
    curl -fsSL \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage" \
      -o "$TOOL"
    chmod +x "$TOOL"
  fi
fi

mkdir -p "$DIST_DIR"
OUT="$DIST_DIR/devnull-${ARCH}.AppImage"
# CI runners often lack FUSE; --appimage-extract-and-run works without it.
ARCH="$ARCH" "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT"
echo "wrote $OUT"
