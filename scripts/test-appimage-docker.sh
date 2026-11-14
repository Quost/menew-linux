#!/usr/bin/env bash
# Run from Git Bash/WSL with Docker: bash scripts/test-appimage-docker.sh <dir containing the AppImage>
# Installs in each clean container ONLY libraries from linuxdeploy's excludelist
# (https://github.com/probonopd/AppImages/blob/master/excludelist): GL/EGL, fontconfig, freetype,
# harfbuzz, X11, xcb, drm, gbm, wayland-client. Nothing from GTK/pango/fribidi.
set -euo pipefail
DIR="${1:?dir with the AppImage}"
run() { # image install-command
    echo "== $1"
    MSYS_NO_PATHCONV=1 docker run --rm -v "$DIR:/dist:ro" "$1" sh -c "$2
cp /dist/Menew-*.AppImage /tmp/a.AppImage && chmod +x /tmp/a.AppImage && cd /tmp
./a.AppImage --appimage-extract-and-run --version; echo exit=\$?
./a.AppImage --appimage-extract >/dev/null; cd squashfs-root
echo '--- libraries resolving outside the AppDir:'
for f in usr/bin/menew usr/lib/*.so* usr/lib/*/*/*.so; do [ -f \$f ] && LD_LIBRARY_PATH=\$PWD/usr/lib ldd \$f 2>/dev/null | awk '/=>/ {print \$3}'; done | sort -u | grep -v \"^\$PWD\" | xargs -n1 basename | sort -u | tr '\n' ' '; echo
echo '--- not found:'; for f in usr/bin/menew usr/lib/*.so* usr/lib/*/*/*.so; do [ -f \$f ] && LD_LIBRARY_PATH=\$PWD/usr/lib ldd \$f 2>/dev/null | grep 'not found'; done | sort -u; echo end"
}
run ubuntu:24.04 'apt-get update -qq >/dev/null && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libgl1 libegl1 libfontconfig1 libfreetype6 libharfbuzz0b libx11-6 libx11-xcb1 libxcb1 libdrm2 libgbm1 libwayland-client0 >/dev/null'
run fedora:latest 'dnf install -y -q mesa-libGL mesa-libEGL fontconfig freetype harfbuzz libX11 libX11-xcb libxcb libdrm mesa-libgbm libwayland-client >/dev/null'
