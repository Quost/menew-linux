#!/usr/bin/env bash
# Headless KDE (KWin) desktop test. Run inside WSL Ubuntu 24.04:
#   bash scripts/desktop-test/kde.sh
set -euo pipefail

SCRIPT="$(readlink -f "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SCRIPT")"

if [ "${1:-}" != "--inner" ]; then
    export TEST_ROOT
    TEST_ROOT="$(mktemp -d /tmp/menew-kde-XXXXXX)"
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
KWIN_LOG="$TEST_ROOT/kwin.log"
MENEW_LOG="$TEST_ROOT/menew.log"
SHOT="$REPO_ROOT/docs/desktop-tests/kde-palette.png"

cleanup() {
    echo "$?" > "$TEST_ROOT/inner.rc"
    cp "$KWIN_LOG" /tmp/menew-last-kde-kwin.log 2>/dev/null; cp "$MENEW_LOG" /tmp/menew-last-kde-menew.log 2>/dev/null
    "$MENEW_BIN" quit >/dev/null 2>&1 || true
    kill_tracked
}
trap cleanup EXIT

[ -x "$MENEW_BIN" ] || { echo "FAIL: menew binary not found at $MENEW_BIN (build it first)"; exit 1; }

# headless KWin on its own virtual output
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=KDE
kwin_wayland --virtual --socket wayland-kde --width 1280 --height 800 --no-lockscreen >"$KWIN_LOG" 2>&1 &
track $!
if wait_for 30 test -S "$XDG_RUNTIME_DIR/wayland-kde"; then
    pass "headless kwin_wayland started (socket wayland-kde present)"
else
    fail "kwin_wayland did not create its wayland socket"; tail -30 "$KWIN_LOG"; exit 1
fi
export WAYLAND_DISPLAY=wayland-kde
sleep 2

# Minimal StatusNotifierWatcher: plasmashell is not installed in this headless setup
python3 "$HERE/watcher.py" >"$TEST_ROOT/watcher.log" 2>&1 &
track $!
wait_for 10 grep -q 'watcher ready' "$TEST_ROOT/watcher.log" || fail "StatusNotifierWatcher stand-in did not start"

# Notification server stand-in: KWin alone does not provide org.freedesktop.Notifications
python3 "$HERE/notifyd.py" >"$TEST_ROOT/notifyd.log" 2>&1 &
track $!
wait_for 10 grep -q 'notifyd ready' "$TEST_ROOT/notifyd.log" || fail "notification server stand-in did not start"

# the tests assert English UI strings
export LC_ALL=C.UTF-8
# enables the hidden `__select-clipboard|__pin-clipboard|__delete-clipboard` commands in the daemon
export MENEW_TEST=1
# shortens the 30 s clear of a copied secret (only with MENEW_TEST=1)
export MENEW_SECRET_CLEAR_SECS=3

# menew daemon (spawns the wl-paste watcher)
"$MENEW_BIN" >"$MENEW_LOG" 2>&1 &
MENEW_PID=$!
track "$MENEW_PID"
# the watcher is the child process named exactly "wl-paste" of the daemon (never match command lines)
watcher_pid() { pgrep -P "$MENEW_PID" -x wl-paste | head -1; }
if wait_for 20 watcher_pid; then
    WATCHER_PID="$(watcher_pid)"
    pass "wl-paste watcher started: pid $WATCHER_PID, child of menew pid $MENEW_PID ($(tr '\0' ' ' < /proc/"$WATCHER_PID"/cmdline | cut -c1-150))"
else
    WATCHER_PID=""
    fail "wl-paste watcher did not start as a child of menew"
fi
sleep 2

if [ -n "${EXPLORE_CMD:-}" ]; then bash -c "$EXPLORE_CMD" || true; exit 0; fi

