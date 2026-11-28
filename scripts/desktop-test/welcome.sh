#!/usr/bin/env bash
# First run on a headless GNOME Shell (no GlobalShortcuts portal, like GNOME 46 on Ubuntu 24.04), as a new user:
# Menew turns start at login on, writes and turns on its GNOME extension, creates the custom shortcuts and opens
# the welcome window; closing it is remembered and `menew open` then shows the palette. Also checks that the old
# extension name is replaced. Run inside WSL Ubuntu 24.04:
#   bash scripts/desktop-test/welcome.sh                          # a build (MENEW_BIN, default ~/target-menew/debug/menew)
#   MENEW_BIN=/usr/bin/menew bash scripts/desktop-test/welcome.sh # the installed .deb
set -euo pipefail

SCRIPT="$(readlink -f "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SCRIPT")"

if [ "${1:-}" != "--inner" ]; then
    export TEST_ROOT
    TEST_ROOT="$(mktemp -d /tmp/menew-welcome-XXXXXX)"
    export HOME="$TEST_ROOT/home"
    mkdir -p "$HOME"
    export XDG_RUNTIME_DIR="$TEST_ROOT/run"
    mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
    export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" XDG_CACHE_HOME="$HOME/.cache"
    export GSETTINGS_BACKEND=keyfile
    unset WAYLAND_DISPLAY DISPLAY MENEW_TEST MENEW_DB_PATH
    set +e
    dbus-run-session -- bash "$SCRIPT" --inner
    rc="$(cat "$TEST_ROOT/inner.rc" 2>/dev/null || echo 1)"
    set -e
    rm -rf "$TEST_ROOT" 2>/dev/null || true
    exit $rc
fi

# shellcheck source=lib.sh
source "$HERE/lib.sh"
SHELL_LOG="$TEST_ROOT/gnome-shell.log"
MENEW_LOG="$TEST_ROOT/menew.log"
UUID="menew-bridge@pururucos.com.br"
OLD_UUID="menew-bridge@menew.web.app"
mkdir -p "$REPO_ROOT/docs/desktop-tests"

cleanup() {
    echo "$?" > "$TEST_ROOT/inner.rc"
    cp "$MENEW_LOG" /tmp/menew-last-welcome.log 2>/dev/null
    "$MENEW_BIN" quit >/dev/null 2>&1 || true
    kill_tracked
    pkill -u "$(id -u)" -f "$TEST_ROOT" 2>/dev/null || true
}
trap cleanup EXIT

[ -x "$MENEW_BIN" ] || { echo "FAIL: menew binary not found at $MENEW_BIN (build it first)"; exit 1; }

export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=ubuntu:GNOME
gnome-shell --wayland --headless --no-x11 --unsafe-mode --virtual-monitor 1280x800 >"$SHELL_LOG" 2>&1 &
track $!
if wait_for 30 bash -c 'for s in "$XDG_RUNTIME_DIR"/wayland-*; do [ -S "$s" ] && exit 0; done; exit 1'; then
    pass "headless gnome-shell started"
else
    fail "headless gnome-shell did not start"; tail -30 "$SHELL_LOG"; exit 1
fi
WAYLAND_DISPLAY="$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")"
export WAYLAND_DISPLAY
wait_for 30 gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.freedesktop.DBus.Peer.Ping >/dev/null || true
sleep 2

