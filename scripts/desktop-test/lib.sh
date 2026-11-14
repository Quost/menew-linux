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
