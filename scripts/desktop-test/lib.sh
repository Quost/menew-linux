#!/usr/bin/env bash
# Shared helpers for the headless desktop tests. Sourced, not executed.

REAL_HOME="$(getent passwd "$(id -u)" | cut -d: -f6)"
MENEW_BIN="${MENEW_BIN:-$REAL_HOME/target-menew/debug/menew}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FAILS=0
PIDS=()

pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
skip() { echo "SKIP: $*"; }
info() { echo "INFO: $*"; }

# check "<description>" <command...> : PASS if the command succeeds
check() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then pass "$desc"; else fail "$desc"; fi
}

# wait_for <seconds> <command...> : poll until the command succeeds
wait_for() {
    local secs="$1"; shift
    local i
    for ((i = 0; i < secs * 4; i++)); do
        if "$@" >/dev/null 2>&1; then return 0; fi
        sleep 0.25
    done
    return 1
}

track() { PIDS+=("$1"); }

kill_tracked() {
    local p
    for p in "${PIDS[@]:-}"; do
        [ -n "$p" ] && kill "$p" 2>/dev/null || true
    done
    sleep 1
    for p in "${PIDS[@]:-}"; do
        [ -n "$p" ] && kill -9 "$p" 2>/dev/null || true
    done
}

db_count() { sqlite3 "$MENEW_DB_PATH" "SELECT COUNT(*) FROM clipboard_entries WHERE text_content = '$1';" 2>/dev/null; }
db_has() { [ "$(db_count "$1")" -ge 1 ] 2>/dev/null; }

# ---- Phase 2 helpers ----

# distinct_colors <png> : number of distinct colours (ImageMagick), 0 if it cannot be read
distinct_colors() { identify -format %k "$1" 2>/dev/null || echo 0; }

# clipboard_text : current clipboard text through wl-paste (data-control, no focus needed)
clipboard_text() { timeout 5 wl-paste --no-newline 2>/dev/null; }
clipboard_is() { [ "$(clipboard_text)" = "$1" ]; }

# db_pinned <text> : is_pinned flag of the entry with exactly that text
db_pinned() { sqlite3 "$MENEW_DB_PATH" "SELECT is_pinned FROM clipboard_entries WHERE text_content = '$1' ORDER BY id DESC LIMIT 1;" 2>/dev/null; }

# db_order : display order of the history (pinned first, then newest), one entry per line
db_order() { sqlite3 "$MENEW_DB_PATH" "SELECT replace(text_content, char(10), ' ') FROM clipboard_entries ORDER BY is_pinned DESC, created_at DESC, id DESC;" 2>/dev/null; }

# tray_item_dest : bus name of the first registered StatusNotifierItem (ksni owns org.kde.StatusNotifierItem-<pid>-<n>)
tray_item_dest() {
    local out
    out="$(gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
        --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems 2>/dev/null)" || return 0
    echo "$out" | grep -o 'org\.kde\.StatusNotifierItem-[0-9]*-[0-9]*' | head -1 || true
}

# tray_menu_find <dest> <label> : prints "<dbusmenu id> <toggle-state or ->" for the item with that label
tray_menu_find() {
    gdbus call --session --dest "$1" --object-path /MenuBar \
        --method com.canonical.dbusmenu.GetLayout 0 10 '[]' 2>/dev/null \
        | python3 -c '
import re, sys
label = sys.argv[1]
text = sys.stdin.read()
for m in re.finditer(r"\((\d+), \{([^{}]*)\}", text):
    props = m.group(2)
    if "\x27label\x27: <\x27" + label + "\x27>" in props:
        ts = re.search(r"\x27toggle-state\x27: <(?:int32 )?(\d+)>", props)
        print(m.group(1), ts.group(1) if ts else "-")
        break
else:
    sys.exit(1)
' "$2"
}

# tray_menu_click <dest> <id> : activate a dbusmenu item like a click on the tray menu
tray_menu_click() {
    gdbus call --session --dest "$1" --object-path /MenuBar \
        --method com.canonical.dbusmenu.Event "$2" clicked "<int32 0>" 0
}

