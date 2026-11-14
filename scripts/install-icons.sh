#!/usr/bin/env bash
set -euo pipefail

DEST="${HOME}/.local/share/icons/hicolor/scalable/apps"
mkdir -p "${DEST}"
cp data/icons/hicolor/scalable/apps/menew.svg "${DEST}/"
cp data/icons/hicolor/scalable/apps/menew-symbolic.svg "${DEST}/"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" || true
fi

echo "Icons installed to ${DEST}"
