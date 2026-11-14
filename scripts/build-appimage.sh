#!/usr/bin/env bash
# Build Menew-<version>-x86_64.AppImage with linuxdeploy + linuxdeploy-plugin-gtk (bundles GTK4 and libadwaita).
# Run from the repository root after `cargo build --release -p menew`.
# Build tooling only: the downloaded tools are pinned and verified by SHA256. Nothing is sent anywhere.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
BIN="$TARGET_DIR/release/menew"
OUT="${OUT_DIR:-$ROOT/dist}"
TOOLS="${TOOLS_DIR:-$ROOT/dist/tools}"

LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20250213-2/linuxdeploy-x86_64.AppImage"
LINUXDEPLOY_SHA256="4648f278ab3ef31f819e67c30d50f462640e5365a77637d7e6f2ad9fd0b4522a"
GTK_PLUGIN_URL="https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/3b67a1d1c1b0c8268f57f2bce40fe2d33d409cea/linuxdeploy-plugin-gtk.sh"
GTK_PLUGIN_SHA256="b0f4cbc684a0103a9651f0955b635eaea0096b3a66c0f5a2c2aa337960375171"

[ -x "$BIN" ] || { echo "missing $BIN (run: cargo build --release -p menew)" >&2; exit 1; }
mkdir -p "$OUT" "$TOOLS"

fetch() { # url sha256 dest
    if [ ! -f "$3" ] || ! echo "$2  $3" | sha256sum -c --quiet 2>/dev/null; then
        curl -fsSL -o "$3" "$1"
    fi
    echo "$2  $3" | sha256sum -c --quiet
    chmod +x "$3"
}
fetch "$LINUXDEPLOY_URL" "$LINUXDEPLOY_SHA256" "$TOOLS/linuxdeploy-x86_64.AppImage"
fetch "$GTK_PLUGIN_URL" "$GTK_PLUGIN_SHA256" "$TOOLS/linuxdeploy-plugin-gtk.sh"

APPDIR="$OUT/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR"

# FUSE is not needed to run the build tool itself.
export APPIMAGE_EXTRACT_AND_RUN=1
export DEPLOY_GTK_VERSION=4
export NO_STRIP=1
export LDAI_OUTPUT="$OUT/Menew-$VERSION-x86_64.AppImage"
export PATH="$TOOLS:$PATH"

# libfribidi is on the linuxdeploy excludelist (assumed present on every host) but minimal hosts
# lack it, and pango needs it. Explicit --library entries are always bundled.
EXTRA_LIBS=()
for lib in libfribidi.so.0; do
    path="$(ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ {print $NF; exit}')"
    [ -n "$path" ] || { echo "cannot find $lib on the build host" >&2; exit 1; }
    EXTRA_LIBS+=(--library "$path")
done

"$TOOLS/linuxdeploy-x86_64.AppImage" \
    "${EXTRA_LIBS[@]}" \
    --appdir "$APPDIR" \
    --executable "$BIN" \
    --desktop-file data/app.menew.Menew.desktop \
    --icon-file data/icons/hicolor/scalable/apps/menew.svg \
    --plugin gtk \
    --output appimage

# linuxdeploy names the file itself; make sure the name is the expected one.
if [ ! -f "$OUT/Menew-$VERSION-x86_64.AppImage" ]; then
    mv "$OUT"/Menew*-x86_64.AppImage "$OUT/Menew-$VERSION-x86_64.AppImage"
fi
ls -la "$OUT/Menew-$VERSION-x86_64.AppImage"