shell_eval() { gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" 2>&1 || true; }
window_titles() { shell_eval "global.get_window_actors().map(a => a.meta_window.get_title()).join('|')"; }
screenshot() {
    rm -f "$TEST_ROOT/shot.png"
    gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false "$TEST_ROOT/shot.png" >/dev/null 2>&1 || true
    if [ -s "$TEST_ROOT/shot.png" ]; then cp "$TEST_ROOT/shot.png" "$1"; return 0; fi
    return 1
}

# 1. a new user opens Menew from the app grid (`Exec=menew open`)
check "no data, config or autostart before the first run" bash -c "[ ! -e '$XDG_DATA_HOME/menew' ] && [ ! -e '$XDG_CONFIG_HOME/menew' ]"
RUST_LOG=info "$MENEW_BIN" open >"$MENEW_LOG" 2>&1 &
track $!
if wait_for 20 grep -a -q "Setup: first run" "$MENEW_LOG"; then pass "first run detected"; else fail "no first run in the log"; fi
sleep 4

check "start at login turned on" test -f "$XDG_CONFIG_HOME/autostart/menew.desktop"
if [ "$MENEW_BIN" = /usr/bin/menew ]; then
    # the installed .deb: the extension comes from /usr/share, nothing is copied to the home folder
    check "the .deb's extension is used" test -f "/usr/share/gnome-shell/extensions/$UUID/metadata.json"
    check "no copy of the extension in the home folder" test ! -e "$XDG_DATA_HOME/gnome-shell/extensions/$UUID"
    PROGRAM=menew
else
    check "extension files written for a non-.deb build" test -f "$XDG_DATA_HOME/gnome-shell/extensions/$UUID/metadata.json"
    check "extension.js is the bundled one" cmp -s "$XDG_DATA_HOME/gnome-shell/extensions/$UUID/extension.js" "$REPO_ROOT/gnome-extension/extension.js"
    PROGRAM="$MENEW_BIN"
fi
ENABLED="$(gsettings get org.gnome.shell enabled-extensions)"
info "enabled-extensions = $ENABLED"
if echo "$ENABLED" | grep -q "$UUID"; then pass "extension turned on in enabled-extensions"; else fail "extension not in enabled-extensions"; fi

KEYS="$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)"
info "custom-keybindings = $KEYS"
if echo "$KEYS" | grep -q "custom-keybindings/menew-palette/"; then pass "custom shortcuts created (no GlobalShortcuts portal)"; else fail "no custom shortcuts"; fi
P="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/menew-palette/"
CMD="$(gsettings get "$P" command)"; BIND="$(gsettings get "$P" binding)"; NAME="$(gsettings get "$P" name)"
info "palette shortcut: $NAME | $CMD | $BIND"
if [ "$CMD" = "'$PROGRAM toggle'" ] && [ "$BIND" = "'<Control><Alt>space'" ]; then pass "palette shortcut runs '$PROGRAM toggle' on Ctrl+Alt+Space"; else fail "palette shortcut is $CMD $BIND"; fi
N="$(echo "$KEYS" | grep -o "menew-" | wc -l)"
if [ "$N" -eq 7 ]; then pass "7 Menew shortcuts"; else fail "$N Menew shortcuts, expected 7"; fi

TITLES="$(window_titles)"
info "windows: $TITLES"
if echo "$TITLES" | grep -q "Menew"; then pass "welcome window is open"; else fail "no welcome window"; fi
if screenshot "$REPO_ROOT/docs/desktop-tests/gnome-welcome.png"; then pass "screenshot docs/desktop-tests/gnome-welcome.png ($(distinct_colors "$REPO_ROOT/docs/desktop-tests/gnome-welcome.png") colours)"; else fail "no screenshot"; fi

# 2. the setup page (`menew welcome`)
"$MENEW_BIN" welcome >/dev/null 2>&1 || true
sleep 3
if screenshot "$REPO_ROOT/docs/desktop-tests/gnome-welcome-setup.png"; then pass "screenshot docs/desktop-tests/gnome-welcome-setup.png"; else fail "no setup screenshot"; fi

# 3. closing the welcome is remembered; then `menew open` shows the palette
shell_eval "global.get_window_actors().map(a => a.meta_window).filter(w => w.get_title() == 'Menew').forEach(w => w.delete(global.get_current_time())); 1" >/dev/null
sleep 2
check "closing the welcome is remembered" test -f "$XDG_DATA_HOME/menew/welcome-done"
"$MENEW_BIN" open >/dev/null 2>&1 || true
sleep 2
TITLES="$(window_titles)"
info "windows after open: $TITLES"
if grep -a -q "CLI command line \[\"open\"\] -> Open" "$MENEW_LOG"; then pass "menew open reached the running Menew"; else fail "menew open not handled"; fi
OPEN_WELCOMES="$(grep -a -c "Setup: first run" "$MENEW_LOG")"
if [ "$OPEN_WELCOMES" -eq 1 ]; then pass "the first run happened once"; else fail "first run logged $OPEN_WELCOMES times"; fi

# 4. a second start: an existing user, old extension name replaced, shortcuts not created twice
"$MENEW_BIN" quit >/dev/null 2>&1 || true
sleep 2
gsettings set org.gnome.shell enabled-extensions "['other@x', '$OLD_UUID']"
RUST_LOG=info "$MENEW_BIN" >"$MENEW_LOG.2" 2>&1 &
track $!
sleep 5
ENABLED="$(gsettings get org.gnome.shell enabled-extensions)"
if [ "$ENABLED" = "['other@x', '$UUID']" ]; then pass "old extension name replaced: $ENABLED"; else fail "enabled-extensions after migration: $ENABLED"; fi
if grep -a -q "Setup: first run" "$MENEW_LOG.2"; then fail "second start ran the first run again"; else pass "second start is not a first run"; fi
N2="$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings | grep -o "menew-" | wc -l)"
if [ "$N2" -eq 7 ]; then pass "shortcuts not duplicated"; else fail "$N2 Menew shortcuts after the second start"; fi
TITLES="$(window_titles)"
if echo "$TITLES" | grep -q "Menew"; then fail "a window opened on a background start: $TITLES"; else pass "background start opens no window"; fi

echo
if [ "$FAILS" -eq 0 ]; then echo "ALL PASSED"; else echo "$FAILS FAILED"; fi
exit "$FAILS"
