#!/usr/bin/env bash
# Regenerate the README screenshots (docs/assets) with `menew --snapshot`, in a headless GNOME session.
# Run inside Ubuntu 24.04 (WSL) or any machine with the packages listed in scripts/desktop-test/README.md:
#   cargo build && bash scripts/snapshots.sh            # all languages
#   bash scripts/snapshots.sh en pt                      # only some languages
#
# English goes to docs/assets/, the others to docs/assets/<lang>/ with the same file names.
# The images are drawn by the app itself (the palette card with a soft shadow and a transparent
# margin, so there is nothing to crop), from fake data only: see docs/snapshots.md.
#
# Fonts: Cantarell and the Adwaita icons are the GNOME defaults; Noto Color Emoji is needed for the emoji
# screen and a CJK font (fonts-noto-cjk) for Chinese. A missing font is a hard error, never a box in the image.
set -euo pipefail

SCRIPT="$(readlink -f "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SCRIPT")"

if [ "${1:-}" != "--inner" ]; then
    LANGS=("$@")
    [ "${#LANGS[@]}" -gt 0 ] || LANGS=(en pt de es zh)
    export TEST_ROOT
    TEST_ROOT="$(mktemp -d /tmp/menew-snap-XXXXXX)"
    export HOME="$TEST_ROOT/home"
    mkdir -p "$HOME"
    export XDG_RUNTIME_DIR="$TEST_ROOT/run"
    mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
    export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" XDG_CACHE_HOME="$HOME/.cache"
    export GSETTINGS_BACKEND=keyfile
    unset WAYLAND_DISPLAY DISPLAY
    set +e
    dbus-run-session -- bash "$SCRIPT" --inner "${LANGS[@]}"
    rc="$(cat "$TEST_ROOT/inner.rc" 2>/dev/null || echo 1)"
    set -e
    fusermount -uz "$XDG_RUNTIME_DIR/doc" 2>/dev/null || true
    rm -rf "$TEST_ROOT" 2>/dev/null || true
    exit "$rc"
fi
shift

# shellcheck source=desktop-test/lib.sh
source "$HERE/desktop-test/lib.sh"
SHELL_LOG="$TEST_ROOT/gnome-shell.log"
cleanup() {
    echo "$?" > "$TEST_ROOT/inner.rc"
    kill_tracked
    pkill -u "$(id -u)" -f "$TEST_ROOT" 2>/dev/null || true
}
trap cleanup EXIT

[ -x "$MENEW_BIN" ] || { echo "FAIL: menew binary not found at $MENEW_BIN (cargo build first, or set MENEW_BIN)"; exit 1; }

have_font() { fc-list "$1" | grep -q .; }
have_font "Cantarell" || { echo "FAIL: font Cantarell is missing (apt install fonts-cantarell)"; exit 1; }
have_font "Noto Color Emoji" || { echo "FAIL: Noto Color Emoji is missing (apt install fonts-noto-color-emoji)"; exit 1; }
for l in "$@"; do
    if [ "$l" = zh ] && [ -z "$(fc-list :lang=zh)" ]; then
        echo "FAIL: no font with Chinese glyphs (sudo apt install fonts-noto-cjk), the zh images would be boxes"; exit 1
    fi
done
[ -d /usr/share/icons/Adwaita ] || { echo "FAIL: Adwaita icon theme is missing (apt install adwaita-icon-theme)"; exit 1; }

gsettings set org.gnome.desktop.interface icon-theme 'Adwaita'
gsettings set org.gnome.desktop.interface font-name 'Cantarell 11'

export XDG_SESSION_TYPE=wayland
gnome-shell --wayland --headless --no-x11 --virtual-monitor 1280x800 >"$SHELL_LOG" 2>&1 &
track $!
if wait_for 30 bash -c 'for s in "$XDG_RUNTIME_DIR"/wayland-*; do [ -S "$s" ] && [[ "$s" != *.lock ]] && exit 0; done; exit 1'; then
    pass "headless gnome-shell started"