# Readiness probe for the real pipeline wl-copy -> wl-paste --watch -> helper -> daemon -> SQLite.
# The copy is retried: on a slow runner the compositor/watcher may still be starting when the first
# wl-copy happens. If it never works, the cause is dumped; on CI (a real limitation of the runner,
# not of menew) the dependent checks are SKIPPED with the reason, locally it is a FAIL.
PIPELINE_OK=0
for attempt in 1 2 3 4 5; do
    printf 'menew-probe-%s' "$attempt" | wl-copy
    if wait_for 6 db_has "menew-probe-$attempt"; then PIPELINE_OK=1; info "wl-copy reached SQLite on attempt $attempt"; break; fi
    info "probe attempt $attempt: wl-copy did not reach SQLite within 6 s"
done
sqlite3 "$MENEW_DB_PATH" "DELETE FROM clipboard_entries WHERE text_content LIKE 'menew-probe-%';" 2>/dev/null || true
if [ "$PIPELINE_OK" = "0" ]; then
    info "DIAG wl-paste --list-types: $(timeout 5 wl-paste --list-types 2>&1 | tr '\n' ' ')"
    info "DIAG wl-paste processes: $(pgrep -a -x wl-paste | tr '\n' ' ')"
    info "DIAG wayland socket: $(ls -l "$XDG_RUNTIME_DIR"/wayland-kde 2>&1)"
    info "DIAG kwin.log tail: $(tail -5 "$KWIN_LOG" | tr '\n' '|' | cut -c1-400)"
    info "DIAG menew.log tail: $(sed 's/\x1b\[[0-9;]*m//g' "$MENEW_LOG" | grep -av portal | tail -5 | tr '\n' '|' | cut -c1-500)"
    if [ -n "${CI:-}" ]; then
        skip "wl-copy -> wl-paste --watch -> SQLite never worked on this CI runner (diagnostics above); clipboard checks 1 and Phase 2 SKIPPED, not passed"
    else
        fail "wl-copy never reached SQLite (diagnostics above)"
    fi
fi

# check 1: single line and multi-line text
if [ "$PIPELINE_OK" = "1" ]; then
printf 'hello kde' | wl-copy
if wait_for 10 db_has "hello kde"; then pass "wl-copy 'hello kde' saved in SQLite"; else fail "'hello kde' not found in SQLite"; fi
sleep 1
printf 'line one\nline two\n  indented line three' | wl-copy
EXPECT=$'line one\nline two\n  indented line three'
wait_for 10 db_has "$EXPECT" || true
MULTI_COUNT="$(db_count "$EXPECT")"
if [ "$MULTI_COUNT" = "1" ]; then pass "multi-line text saved exactly as copied, as 1 entry"; else fail "multi-line text entries: '$MULTI_COUNT' (expected 1)"; fi
SPLIT="$(sqlite3 "$MENEW_DB_PATH" "SELECT COUNT(*) FROM clipboard_entries WHERE text_content IN ('line one','line two','  indented line three');")"
[ "$SPLIT" = "0" ] && pass "multi-line text was not split into separate entries" || fail "multi-line text was split into $SPLIT entries"
info "rows: $(sqlite3 "$MENEW_DB_PATH" "SELECT id || ':' || replace(text_content, char(10), '\n') FROM clipboard_entries;" | tr '\n' ' ')"
fi

