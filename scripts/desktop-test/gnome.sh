#!/usr/bin/env bash
# Headless GNOME desktop test. Run inside WSL Ubuntu 24.04:
#   bash scripts/desktop-test/gnome.sh
set -euo pipefail

SCRIPT="$(readlink -f "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SCRIPT")"

if [ "${1:-}" != "--inner" ]; then
    # Outer: fresh private session bus, throwaway HOME, then re-run ourselves inside it
    export TEST_ROOT
    TEST_ROOT="$(mktemp -d /tmp/menew-gnome-XXXXXX)"
    export HOME="$TEST_ROOT/home"
    mkdir -p "$HOME"
    export XDG_RUNTIME_DIR="$TEST_ROOT/run"
    mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
    export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" XDG_CACHE_HOME="$HOME/.cache"
    export GSETTINGS_BACKEND=keyfile
    unset WAYLAND_DISPLAY DISPLAY
    set +e
    dbus-run-session -- bash "$SCRIPT" --inner
    # dbus-run-session does not reliably pass on the child's exit status, so the inner script
    # records it in a file (see the EXIT trap in cleanup)
    rc="$(cat "$TEST_ROOT/inner.rc" 2>/dev/null || echo 1)"
    set -e
    fusermount -uz "$XDG_RUNTIME_DIR/doc" 2>/dev/null || true
    rm -rf "$TEST_ROOT" 2>/dev/null || true
    exit $rc
fi

# shellcheck source=lib.sh
source "$HERE/lib.sh"
export MENEW_DB_PATH="$TEST_ROOT/clipboard.db"
SHELL_LOG="$TEST_ROOT/gnome-shell.log"
MENEW_LOG="$TEST_ROOT/menew.log"
UUID="menew-bridge@menew.web.app"
SHOT="$REPO_ROOT/docs/desktop-tests/gnome-palette.png"

cleanup() {
    echo "$?" > "$TEST_ROOT/inner.rc"
    cp "$SHELL_LOG" /tmp/menew-last-gnome-shell.log 2>/dev/null; cp "$MENEW_LOG" /tmp/menew-last-gnome-menew.log 2>/dev/null
    "$MENEW_BIN" quit >/dev/null 2>&1 || true
    kill_tracked
    pkill -u "$(id -u)" -f "$TEST_ROOT" 2>/dev/null || true
}
trap cleanup EXIT

[ -x "$MENEW_BIN" ] || { echo "FAIL: menew binary not found at $MENEW_BIN (build it first)"; exit 1; }

# 1. install extension into the temporary HOME
bash -c "cd '$REPO_ROOT/gnome-extension' && ./install.sh" >/dev/null
gsettings set org.gnome.shell enabled-extensions "['$UUID']"
gsettings set org.gnome.shell disable-user-extensions false

# start the headless shell (unsafe mode allows org.gnome.Shell.Screenshot)
export XDG_SESSION_TYPE=wayland
gnome-shell --wayland --headless --no-x11 --unsafe-mode --virtual-monitor 1280x800 >"$SHELL_LOG" 2>&1 &
track $!

if wait_for 30 bash -c 'for s in "$XDG_RUNTIME_DIR"/wayland-*; do [ -S "$s" ] && exit 0; done; exit 1'; then
    pass "headless gnome-shell started (wayland socket present)"
else
    fail "headless gnome-shell did not create a wayland socket"; tail -30 "$SHELL_LOG"; exit 1
fi
WAYLAND_DISPLAY="$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")"
export WAYLAND_DISPLAY
info "WAYLAND_DISPLAY=$WAYLAND_DISPLAY"

wait_for 30 gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
    --method org.freedesktop.DBus.Peer.Ping || true
sleep 3

# check 1: extension enabled, no errors
# gnome-extensions needs org.gnome.Shell.Extensions, which is not exported by a headless shell
# (see docs/spike-findings.md), so the state is read through org.gnome.Shell.Eval (unsafe mode).
STATE_RAW="$(gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval     "Main.extensionManager.lookup('$UUID') ? Main.extensionManager.lookup('$UUID').state : -1" 2>&1 || true)"
info "Eval state -> $STATE_RAW"
# ExtensionState: 1 = ACTIVE (enabled), 2 = INACTIVE, 3 = ERROR, 4 = OUT_OF_DATE
if echo "$STATE_RAW" | grep -q "true, '1'"; then pass "extension $UUID state is ACTIVE (1) in the running shell"; else fail "extension $UUID is not ACTIVE: $STATE_RAW"; fi
if gnome-extensions list --enabled 2>/dev/null | grep -q "$UUID"; then
    pass "gnome-extensions list --enabled shows the extension"
else
    info "gnome-extensions CLI unavailable headless (org.gnome.Shell.Extensions not exported); enabled-extensions gsettings: $(gsettings get org.gnome.shell enabled-extensions)"
    gsettings get org.gnome.shell enabled-extensions | grep -q "$UUID" && pass "enabled-extensions gsetting contains the extension" || fail "enabled-extensions gsetting lacks the extension"