else
    fail "headless gnome-shell did not create a wayland socket"; tail -30 "$SHELL_LOG"; exit 1
fi
WAYLAND_DISPLAY="$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")"
export WAYLAND_DISPLAY
sleep 3

# --- bait: user data that --snapshot must never read or change -------------------------------
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_DATA_HOME/menew" "$XDG_CONFIG_HOME/menew"
cat > "$XDG_DATA_HOME/applications/real-bait-app.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=REAL-BAIT-APP
Exec=true
EOF
export MENEW_DB_PATH="$TEST_ROOT/real-clipboard.db"
sqlite3 "$MENEW_DB_PATH" "CREATE TABLE bait (v TEXT); INSERT INTO bait VALUES ('REAL-BAIT-CLIPBOARD');"
printf 'REAL-BAIT-CONFIG\n' > "$XDG_CONFIG_HOME/menew/bait.conf"
bait_sum() { (sqlite3 "$MENEW_DB_PATH" "SELECT group_concat(v) FROM bait;"; find "$XDG_DATA_HOME/menew" "$XDG_CONFIG_HOME/menew" "$XDG_DATA_HOME/applications" -type f -exec md5sum {} + | sort) | md5sum; }
BEFORE="$(bait_sum)"
# a bait on the real clipboard (works where the compositor lets a client without focus set it)
CLIP_BAIT="REAL-BAIT-CLIPBOARD-TEXT"
printf '%s' "$CLIP_BAIT" | timeout 5 wl-copy 2>/dev/null || true

for lang in "$@"; do
    case "$lang" in
        en) out="$REPO_ROOT/docs/assets" ;;
        pt|de|es|zh) out="$REPO_ROOT/docs/assets/$lang" ;;
        *) fail "unknown language '$lang'"; continue ;;
    esac
    mkdir -p "$out"
    rm -f "$out"/palette*-light.png "$out"/palette*-dark.png "$out"/clipboard*-light.png "$out"/clipboard*-dark.png
    if "$MENEW_BIN" --snapshot "$out" -language "$lang" >"$TEST_ROOT/snap-$lang.log" 2>&1; then
        n="$(grep -c '^snapshot: ' "$TEST_ROOT/snap-$lang.log" || true)"
        pass "menew --snapshot -language $lang wrote $n images to ${out#"$REPO_ROOT"/}"
    else
        fail "menew --snapshot -language $lang failed"; tail -20 "$TEST_ROOT/snap-$lang.log"
    fi
    for f in "$out"/*.png; do
        case "$(basename "$f")" in icon.png) continue ;; esac
        colors="$(distinct_colors "$f")"
        [ "${colors:-0}" -gt 20 ] || fail "$(basename "$f") looks blank ($colors colours)"
    done
done

# --- the privacy check: nothing of the user was read or changed ------------------------------
AFTER="$(bait_sum)"
if [ "$BEFORE" = "$AFTER" ]; then pass "bait database, config and menew files in the temporary HOME are unchanged"; else fail "--snapshot changed files of the user"; fi
if [ -n "$(find "$XDG_DATA_HOME/menew" "$XDG_CONFIG_HOME/menew" -type f ! -name bait.conf 2>/dev/null)" ]; then
    fail "--snapshot created files in the menew data or config folders"
else
    pass "no new file in the menew data or config folders"
fi
if grep -rl -a 'REAL-BAIT' "$TEST_ROOT"/snap-*.log >/dev/null 2>&1; then fail "a bait string showed up in the snapshot output"; else pass "no bait string in the snapshot output"; fi
if [ "$(timeout 5 wl-paste --no-newline 2>/dev/null || true)" = "$CLIP_BAIT" ] || [ -z "$(timeout 5 wl-paste --no-newline 2>/dev/null || true)" ]; then
    pass "the real clipboard was not replaced by --snapshot"
else
    fail "the real clipboard changed during --snapshot"
fi

echo "FAILS=$FAILS"
[ "$FAILS" -eq 0 ]
