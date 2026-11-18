#!/usr/bin/env bash
# Phase 10 checks (system monitor: tray metrics, the window, the top processes, quit, root refused).
# Sourced by gnome.sh and kde.sh after lib.sh. Needs from the caller: DE, de_screenshot, TRAY_DEST (or a
# registered tray), and the daemon started with MENEW_TEST=1 and a config.json that chooses the tray
# items (`monitor_tray_items`: cpu, ram, disk), written before the daemon started.

# alive <pid> : the process exists and is not a zombie waiting to be reaped
alive() {
    [ -d "/proc/$1" ] && ! grep -q '^State:.*Z' "/proc/$1/status" 2>/dev/null
}

# top_has <list> <pid> <name> : the last __monitor-log printed that process in that top list
top_has() { log_since "$P10_START" | grep -a -F -q -- "Monitor top $1: pid=$2 name=$3 "; }

phase10_checks() {
    local shot="$REPO_ROOT/docs/desktop-tests/$DE-monitor.png" busy stubborn root_pid dest title tip start rc

    # (a) metrics in the tray: StatusNotifierItem Title (KDE shows it in the panel) and ToolTip, read over D-Bus
    dest="$(tray_item_dest)"
    if [ -z "$dest" ]; then
        fail "no StatusNotifierItem registered: the tray metrics cannot be read"
    else
        if wait_for 15 bash -c "gdbus call --session --dest '$dest' --object-path /StatusNotifierItem --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierItem Title | grep -a -q 'CPU [0-9]*%.*RAM [0-9]*%.*SSD [0-9]*%'"; then
            title="$(tray_prop "$dest" Title)"; tip="$(tray_prop "$dest" ToolTip)"
            info "tray Title -> $title"
            info "tray ToolTip -> $(echo "$tip" | cut -c1-260)"
            pass "tray Title shows the chosen metrics (config monitor_tray_items = cpu, ram, disk): $(echo "$title" | grep -a -o 'Menew[^'"'"']*')"
            if echo "$tip" | grep -a -q 'CPU: [0-9]*%' && echo "$tip" | grep -a -q 'Memory: [0-9]*%' && echo "$tip" | grep -a -q 'Disk: [0-9]*%'; then pass "tray ToolTip has one line per chosen metric (CPU, Memory, Disk)"; else fail "tray ToolTip lacks the metric lines"; fi
        else
            fail "tray Title never showed CPU/RAM/SSD: $(tray_prop "$dest" Title)"
        fi
    fi

    # (b) `menew monitor` opens the window, which samples while it is visible
    start="$(log_lines)"
    "$MENEW_BIN" monitor >/dev/null 2>&1 || true
    if wait_for 10 bash -c "tail -n +$((start + 1)) '$MENEW_LOG' | grep -a -q 'Monitor window presented'"; then pass "menew monitor presented the monitor window (daemon log)"; else fail "no 'Monitor window presented' in the daemon log"; fi
    if wait_for 10 bash -c "tail -n +$((start + 1)) '$MENEW_LOG' | grep -a -q 'Monitor: sampling started'"; then pass "the window started sampling when it became visible"; else fail "no 'Monitor: sampling started'"; fi
    sleep 3
    rm -f "$shot"; de_screenshot "$shot"
    if [ -s "$shot" ] && [ "$(distinct_colors "$shot")" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-monitor.png ($(stat -c %s "$shot") bytes, $(distinct_colors "$shot") colours)"; else fail "no usable monitor screenshot"; fi

    # (c) a process of the user that burns CPU shows up in the top lists; root processes never do
    cp /bin/sh "$TEST_ROOT/menew-busy"
    "$TEST_ROOT/menew-busy" -c 'while :; do :; done' &
    busy=$!
    track "$busy"
    P10_START="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __monitor-log >/dev/null 2>&1 || true
    if wait_for 15 bash -c "tail -n +$((P10_START + 1)) '$MENEW_LOG' | grep -a -q 'Monitor top done'"; then
        info "top lists: $(log_since "$P10_START" | grep -a 'Monitor top' | cut -c12-120 | tr '\n' '|')"
        if top_has cpu "$busy" menew-busy; then pass "the test process (pid $busy, a busy loop) is in the top CPU list"; else fail "pid $busy not in the top CPU list"; fi
        if [ "$(id -u)" = 0 ]; then
            skip "root is the test user here: the 'root processes are never offered' check is NOT run"
        else
            local bad="" p
            for p in $(log_since "$P10_START" | grep -a -o 'Monitor top [a-z]*: pid=[0-9]*' | sed 's/.*pid=//' | sort -u); do
                [ "$(stat -c %u "/proc/$p" 2>/dev/null)" = "$(id -u)" ] || bad="$bad $p"
            done
            if [ -z "$bad" ]; then pass "every listed pid belongs to the current user (uid $(id -u)); pid 1 and the root processes are not listed"; else fail "processes of other users were listed:$bad"; fi
            if log_since "$P10_START" | grep -a -q 'Monitor top [a-z]*: pid=1 '; then fail "pid 1 was listed"; fi
        fi
    else
        fail "the monitor-log hook did not answer"
    fi

    # (d) quit: SIGTERM first, force as a separate explicit step, root refused
    start="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __quit-process "$busy" >/dev/null 2>&1 || true
    if wait_for 10 bash -c "! [ -d /proc/$busy ] || grep -q '^State:.*Z' /proc/$busy/status"; then
        rc=0; wait "$busy" 2>/dev/null || rc=$?
        pass "__quit-process $busy ended the process (exit status $rc; 143 = 128 + SIGTERM)"
        [ "$rc" = 143 ] || fail "expected exit status 143 (SIGTERM), got $rc"
    else
        fail "process $busy is still running after __quit-process"
    fi
    log_since "$start" | grep -a 'Test hook: quit-process' | head -2 | sed 's/^/INFO: /'

    cp /bin/bash "$TEST_ROOT/menew-stubborn"
    "$TEST_ROOT/menew-stubborn" -c 'trap "" TERM; while :; do :; done' &
    stubborn=$!
    track "$stubborn"
    sleep 1
    MENEW_TEST=1 "$MENEW_BIN" __quit-process "$stubborn" >/dev/null 2>&1 || true
    sleep 2
    if alive "$stubborn"; then pass "a process that ignores SIGTERM (pid $stubborn) survived the plain quit"; else fail "the stubborn process died from SIGTERM, the test is inconclusive"; fi
    MENEW_TEST=1 "$MENEW_BIN" __quit-process "$stubborn" force >/dev/null 2>&1 || true
    if wait_for 10 bash -c "! [ -d /proc/$stubborn ] || grep -q '^State:.*Z' /proc/$stubborn/status"; then
        rc=0; wait "$stubborn" 2>/dev/null || rc=$?
        if [ "$rc" = 137 ]; then pass "the explicit force step ended it (exit status 137 = 128 + SIGKILL)"; else fail "force quit exit status $rc, expected 137"; fi
    else
        fail "force quit did not end pid $stubborn"
    fi

    if [ "$(id -u)" = 0 ]; then
        skip "root is the test user here: refusing root processes is NOT run"
    else
        root_pid="$(pgrep -u 0 -n . | head -1 || true)"
        start="$(log_lines)"
        MENEW_TEST=1 "$MENEW_BIN" __quit-process 1 force >/dev/null 2>&1 || true
        sleep 1
        if log_since "$start" | grep -a -q 'quit-process 1 force=true -> refused' && [ -d /proc/1 ]; then pass "quit of pid 1 was refused (and pid 1 is still there)"; else fail "pid 1 was not refused: $(log_since "$start" | grep -a 'quit-process' | head -1)"; fi
        if [ -n "$root_pid" ] && [ "$root_pid" != 1 ]; then
            start="$(log_lines)"
            MENEW_TEST=1 "$MENEW_BIN" __quit-process "$root_pid" >/dev/null 2>&1 || true
            sleep 1
            if log_since "$start" | grep -a -q "quit-process $root_pid force=false -> refused" && [ -d "/proc/$root_pid" ]; then pass "quit of root-owned pid $root_pid ($(cat "/proc/$root_pid/comm")) was refused by Menew itself and it still runs"; else fail "root-owned pid $root_pid was not refused: $(log_since "$start" | grep -a 'quit-process' | head -1)"; fi
        else
            skip "no root-owned process other than pid 1 to try"
        fi
    fi

    # (e) closing the window stops the sampling
    start="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __monitor-hide >/dev/null 2>&1 || true
    if wait_for 10 bash -c "tail -n +$((start + 1)) '$MENEW_LOG' | grep -a -q 'Monitor: sampling stopped'"; then pass "hiding the window stopped its sampling (daemon log)"; else fail "no 'Monitor: sampling stopped' after hiding the window"; fi
}