fi
if grep -qE 'JS ERROR|Extension .*menew.*(error|failed)|MenewExtension\].*(Failed|Error)' "$SHELL_LOG"; then
    fail "errors about the extension in the shell log"; grep -nE 'Menew|JS ERROR' "$SHELL_LOG" | head
else
    pass "no extension errors in the shell log"
fi
grep -n 'MenewExtension' "$SHELL_LOG" | sed 's/^/    log: /' || true

# A plain GNOME session has no StatusNotifierWatcher (that is what the AppIndicator extension
# provides), so the same stand-in used for KDE is started to let the tray register and be tested
python3 "$HERE/watcher.py" >"$TEST_ROOT/watcher.log" 2>&1 &
track $!
wait_for 10 grep -q 'watcher ready' "$TEST_ROOT/watcher.log" || fail "StatusNotifierWatcher stand-in did not start"

# the tests assert English UI strings
export LC_ALL=C.UTF-8
# enables the hidden `__select-clipboard|__pin-clipboard|__delete-clipboard` commands in the daemon
export MENEW_TEST=1
# shortens the 30 s clear of a copied secret (only with MENEW_TEST=1)
export MENEW_SECRET_CLEAR_SECS=3

# Phase 10: the tray shows CPU, memory and disk (the choice lives in config.json until the settings window of Phase 11)
mkdir -p "$XDG_CONFIG_HOME/menew"
printf '{"monitor_tray_items": ["cpu", "ram", "disk"]}
' > "$XDG_CONFIG_HOME/menew/config.json"

# start menew (daemon)
"$MENEW_BIN" >"$MENEW_LOG" 2>&1 &
track $!

# check 2: D-Bus service
if wait_for 20 gdbus call --session --dest org.menew.ClipboardService --object-path /org/menew/Clipboard \
    --method org.menew.ClipboardService.Ping; then
    PONG="$(gdbus call --session --dest org.menew.ClipboardService --object-path /org/menew/Clipboard --method org.menew.ClipboardService.Ping)"
    info "Ping -> $PONG"
    echo "$PONG" | grep -q pong && pass "org.menew.ClipboardService answers Ping with pong" || fail "Ping did not return pong"
else
    fail "org.menew.ClipboardService not on the bus"
fi
sleep 2   # let the extension proxy reconnect

# developer aid: EXPLORE_CMD='...' runs an arbitrary command in the live session and stops
if [ -n "${EXPLORE_CMD:-}" ]; then bash -c "$EXPLORE_CMD" || true; exit 0; fi

# Copies are made INSIDE the shell (org.gnome.Shell.Eval, unsafe mode) instead of with wl-copy:
# wl-copy/wl-paste need keyboard focus for their helper surface, and a headless gnome-shell has
# no input device ("This seat has no keyboard") so they never get focus and hang. Setting the
# selection from inside the shell fires the same Meta.Selection owner-changed signal the
# extension listens to for a real application copy.
shell_eval() {
    gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell         --method org.gnome.Shell.Eval "$1"
}
shell_copy_text() {
    shell_eval "const St = imports.gi.St; St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, '$1'); 1"
}

# check 3: copy reaches SQLite through extension -> D-Bus -> daemon
info "wl-copy: NOT usable headless (no keyboard on the seat); using in-shell St.Clipboard.set_text"
shell_copy_text 'hello gnome' >/dev/null
if wait_for 10 db_has "hello gnome"; then pass "copy 'hello gnome' saved in SQLite (extension -> D-Bus -> daemon)"; else fail "copy 'hello gnome' not found in SQLite"; fi

# check 4: password hint ignored. The in-shell selection source is hint-only (GJS cannot subclass
# Meta.SelectionSource), so absence from SQLite alone is weak; we also require the extension to
# log that it took its hint branch.
shell_eval "const {Meta} = imports.gi; global.display.get_selection().set_owner(Meta.SelectionType.SELECTION_CLIPBOARD, Meta.SelectionSourceMemory.new('x-kde-passwordManagerHint', new imports.gi.GLib.Bytes(new TextEncoder().encode('secret-gnome-pw')))); 1" >/dev/null
sleep 3
if grep -q 'Ignoring clipboard content marked secret' "$SHELL_LOG"; then pass "extension logged it ignored a copy carrying x-kde-passwordManagerHint"; else fail "extension did not log the password-hint branch"; fi
if db_has "secret-gnome-pw"; then fail "password-hint copy was saved"; else pass "password-hint copy not in SQLite"; fi