# phase2_checks : clipboard mode screenshot, selecting an entry, pin/delete hooks, tray actions.
# The calling script defines: DE (kde|gnome), FIRST_TEXT (an entry already in the history),
# de_copy <text>, de_clipboard_is <text>, de_screenshot <png>.
phase2_checks() {
    local shot="$REPO_ROOT/docs/desktop-tests/$DE-clipboard.png"
    local t

    # fake data only: temporary database, invented strings
    local fake_url='https://example.com/docs/getting-started'
    local fake_ssh='ssh deploy@staging.example.com'
    local target="$DE-select-target"
    for t in "$fake_url" "$fake_ssh" "$target" "$DE-newest"; do
        de_copy "$t"
        wait_for 10 db_has "$t" || fail "fake entry '$t' not saved"
        sleep 1
    done
    # varied ages and one pinned entry for the screenshot
    sqlite3 "$MENEW_DB_PATH" "UPDATE clipboard_entries SET created_at = created_at - 300000 WHERE text_content = '$fake_url';
        UPDATE clipboard_entries SET created_at = created_at - 7200000 WHERE text_content = '$fake_ssh';
        UPDATE clipboard_entries SET is_pinned = 1 WHERE text_content = '$FIRST_TEXT';"
    info "history rows: $(sqlite3 "$MENEW_DB_PATH" "SELECT count(*) FROM clipboard_entries;")"

    # (a) palette in clipboard mode
    "$MENEW_BIN" toggle >/dev/null 2>&1 || true     # the Apps palette is still open from the screenshot check
    sleep 1
    rm -f "$shot"
    "$MENEW_BIN" clipboard >/dev/null 2>&1 || true
    sleep 3
    if grep -a -q 'presented and focused (Clipboard)' "$MENEW_LOG"; then pass "menew clipboard opened the palette in clipboard mode (daemon log)"; else fail "no 'presented and focused (Clipboard)' in the daemon log"; fi
    de_screenshot "$shot"
    if [ -s "$shot" ]; then
        local colors; colors="$(distinct_colors "$shot")"
        if [ "${colors:-0}" -gt 20 ]; then pass "clipboard screenshot written to docs/desktop-tests/$DE-clipboard.png ($(stat -c %s "$shot") bytes, $colors colours)"; else fail "clipboard screenshot looks blank ($colors colours)"; fi
    else
        fail "no clipboard screenshot produced"
    fi

    # (b) selecting an entry puts it on the clipboard. The palette stays open: a Wayland client can
    # only set the selection while it has a mapped window (a hidden-palette select is rejected by
    # the compositor in these headless sessions), which is also how Enter works in real use.
    # After "before-select" is captured the list is: pinned FIRST_TEXT, before-select, DE-newest, ...
    # so index 2 must be DE-newest (skips the pinned entry and the newest one).
    de_copy "$DE-before-select"
    wait_for 10 db_has "$DE-before-select" || true
    sleep 1
    de_clipboard_is "$DE-before-select" || info "clipboard not yet '$DE-before-select'"
    # GLib picks org.gtk.Notifications when the shell offers it (GNOME), else org.freedesktop.Notifications
    dbus-monitor --session "interface='org.freedesktop.Notifications',member='Notify'"         "interface='org.gtk.Notifications',member='AddNotification'" >"$TEST_ROOT/notify-monitor.log" 2>&1 &
    track $!
    sleep 1
    local want="$DE-newest"
    local rows_before; rows_before="$(db_order | wc -l)"
    info "history order (top 4): $(db_order | head -4 | tr '\n' '|')"
    MENEW_TEST=1 "$MENEW_BIN" __select-clipboard 2 >/dev/null 2>&1 || true
    if wait_for 10 de_clipboard_is "$want"; then pass "MENEW_TEST=1 menew __select-clipboard 2 put '$want' on the clipboard"; else fail "clipboard does not hold '$want' after __select-clipboard 2"; fi
    # the watcher / extension re-captures the selected text: it must bump the existing row, not add a twin
    sleep 3
    local rows_after; rows_after="$(db_order | wc -l)"
    if [ "$rows_after" = "$rows_before" ]; then pass "re-copying an older entry did not add a row ($rows_before -> $rows_after)"; else fail "row count grew after selecting an older entry ($rows_before -> $rows_after)"; fi
    if [ "$(db_count "$want")" = "1" ]; then pass "'$want' still has exactly one row"; else fail "'$want' has $(db_count "$want") rows"; fi
    if [ "$(db_order | sed -n '2p')" = "$want" ]; then pass "selected entry moved to the top of the unpinned entries"; else fail "selected entry is not first after the pinned one: $(db_order | head -3 | tr '\n' '|')"; fi
    if grep -a -q 'Test hook: Select clipboard entry 2 -> found=true' "$MENEW_LOG"; then pass "daemon log: select hook found entry 2"; else fail "select hook did not find entry 2"; fi
    if [ -w /dev/uinput ]; then
        info "/dev/uinput is writable: expecting the real Ctrl+V injection path"
        if wait_for 10 grep -a -q 'Injected Ctrl+V through uinput' "$MENEW_LOG"; then pass "uinput Ctrl+V injection ran (daemon log)"; else fail "uinput writable but no injection logged"; fi
    else
        info "/dev/uinput not writable here: $(ls -l /dev/uinput 2>&1 | cut -c1-60); expecting the notification fallback"
        sleep 2
        if grep -a -q 'Copied. Press Ctrl+V to paste' "$TEST_ROOT/notify-monitor.log"; then pass "fallback notification 'Copied. Press Ctrl+V to paste' sent on the session bus"; else fail "no fallback notification seen on the bus"; tail -5 "$TEST_ROOT/notify-monitor.log"; fi
    fi

    # Ctrl+P / Ctrl+Delete code paths, through the hidden test hooks (no key events headless)
    local pin_text="$fake_ssh" del_text="$fake_url" before after pos
    pos="$(db_order | grep -nxF -- "$pin_text" | head -1 | cut -d: -f1)"; pos=$((pos - 1))
    MENEW_TEST=1 "$MENEW_BIN" __pin-clipboard "$pos" >/dev/null 2>&1 || true
    if [ "$(db_pinned "$pin_text")" = "1" ]; then pass "__pin-clipboard $pos pinned '$pin_text' (is_pinned=1)"; else fail "entry '$pin_text' not pinned (is_pinned=$(db_pinned "$pin_text"))"; fi
    if [ "$(db_order | sed -n '1,2p' | grep -cxF -e "$pin_text" -e "$FIRST_TEXT")" = "2" ]; then pass "both pinned entries are listed first"; else fail "pinned entries are not first: $(db_order | head -3 | tr '\n' '|')"; fi
    pos="$(db_order | grep -nxF -- "$pin_text" | head -1 | cut -d: -f1)"; pos=$((pos - 1))
    MENEW_TEST=1 "$MENEW_BIN" __pin-clipboard "$pos" >/dev/null 2>&1 || true
    if [ "$(db_pinned "$pin_text")" = "0" ]; then pass "second __pin-clipboard unpinned it again"; else fail "entry still pinned after the toggle"; fi
    before="$(db_order | wc -l)"
    pos="$(db_order | grep -nxF -- "$del_text" | head -1 | cut -d: -f1)"; pos=$((pos - 1))
    MENEW_TEST=1 "$MENEW_BIN" __delete-clipboard "$pos" >/dev/null 2>&1 || true
    after="$(db_order | wc -l)"
    if ! db_has "$del_text" && [ "$after" = "$((before - 1))" ]; then pass "__delete-clipboard $pos removed '$del_text' only ($before -> $after entries)"; else fail "delete hook result unexpected ($before -> $after entries)"; fi

    # (c) tray "Start at login" creates and removes the autostart file in the temporary HOME
    local af="$XDG_CONFIG_HOME/autostart/menew.desktop" dest found id state
    case "$af" in "$TEST_ROOT"/*) ;; *) fail "refusing to touch $af: not under the temporary HOME"; return;; esac
    if [ ! -e "$af" ]; then pass "autostart is off by default (no $af)"; else fail "autostart file exists before any click"; fi
    dest="$(tray_item_dest)"
    if [ -z "$dest" ]; then fail "no tray item registered, cannot test the tray menu"; return; fi
    found="$(tray_menu_find "$dest" 'Start at login')" || found=""
    info "tray item $dest, dbusmenu entry for 'Start at login': '$found'"
    if [ -z "$found" ]; then fail "tray menu has no 'Start at login' item"; return; fi
    id="${found%% *}"
    for t in 'Open Command Palette' 'Clipboard History' 'Clear history' 'Start at login' 'Quit Menew'; do
        if tray_menu_find "$dest" "$t" >/dev/null; then pass "tray menu has '$t'"; else fail "tray menu lacks '$t'"; fi
    done
    tray_menu_click "$dest" "$id" >/dev/null 2>&1 || true
    if wait_for 5 test -f "$af" && grep -qx 'Exec=menew' "$af"; then pass "clicking 'Start at login' created $af with Exec=menew"; else fail "autostart file not created"; fi
    state="$(tray_menu_find "$dest" 'Start at login' | cut -d' ' -f2)"
    if [ "$state" = "1" ]; then pass "menu item is now checked (toggle-state=1)"; else fail "menu item toggle-state is '$state' after enabling"; fi
    tray_menu_click "$dest" "$id" >/dev/null 2>&1 || true
    if wait_for 5 test ! -e "$af"; then pass "clicking again removed the autostart file"; else fail "autostart file still present"; fi
    state="$(tray_menu_find "$dest" 'Start at login' | cut -d' ' -f2)"
    if [ "$state" = "0" ]; then pass "menu item unchecked again (toggle-state=0)"; else fail "menu item toggle-state is '$state' after disabling"; fi

    # tray "Clear history" removes the unpinned entries and keeps the pinned one
    local clear_id; clear_id="$(tray_menu_find "$dest" 'Clear history' | cut -d' ' -f1)"
    tray_menu_click "$dest" "$clear_id" >/dev/null 2>&1 || true
    unpinned_zero() { [ "$(sqlite3 "$MENEW_DB_PATH" 'SELECT COUNT(*) FROM clipboard_entries WHERE is_pinned = 0;')" = "0" ]; }
    if wait_for 5 unpinned_zero; then pass "tray 'Clear history' removed every unpinned entry"; else fail "unpinned entries remain after 'Clear history'"; fi
    if db_has "$FIRST_TEXT" && [ "$(db_pinned "$FIRST_TEXT")" = "1" ]; then pass "pinned entry '$FIRST_TEXT' survived 'Clear history'"; else fail "pinned entry was removed by 'Clear history'"; fi
}

# ---- Phase 4: palette results (units, text transforms, emoji, currency, timers, focus) ----

# log_since <line> : the daemon log after that line, without colour codes
log_since() { tail -n +"$(($1 + 1))" "$MENEW_LOG" | sed 's/\x1b\[[0-9;]*m//g'; }
log_lines() { wc -l < "$MENEW_LOG"; }

# palette_query <text> : type the text in the palette (MENEW_TEST=1 hook) and wait until the results are logged
PQ_START=0
palette_query() {
    PQ_START="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __palette-query "$1" >/dev/null 2>&1 || true
    wait_for 10 bash -c "tail -n +$((PQ_START + 1)) '$MENEW_LOG' | grep -a -q 'Test hook: palette query'"
}
# palette_has <text> : the results of the last query contain that text
palette_has() { log_since "$PQ_START" | grep -a -F -q -- "$1"; }
palette_show() { log_since "$PQ_START" | grep -a 'Test hook' | head -5; }

# tray_prop <dest> <property> : a StatusNotifierItem property over D-Bus
tray_prop() {
    gdbus call --session --dest "$1" --object-path /StatusNotifierItem \
        --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierItem "$2" 2>&1
}
tray_title_has() { tray_prop "$TRAY_DEST" Title | grep -a -q -- "$1"; }

# palette_shot <case> : screenshot of the palette as it is now, docs/desktop-tests/<de>-palette-<case>.png
palette_shot() {
    local png="$REPO_ROOT/docs/desktop-tests/$DE-palette-$1.png" colors
    sleep 2
    rm -f "$png"
    de_screenshot "$png"
    if [ -s "$png" ]; then
        colors="$(distinct_colors "$png")"
        if [ "${colors:-0}" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-palette-$1.png ($(stat -c %s "$png") bytes, $colors colours)"; else fail "the $1 screenshot looks blank ($colors colours)"; fi
    else
        fail "no screenshot for the $1 case"
    fi
}

# palette_checks : the results the palette shows for typed text, with a screenshot of two of them.
# The calling script defines DE (kde|gnome), de_copy <text> and de_screenshot <png>.
palette_checks() {
    # 1. units: "5 km to mi" -> 3.1069 mi
    if palette_query "5 km to mi"; then
        if palette_has "result[0] [Calculator] 3.1069 mi | 5 km | copy:3.1069 mi"; then pass "palette '5 km to mi' shows '3.1069 mi' (detail '5 km') first"; else fail "palette '5 km to mi' did not show 3.1069 mi first"; palette_show; fi
    else
        fail "palette '5 km to mi': no results logged (the __palette-query hook did not answer)"
    fi
    palette_shot units

    # 2. text transform of the copied text: snake_case
    de_copy "Hello Palette World"
    wait_for 10 db_has "Hello Palette World" || fail "copy 'Hello Palette World' not saved"
    sleep 1
    palette_query "snake_case" || fail "palette 'snake_case': no results logged"
    if palette_has "result[0] [Action] snake_case | hello_palette_world | transform:snake"; then pass "palette 'snake_case' previews 'hello_palette_world' for the copied text"; else fail "palette 'snake_case' did not preview hello_palette_world"; palette_show; fi

    # 3. emoji: "emoji rocket"
    palette_query "emoji rocket" || fail "palette 'emoji rocket': no results logged"
    if palette_has "result[0] [Calculator] Rocket | ↩ pastes into the front app | paste:🚀"; then pass "palette 'emoji rocket' shows the rocket emoji first"; else fail "palette 'emoji rocket' did not show the rocket first"; palette_show; fi
    palette_shot emoji

    # 4. no request goes out while typing a currency conversion (the rate is fetched only on Enter)
    palette_query "100 usd to brl" || fail "palette '100 usd to brl': no results logged"
    if palette_has "result[0] [Calculator] Convert \$100.00 to BRL | ↩ fetches today's rate (frankfurter.app) | fetch-rate:100 USD->BRL"; then pass "palette '100 usd to brl' offers the fetch and shows no rate yet"; else fail "currency item not as expected"; palette_show; fi
    if log_since "$PQ_START" | grep -a -q 'Currency: fetching'; then fail "a currency request was made while typing"; else pass "no currency request was made while typing (no 'Currency: fetching' in the daemon log)"; fi

    # 5. timer: the countdown is in the tray (StatusNotifierItem Title and ToolTip)
    TRAY_DEST="$(tray_item_dest)"
    palette_query "timer 1m test" || fail "palette 'timer 1m test': no results logged"
    if palette_has "result[0] [Calculator] 1 min timer: test | "; then pass "palette 'timer 1m test' offers '1 min timer: test'"; else fail "timer item not as expected"; palette_show; fi
    MENEW_TEST=1 "$MENEW_BIN" __palette-activate 0 >/dev/null 2>&1 || true
    if [ -z "$TRAY_DEST" ]; then
        fail "no tray item registered: cannot read the countdown"
    elif wait_for 10 tray_title_has "TIMER"; then
        info "tray Title -> $(tray_prop "$TRAY_DEST" Title)"
        info "tray ToolTip -> $(tray_prop "$TRAY_DEST" ToolTip | cut -c1-200)"
        pass "tray Title shows the countdown: $(tray_prop "$TRAY_DEST" Title | grep -a -o "Menew[^']*")"
        if tray_prop "$TRAY_DEST" ToolTip | grep -a -q 'TIMER .*: test'; then pass "tray ToolTip lists the timer with its label"; else fail "tray ToolTip does not list the timer"; fi
    else
        fail "tray Title did not show the timer countdown: $(tray_prop "$TRAY_DEST" Title)"
    fi

    # 6. focus: "focus 1" adds the focus countdown to the same tray title
    palette_query "focus 1" || fail "palette 'focus 1': no results logged"
    if palette_has "result[0] [Action] Focus for 1 min | "; then pass "palette 'focus 1' offers 'Focus for 1 min'"; else fail "focus item not as expected"; palette_show; fi
    MENEW_TEST=1 "$MENEW_BIN" __palette-activate 0 >/dev/null 2>&1 || true
    if [ -n "$TRAY_DEST" ] && wait_for 10 tray_title_has "FOCUS"; then pass "tray Title shows the focus countdown: $(tray_prop "$TRAY_DEST" Title | grep -a -o "Menew[^']*")"; else fail "tray Title did not show FOCUS"; fi

    # 7. a timer that ends: notification and sound (a 3 second timer)
    local before; before="$(log_lines)"
    palette_query "timer 3s done" || fail "palette 'timer 3s done': no results logged"
    MENEW_TEST=1 "$MENEW_BIN" __palette-activate 0 >/dev/null 2>&1 || true
    if wait_for 15 bash -c "tail -n +$((before + 1)) '$MENEW_LOG' | grep -a -q \"Notification sent: Timer done: done / Time's up.\""; then
        pass "the 3 second timer ended: notification 'Timer done: done / Time's up.' sent"
    else
        fail "no end-of-timer notification in the daemon log"
    fi
    if log_since "$before" | grep -a -q 'Sound: '; then pass "sound played: $(log_since "$before" | grep -a -o 'Sound: .*' | head -1)"; else fail "no sound line in the daemon log"; fi
}