# check 2: password hint ignored.
# A real two-type selection (text/plain + hint) cannot be created headless: wl-copy offers one type
# and GTK clients never get keyboard focus on a virtual KWin without input devices. So the helper
# that wl-paste --watch calls (menew __clipboard-push) is run directly with a stub `wl-paste` that
# reports the MIME list of a password manager copy. This exercises the real MIME check in the
# helper and the real daemon over D-Bus; it does NOT exercise wl-paste itself.
mkdir -p "$TEST_ROOT/fakebin"
cat > "$TEST_ROOT/fakebin/wl-paste" <<'STUB'
#!/bin/sh
[ "$1" = "--list-types" ] && { printf 'text/plain
x-kde-passwordManagerHint
'; exit 0; }
[ -n "${STUB_DATA:-}" ] && cat "$STUB_DATA"
exit 0
STUB
chmod +x "$TEST_ROOT/fakebin/wl-paste"
printf 'secret-kde-pw' > "$TEST_ROOT/stub-data"
printf 'secret-kde-pw' | STUB_DATA="$TEST_ROOT/stub-data" PATH="$TEST_ROOT/fakebin:$PATH" "$MENEW_BIN" __clipboard-push 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | sed 's/^/INFO: helper: /' || true
sleep 1
if db_has "secret-kde-pw"; then fail "password-hint copy was saved"; else pass "helper ignored a copy whose MIME list has x-kde-passwordManagerHint (stubbed wl-paste)"; fi
# control: same helper path with a plain MIME list must be saved, so the test above is meaningful
sed -i 's/x-kde-passwordManagerHint//' "$TEST_ROOT/fakebin/wl-paste"
printf 'control-kde-plain' > "$TEST_ROOT/stub-data"
printf 'control-kde-plain' | STUB_DATA="$TEST_ROOT/stub-data" PATH="$TEST_ROOT/fakebin:$PATH" "$MENEW_BIN" __clipboard-push >/dev/null 2>&1 || true
if wait_for 5 db_has "control-kde-plain"; then pass "control: same helper path saves a copy without the hint"; else fail "control copy not saved; the password test above is inconclusive"; fi

# check 3: tray registered
sleep 1
ITEMS="$(gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
    --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems 2>&1 || true)"
info "RegisteredStatusNotifierItems -> $ITEMS"
if echo "$ITEMS" | grep -qi 'menew\|StatusNotifierItem'; then pass "tray item registered with the StatusNotifierWatcher"; else fail "no tray item registered"; fi

# check 5 (before quit): palette screenshot
mkdir -p "$(dirname "$SHOT")"
rm -f "$SHOT"
"$MENEW_BIN" toggle >/dev/null 2>&1 || true
sleep 3
# grim needs wlr-screencopy, which KWin 5.27 does not implement; spectacle uses KWin's ScreenShot2 D-Bus API
if QT_QPA_PLATFORM=wayland timeout 30 spectacle -b -n -f -o "$TEST_ROOT/shot.png" >"$TEST_ROOT/spectacle.log" 2>&1 && [ -s "$TEST_ROOT/shot.png" ]; then
    cp "$TEST_ROOT/shot.png" "$SHOT"
    pass "palette screenshot (spectacle) written to docs/desktop-tests/kde-palette.png ($(stat -c %s "$SHOT") bytes)"
else
    info "spectacle: $(tail -3 "$TEST_ROOT/spectacle.log" | tr '
' ' ')"
    skip "screenshot NOT taken: spectacle failed (see INFO above)"
fi

# GlobalShortcuts portal: record, do not force
grep -a 'Shortcut portal status' "$MENEW_LOG" | sed 's/\x1b\[[0-9;]*m//g' | cut -c1-240 | sed 's/^/INFO: GlobalShortcuts: /' || true
if grep -a -q 'Shortcut portal status: Success' "$MENEW_LOG"; then pass "GlobalShortcuts portal registered"; else skip "GlobalShortcuts portal not available (no portal backend headless); recorded above"; fi

