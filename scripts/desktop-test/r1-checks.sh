#!/usr/bin/env bash
# Release 1 checks, run inside a live headless GNOME session by gnome.sh:
#   EXPLORE_CMD='bash scripts/desktop-test/r1-checks.sh' bash scripts/desktop-test/gnome.sh
# Screenshots (fake data only) land in docs/desktop-tests/gnome-r1-*.png.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"
MENEW_LOG="$TEST_ROOT/menew.log"
OUT="$REPO_ROOT/docs/desktop-tests"
mkdir -p "$OUT"

shell_eval() { gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1"; }
shot() { # shot <name> : screenshot of the whole headless screen
    shell_eval "Main.messageTray.close(); Main.messageTray._bannerBin && Main.messageTray._bannerBin.hide(); 1" >/dev/null
    rm -f "$TEST_ROOT/r1.png"
    gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false "$TEST_ROOT/r1.png" >/dev/null 2>&1
    if [ -s "$TEST_ROOT/r1.png" ]; then cp "$TEST_ROOT/r1.png" "$OUT/gnome-r1-$1.png"; pass "screenshot gnome-r1-$1.png"; else fail "no screenshot for $1"; fi
}
log_has() { grep -a -q "$1" "$MENEW_LOG"; }
close_windows() { shell_eval "global.get_window_actors().forEach(a => { const w = a.meta_window; if (w.get_gtk_application_id && w.get_gtk_application_id() === 'app.menew.Menew') w.delete(global.get_current_time()); }); 1" >/dev/null; }

shell_eval "Main.overview.hide(); 1" >/dev/null
"$MENEW_BIN" toggle >/dev/null 2>&1; sleep 1; "$MENEW_BIN" toggle >/dev/null 2>&1   # palette closed

# ---- notes: fake data written while running, the section reloads it ----
NOTES="$XDG_DATA_HOME/menew/notes.json"
mkdir -p "$(dirname "$NOTES")"
cat >"$NOTES" <<'JSON'
[
 {"id":"a1","text":"# Menew ideas\n- [x] Snippets\n- [ ] Per-app shortcuts\n- [ ] Weather widget\n\n**Bold** and `code` and *italic*\n\n#ideas #menew","secure":false,"created":1794600000,"updated":1794657000},
 {"id":"a2","text":"Friday meeting\n**Agenda:** budget and deadlines\n> Bring the quarterly numbers\n\n#work","secure":false,"created":1794500000,"updated":1794650000},
 {"id":"a3","text":"Groceries\n- coffee\n- bread\n- cheese\n\n#home","secure":false,"created":1794400000,"updated":1794600000}
]
JSON
"$MENEW_BIN" run open.notes >/dev/null 2>&1; sleep 3
shot notes
"$MENEW_BIN" __palette-query "groceries" >/dev/null 2>&1; sleep 2
if log_has 'result\[.*\] \[Note\] Groceries\|result\[.*\] .*Groceries'; then pass "palette finds the note 'Groceries'"; else fail "palette did not list the note"; fi
"$MENEW_BIN" toggle >/dev/null 2>&1; sleep 1

# ---- performance and settings ----
"$MENEW_BIN" run open.performance >/dev/null 2>&1; sleep 5
shot performance
"$MENEW_BIN" run open.settings >/dev/null 2>&1; sleep 2
shot settings
close_windows; sleep 1

# ---- popover ----
"$MENEW_BIN" popover >/dev/null 2>&1; sleep 3
shot popover
"$MENEW_BIN" popover >/dev/null 2>&1; sleep 1

# ---- screenshot editor and QR code ----
python3 - "$TEST_ROOT/fake-shot.png" <<'PY'
import sys
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, 900, 520)
p.fill(0x3b6fd8ff)
p.savev(sys.argv[1], "png", [], [])
PY
"$MENEW_BIN" run "cap.edit:$TEST_ROOT/fake-shot.png" >/dev/null 2>&1; sleep 3
shot editor
close_windows; sleep 1
"$MENEW_BIN" run "cap.qrshow:https://menew.web.app" >/dev/null 2>&1; sleep 3
shot qr
close_windows; sleep 1

# ---- quick actions ----
"$MENEW_BIN" __palette-query "awake 2h" >/dev/null 2>&1; sleep 2
if log_has 'Keep awake for 2 h'; then pass "palette 'awake 2h' offers keep awake for 2 h"; else fail "no keep awake item for 'awake 2h'"; fi
"$MENEW_BIN" __palette-query "brightness 40" >/dev/null 2>&1; sleep 1
"$MENEW_BIN" toggle >/dev/null 2>&1
before="$(gsettings get org.gnome.desktop.interface color-scheme)"
"$MENEW_BIN" run qa.dark >/dev/null 2>&1; sleep 2
after="$(gsettings get org.gnome.desktop.interface color-scheme)"
if [ "$before" != "$after" ]; then pass "qa.dark switched color-scheme ($before -> $after)"; else fail "qa.dark did not change color-scheme ($before)"; fi
"$MENEW_BIN" run qa.dark >/dev/null 2>&1; sleep 1
"$MENEW_BIN" run qa.awake:1 >/dev/null 2>&1; sleep 2
if pgrep -f 'systemd-inhibit --what=idle:sleep --who=Menew' >/dev/null || log_has 'Keep awake needs systemd-inhibit'; then pass "keep awake started systemd-inhibit (or said it is missing)"; else fail "keep awake did nothing"; fi
"$MENEW_BIN" run qa.awake >/dev/null 2>&1

# ---- window snapping through the extension ----
python3 - >/dev/null 2>&1 <<'PY' &
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
app = Gtk.Application(application_id="org.example.SnapTarget")
def on_activate(a):
    w = Gtk.ApplicationWindow(application=a, title="Snap target")
    w.set_default_size(400, 300)
    w.present()
    GLib.timeout_add_seconds(40, a.quit)
app.connect("activate", on_activate)
app.run([])
PY
track $!
sleep 4
shell_eval "global.display.focus_window && global.display.focus_window.get_gtk_application_id() === 'org.example.SnapTarget'" | grep -q true || shell_eval "global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === 'Snap target').activate(global.get_current_time()); 1" >/dev/null
sleep 1
"$MENEW_BIN" run win.left >/dev/null 2>&1; sleep 2
GEO="$(shell_eval "const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === 'Snap target'); const r = w.get_frame_rect(); const a = w.get_work_area_current_monitor(); [r.x - a.x, r.width, a.width].join(',')")"
info "snap target geometry (x offset, width, work width): $GEO"
if echo "$GEO" | python3 -c 'import sys,re; v=[int(x) for x in re.findall(r"-?\d+", sys.stdin.read())][-3:]; sys.exit(0 if len(v)==3 and abs(v[0])<=2 and abs(v[1]-v[2]//2)<=4 else 1)'; then
    pass "win.left put the window on the left half (through org.menew.Shell)"
else
    fail "win.left did not snap the window: $GEO"; grep -a -i 'snap' "$MENEW_LOG" | tail -5
fi

if grep -a -q 'panicked' "$MENEW_LOG"; then fail "the daemon panicked"; grep -a -A3 panicked "$MENEW_LOG" | head; else pass "no panic in the daemon log"; fi
[ "$FAILS" -eq 0 ]
