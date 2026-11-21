#!/usr/bin/env bash
# Quick actions and window snapping checks (keep awake from the palette, dark mode, systemd-inhibit,
# `menew run win.*` read back from the compositor). Sourced by gnome.sh and kde.sh after lib.sh. Needs
# from the caller: DE, MENEW_LOG, and on GNOME shell_eval (unsafe mode) and the Menew extension
# version 2 (org.menew.Shell), on KDE KWIN_LOG.

# snap_target <seconds> : open a plain GTK window titled "Snap target" for the compositor to move
snap_target() {
    python3 - "$1" >/dev/null 2>&1 <<'PY' &
import sys
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
app = Gtk.Application(application_id="org.example.SnapTarget")
def on_activate(a):
    w = Gtk.ApplicationWindow(application=a, title="Snap target")
    w.set_default_size(400, 300)
    w.present()
    GLib.timeout_add_seconds(int(sys.argv[1]), a.quit)
app.connect("activate", on_activate)
app.run([])
PY
    track $!
    sleep 4
}

# half_ok <x offset> <width> <work width> <left|right> : the window fills that half (4 px slack)
half_ok() {
    local x="$1" w="$2" aw="$3" half
    [ -n "$x" ] && [ -n "$w" ] && [ -n "$aw" ] || return 1
    half=$((aw / 2))
    [ "$w" -ge $((half - 4)) ] && [ "$w" -le $((half + 4)) ] || return 1
    if [ "$4" = left ]; then [ "${x#-}" -le 2 ]; else [ "$x" -ge $((half - 4)) ] && [ "$x" -le $((half + 4)) ]; fi
}

actions_checks() {
    local start before after geo x w aw probe id

    # (a) the palette offers keep awake for a typed time
    start="$(log_lines)"
    "$MENEW_BIN" __palette-query "awake 2h" >/dev/null 2>&1; sleep 2
    if log_since "$start" | grep -a -q 'Keep awake for 2 h'; then pass "palette 'awake 2h' offers keep awake for 2 h"; else fail "no keep awake item for 'awake 2h'"; fi
    "$MENEW_BIN" toggle >/dev/null 2>&1; sleep 1

    # (b) keep awake runs systemd-inhibit (or says it is missing), and stops
    "$MENEW_BIN" run qa.awake:1 >/dev/null 2>&1; sleep 2
    if pgrep -f 'systemd-inhibit --what=idle:sleep --who=Menew' >/dev/null; then
        pass "keep awake holds a systemd-inhibit lock"
    elif grep -a -q 'Notification sent: The system refused to keep the computer awake\|Notification sent: Keep awake needs systemd-inhibit' "$MENEW_LOG"; then
        pass "keep awake could not take the lock here (no logind) and said so"
    else
        fail "keep awake did nothing, or said it was on without a lock"
    fi
    "$MENEW_BIN" run qa.awake >/dev/null 2>&1; sleep 1

    if [ "$DE" = gnome ]; then
        # (c) dark mode flips GNOME's color-scheme, then back
        before="$(gsettings get org.gnome.desktop.interface color-scheme)"
        "$MENEW_BIN" run qa.dark >/dev/null 2>&1; sleep 2
        after="$(gsettings get org.gnome.desktop.interface color-scheme)"
        if [ "$before" != "$after" ]; then pass "qa.dark switched color-scheme ($before -> $after)"; else fail "qa.dark did not change color-scheme ($before)"; fi
        "$MENEW_BIN" run qa.dark >/dev/null 2>&1; sleep 1

        # (d) win.left through the extension's org.menew.Shell
        snap_target 40
        shell_eval "global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === 'Snap target').activate(global.get_current_time()); 1" >/dev/null
        sleep 1
        "$MENEW_BIN" run win.left >/dev/null 2>&1; sleep 2
        geo="$(shell_eval "const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === 'Snap target'); const r = w.get_frame_rect(); const a = w.get_work_area_current_monitor(); [r.x - a.x, r.width, a.width].join(',')" | grep -o -- '-\?[0-9]*,[0-9]*,[0-9]*' | tail -1)"
        info "snap target geometry (x offset, width, work width): ${geo:-none}"
        IFS=, read -r x w aw <<<"${geo:-,,}"
        if half_ok "$x" "$w" "$aw" left; then
            pass "win.left put the window on the left half (through org.menew.Shell)"
        else
            fail "win.left did not snap the window: ${geo:-no geometry}"; grep -a -i 'snap' "$MENEW_LOG" | tail -5
        fi
    else
        # (d) win.right through a KWin script
        snap_target 30
        "$MENEW_BIN" run win.right >/dev/null 2>&1; sleep 3
        probe="$TEST_ROOT/probe.js"
        cat >"$probe" <<'JS'
var list = workspace.windowList ? workspace.windowList() : workspace.clientList();
for (var i = 0; i < list.length; i++) {
    var w = list[i];
    if (w.caption && w.caption.indexOf("Snap target") === 0) {
        var a = workspace.clientArea(KWin.MaximizeArea, w);
        print("MENEW_GEO " + Math.round(w.frameGeometry.x - a.x) + "," + Math.round(w.frameGeometry.width) + "," + Math.round(a.width));
    }
}
JS
        id="$(gdbus call --session --dest org.kde.KWin --object-path /Scripting --method org.kde.kwin.Scripting.loadScript "$probe" menew-probe | grep -o '[0-9]\+' | head -1)"
        gdbus call --session --dest org.kde.KWin --object-path "/Scripting/Script$id" --method org.kde.kwin.Script.run >/dev/null 2>&1 \
          || gdbus call --session --dest org.kde.KWin --object-path "/$id" --method org.kde.kwin.Script.run >/dev/null 2>&1
        sleep 1
        geo="$(grep -a -o 'MENEW_GEO [0-9,-]*' "$KWIN_LOG" | tail -1 | cut -d' ' -f2)"
        info "snap target geometry (x offset, width, work width): ${geo:-none}"
        IFS=, read -r x w aw <<<"${geo:-,,}"
        if half_ok "$x" "$w" "$aw" right; then
            pass "win.right put the window on the right half (KWin scripting)"
        else
            fail "win.right did not snap the window on KWin: ${geo:-no geometry}"; grep -a -i 'snap' "$MENEW_LOG" | tail -5
        fi
    fi

    if grep -a -q 'panicked' "$MENEW_LOG"; then fail "the daemon panicked"; grep -a -A3 panicked "$MENEW_LOG" | head; else pass "no panic in the daemon log"; fi
}