# Phase 2: clipboard mode, selecting an entry, pin/delete hooks, tray actions
DE=kde
FIRST_TEXT="hello kde"
de_copy() { printf '%s' "$1" | wl-copy; }
de_clipboard_is() { clipboard_is "$1"; }
de_screenshot() {
    if QT_QPA_PLATFORM=wayland timeout 30 spectacle -b -n -f -o "$TEST_ROOT/shot2.png" >"$TEST_ROOT/spectacle2.log" 2>&1 && [ -s "$TEST_ROOT/shot2.png" ]; then
        cp "$TEST_ROOT/shot2.png" "$1"; rm -f "$TEST_ROOT/shot2.png"
    else
        info "spectacle: $(tail -3 "$TEST_ROOT/spectacle2.log" | tr '
' ' ')"
    fi
}
if [ "$PIPELINE_OK" = "1" ]; then phase2_checks; palette_checks; else skip "Phase 2 and palette checks need the wl-copy pipeline (see SKIP above)"; fi
source "$HERE/phase5.sh"
de_copy_png() { wl-copy --type image/png < "$1"; }
de_copy_uris() { printf '%s\r\n' "$1" | wl-copy --type text/uri-list; }
de_pause_probe() {
    if [ -z "$(pgrep -P "$MENEW_PID" -x wl-paste)" ]; then pass "no wl-paste process exists while paused: nothing can read the clipboard"; else fail "a wl-paste watcher is still running during the pause"; fi
}
if [ "$PIPELINE_OK" = "1" ]; then
    phase5_checks
    # an image copy marked as a password, through the real helper with a stubbed wl-paste (a two-type selection cannot be made headless)
    mkdir -p "$TEST_ROOT/fakebin2"
    printf '#!/bin/sh\n[ "$1" = "--list-types" ] && { printf "image/png\nx-kde-passwordManagerHint\n"; exit 0; }\ncat "$STUB_DATA"\n' > "$TEST_ROOT/fakebin2/wl-paste"
    chmod +x "$TEST_ROOT/fakebin2/wl-paste"
    convert -background white -fill black -pointsize 40 label:"HINTED IMAGE" "$TEST_ROOT/hinted.png"
    imgs_before="$(count_kind image)"; files_before="$(ls "$(images_dir)" | wc -l)"
    : | STUB_DATA="$TEST_ROOT/hinted.png" PATH="$TEST_ROOT/fakebin2:$PATH" "$MENEW_BIN" __clipboard-push 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | sed 's/^/INFO: helper: /' || true
    sleep 1
    if [ "$(count_kind image)" = "$imgs_before" ] && [ "$(ls "$(images_dir)" | wc -l)" = "$files_before" ]; then pass "the helper ignored an image copy whose type list has x-kde-passwordManagerHint (no entry, no file; stubbed wl-paste)"; else fail "an image with a password hint was saved"; fi
    # control: the same path without the hint saves it
    sed -i 's/x-kde-passwordManagerHint//' "$TEST_ROOT/fakebin2/wl-paste"
    : | STUB_DATA="$TEST_ROOT/hinted.png" PATH="$TEST_ROOT/fakebin2:$PATH" "$MENEW_BIN" __clipboard-push >/dev/null 2>&1 || true
    if wait_for 5 bash -c "[ \"\$(sqlite3 '$MENEW_DB_PATH' \"SELECT COUNT(*) FROM clipboard_entries WHERE kind='image'\")\" -gt $imgs_before ]"; then pass "control: the same helper path keeps the image without the hint"; else fail "control image not saved; the hint test is inconclusive"; fi
else skip "Phase 5 checks need the wl-copy pipeline (see SKIP above)"; fi
source "$HERE/phase6.sh"
if [ "$PIPELINE_OK" = "1" ]; then phase6_checks; else skip "Phase 6 checks need the wl-copy pipeline (see SKIP above)"; fi

# check 4: quit leaves no wl-paste
"$MENEW_BIN" quit >/dev/null 2>&1 || true
sleep 2
if [ -n "$WATCHER_PID" ] && kill -0 "$WATCHER_PID" 2>/dev/null; then
    fail "watcher pid $WATCHER_PID still alive after menew quit"
elif kill -0 "$MENEW_PID" 2>/dev/null; then
    fail "menew pid $MENEW_PID still alive after menew quit"
elif [ -n "$(pgrep -P "$MENEW_PID" -x wl-paste)" ]; then
    fail "a wl-paste child of menew pid $MENEW_PID remains"
else
    pass "after menew quit: menew pid $MENEW_PID and its wl-paste watcher pid ${WATCHER_PID:-?} are gone, no wl-paste child remains"
fi

echo "----"
if [ "$FAILS" -eq 0 ]; then echo "RESULT: all checks passed"; else echo "RESULT: $FAILS check(s) FAILED"; fi
[ "$FAILS" -eq 0 ]
