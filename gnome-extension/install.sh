#!/usr/bin/env bash
set -euo pipefail

UUID="menew-bridge@menew.web.app"
DEST_DIR="${HOME}/.local/share/gnome-shell/extensions/${UUID}"

echo "Installing Menew GNOME Shell extension to ${DEST_DIR}..."
mkdir -p "${DEST_DIR}"
cp metadata.json extension.js "${DEST_DIR}/"

echo "Extension files installed."
echo "To enable:"
echo "  1. If running X11, press Alt+F2, type 'r', and hit Enter."
echo "  2. If running Wayland, log out and log back in (GNOME Shell limitation for new extensions)."
echo "  3. Enable the extension via CLI: gnome-extensions enable ${UUID}"
echo "  4. Or enable it via the GNOME 'Extensions' app."