# check 5: palette screenshot
mkdir -p "$(dirname "$SHOT")"
rm -f "$SHOT" "$TEST_ROOT/shot.png"
shell_eval "Main.overview.hide(); Main.messageTray.close(); 1" >/dev/null || true   # headless shell starts in the Overview
sleep 2
"$MENEW_BIN" toggle >/dev/null 2>&1 || true
sleep 3
SHOT_RES="$(gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot     --method org.gnome.Shell.Screenshot.Screenshot false false "$TEST_ROOT/shot.png" 2>&1 || true)"
info "Screenshot -> $SHOT_RES"
if [ -s "$TEST_ROOT/shot.png" ]; then
    cp "$TEST_ROOT/shot.png" "$SHOT"
    pass "palette screenshot written to docs/desktop-tests/gnome-palette.png ($(stat -c %s "$SHOT") bytes)"
    # a bare desktop background is (almost) one colour; a visible palette adds many
    COLORS="$(python3 - "$SHOT" <<'PY'
import sys
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
d = p.get_pixels(); n = p.get_n_channels()
print(len({d[i:i+3] for i in range(0, len(d), n * 7)}))
PY
)"
    info "distinct sampled colours in screenshot: $COLORS"
    if [ "${COLORS:-0}" -gt 20 ]; then pass "screenshot is not a blank background ($COLORS colours)"; else fail "screenshot looks blank ($COLORS colours)"; fi
else
    fail "no screenshot produced"
fi

# GlobalShortcuts portal: record, do not force
if grep -a -q 'Shortcut portal status' "$MENEW_LOG"; then
    info "GlobalShortcuts: $(grep -a 'Shortcut portal status' "$MENEW_LOG" | sed 's/\x1b\[[0-9;]*m//g' | cut -c1-220)"
fi
if grep -a -q 'Shortcut portal status: Success\|Shortcut portal status: Active' "$MENEW_LOG"; then pass "GlobalShortcuts portal registered"; else skip "GlobalShortcuts portal not available on this headless GNOME (result recorded above, not a failure)"; fi
if grep -a -q 'Could not start tray' "$MENEW_LOG"; then fail "tray did not start even with the StatusNotifierWatcher stand-in"; else pass "tray registered with the StatusNotifierWatcher stand-in (a plain GNOME session needs the AppIndicator extension for this)"; fi

# Phase 2: clipboard mode, selecting an entry, pin/delete hooks, tray actions
DE=gnome
FIRST_TEXT="hello gnome"
de_copy() { shell_copy_text "$1" >/dev/null; }
de_clipboard_is() {
    # St.Clipboard.get_text is asynchronous: store the result in a global, then read it back
    shell_eval "globalThis.__menewClip = null; const St = imports.gi.St; St.Clipboard.get_default().get_text(St.ClipboardType.CLIPBOARD, (c, t) => { globalThis.__menewClip = t; }); 1" >/dev/null 2>&1
    sleep 1
    local out; out="$(shell_eval "globalThis.__menewClip" 2>/dev/null)"
    [ "$out" = "(true, '\"$1\"')" ] || [ "$out" = "(true, '$1')" ]
}
de_screenshot() {
    local res
    res="$(gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot         --method org.gnome.Shell.Screenshot.Screenshot false false "$TEST_ROOT/shot2.png" 2>&1 || true)"
    if [ -s "$TEST_ROOT/shot2.png" ]; then cp "$TEST_ROOT/shot2.png" "$1"; rm -f "$TEST_ROOT/shot2.png"; else info "Screenshot -> $res"; fi
}
phase2_checks
palette_checks
source "$HERE/phase5.sh"
de_copy_png() { shell_eval "const {GLib, St} = imports.gi; St.Clipboard.get_default().set_content(St.ClipboardType.CLIPBOARD, 'image/png', new GLib.Bytes(GLib.file_get_contents('$1')[1])); 1" >/dev/null; }
de_copy_uris() { shell_eval "const {GLib, St} = imports.gi; St.Clipboard.get_default().set_content(St.ClipboardType.CLIPBOARD, 'text/uri-list', new GLib.Bytes(new TextEncoder().encode('$1' + String.fromCharCode(13, 10)))); 1" >/dev/null; }
de_pause_probe() {
    if grep -a -q 'Capture paused or app ignored: not reading the clipboard' "$SHELL_LOG"; then pass "the extension did not read the clipboard while paused (shell log: 'not reading the clipboard')"; else fail "no 'not reading the clipboard' line in the shell log"; fi
    if grep -a -q 'Capture paused: the clipboard is not read' "$MENEW_LOG"; then pass "the daemon answered ShouldCapture=false while paused (daemon log)"; else fail "no 'Capture paused' line in the daemon log"; fi
}
phase5_checks
source "$HERE/phase6.sh"
phase6_checks
source "$HERE/phase10.sh"
phase10_checks

echo "----"
if [ "$FAILS" -eq 0 ]; then echo "RESULT: all checks passed"; else echo "RESULT: $FAILS check(s) FAILED"; fi
[ "$FAILS" -eq 0 ]
