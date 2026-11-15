#!/usr/bin/env bash
# Release 1 checks on headless KWin, run by kde.sh:
#   EXPLORE_CMD='bash scripts/desktop-test/r1-kde-checks.sh' bash scripts/desktop-test/kde.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"
MENEW_LOG="$TEST_ROOT/menew.log"

# a window to move
python3 - >/dev/null 2>&1 <<'PY' &
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
app = Gtk.Application(application_id="org.example.SnapTarget")
def on_activate(a):
    w = Gtk.ApplicationWindow(application=a, title="Snap target")
    w.set_default_size(400, 300)
    w.present()
    GLib.timeout_add_seconds(30, a.quit)
app.connect("activate", on_activate)
app.run([])
PY
track $!
sleep 4
"$MENEW_BIN" run win.right >/dev/null 2>&1; sleep 3

# ask KWin where the window is: a script prints it to KWin's log
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
GEO="$(grep -a -o 'MENEW_GEO [0-9,-]*' "$KWIN_LOG" | tail -1 | cut -d' ' -f2)"
info "snap target geometry (x offset, width, work width): ${GEO:-none}"
IFS=, read -r x w aw <<<"${GEO:-,,}"
if [ -n "$aw" ] && [ "$x" -ge $((aw / 2 - 4)) ] && [ "$x" -le $((aw / 2 + 4)) ] && [ "$w" -ge $((aw / 2 - 4)) ] && [ "$w" -le $((aw / 2 + 4)) ]; then
    pass "win.right put the window on the right half (KWin scripting)"
else
    fail "win.right did not snap the window on KWin: ${GEO:-no geometry}"; grep -a -i 'snap' "$MENEW_LOG" | tail -5
fi
if grep -a -q 'panicked' "$MENEW_LOG"; then fail "the daemon panicked"; else pass "no panic in the daemon log"; fi
[ "$FAILS" -eq 0 ]
