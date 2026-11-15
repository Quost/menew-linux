# Headless desktop tests

Two scripts bring up a headless compositor inside its own `dbus-run-session` with a throwaway
`HOME`, run the `menew` daemon, check the clipboard / tray / palette behaviour and kill
everything at the end. They print `PASS` / `FAIL` / `SKIP` per check and exit non-zero if any
check failed. A `SKIP` is never counted as a pass: it means the check could not run here.

```bash
cargo build                                   # binary at target/debug/menew
export MENEW_BIN="$PWD/target/debug/menew"    # default: ~/target-menew/debug/menew
bash scripts/desktop-test/gnome.sh            # headless GNOME Shell + the bridge extension
bash scripts/desktop-test/kde.sh              # headless KWin + wl-paste watcher
```

Release 1 checks run inside the same sessions (`EXPLORE_CMD` runs a script in the live session and
passes on its exit status):

```bash
EXPLORE_CMD='bash scripts/desktop-test/r1-checks.sh' bash scripts/desktop-test/gnome.sh
EXPLORE_CMD='bash scripts/desktop-test/r1-kde-checks.sh' bash scripts/desktop-test/kde.sh
```

GNOME: notes (written to `notes.json` while running, found in the palette), the Performance and
Settings sections, the tray popover (`menew popover`), the screenshot editor on a generated image,
a QR code window, `awake 2h` in the palette, dark mode through `gsettings`, keep awake, and a GTK
window snapped to the left half through the extension's `org.menew.Shell` (geometry read back from
the shell). KDE: a window snapped to the right half through a KWin script, geometry read back with
a second KWin script. Screenshots go to `docs/desktop-tests/gnome-r1-*.png` (not committed).

From macOS or Windows, everything runs in the Linux dev image (`scripts/dev/Dockerfile`):

```bash
docker build -t menew-dev scripts/dev
bash scripts/dev/run.sh "cargo test --workspace"
bash scripts/dev/run.sh "bash scripts/dev/desktop-tests.sh"   # GNOME and KDE, plus release 1
```

In a container there is no logind: the script removes `/run/systemd/seats` so GNOME Shell uses its
dummy login manager, starts a system bus, and drops `cap_sys_nice` from `kwin_wayland`. Keep awake
therefore reports that `systemd-inhibit` cannot work there, which the check accepts.

Screenshots land in `docs/desktop-tests/`: `gnome-palette.png`, `kde-palette.png` (Apps mode),
`gnome-clipboard.png`, `kde-clipboard.png` (clipboard mode, fake data only) and
`<de>-palette-units.png`, `<de>-palette-emoji.png` (the palette with `5 km to mi` and `emoji rocket`).
The palette checks (`palette_checks` in `lib.sh`) also read the daemon log for the results of `5 km to mi`,
`snake_case` (on a copied text), `emoji rocket`, `100 usd to brl` (and that no currency request is made while
typing), `timer 1m test` and `focus 1`, read the countdown from the tray's StatusNotifierItem `Title` and
`ToolTip` properties over D-Bus, and wait for a `timer 3s done` to end (notification and sound lines in the log).
The sound in these sessions is the display beep: no `canberra-gtk-play` or `paplay` is installed.
The daemon is pointed at a temporary database with the test hook `MENEW_DB_PATH`, and is started
with `MENEW_TEST=1`, which enables the hidden commands `__select-clipboard`, `__pin-clipboard` and
`__delete-clipboard <index>` (0-based), and the palette hooks `__palette-query "<text>"` (open the palette,
type the text, log the results as `Test hook: result[N] [Kind] title | subtitle | action`) and
`__palette-activate <index>` (Enter on the Nth result). The scripts exit with the real status of the run
(`dbus-run-session` itself does not pass it on, so it goes through a file).

To read the exit status from Windows use `wsl.exe --exec`; a plain `wsl.exe -- bash -lc "...; echo $?"`
expands `$?` in the wrapper shell and always prints 0:

```bash
wsl.exe -d Ubuntu-24.04 --exec bash -lc 'cd /path/to/menew-linux && bash scripts/desktop-test/kde.sh; echo exit=$?'
```

## Packages (Ubuntu 24.04, GNOME Shell 46, KWin 5.27)

`gnome-shell gnome-shell-extensions mutter kwin-wayland kde-spectacle qtwayland5 wl-clipboard
xdg-desktop-portal xdg-desktop-portal-gnome xdg-desktop-portal-kde sqlite3 grim imagemagick
dbus-user-session dbus-x11 libglib2.0-bin python3-yaml procps psmisc` (plus `python3-gi` and GTK 4
typelibs, which come with the GTK dev packages).

## What is real and what is substituted

| Check | Real | Substituted, and why |
|---|---|---|
| GNOME extension enabled | state read from the running shell | `gnome-extensions` CLI needs `org.gnome.Shell.Extensions`, which a headless shell does not export; state is read through `org.gnome.Shell.Eval` (`--unsafe-mode`) |
| GNOME copy -> SQLite | extension -> D-Bus -> daemon -> SQLite | the copy is made inside the shell (`St.Clipboard.set_text`), because `wl-copy`/`wl-paste` need keyboard focus and a headless shell has no keyboard ("This seat has no keyboard") |
| GNOME password hint | extension hint branch (asserted from its log line) | selection source offers only the hint type (GJS cannot subclass `Meta.SelectionSource`) |
| KDE copy -> SQLite | real `wl-copy` -> `wl-paste --watch` -> helper -> daemon | none |
| KDE password hint | real helper MIME check + daemon | a stub `wl-paste` reports the MIME list, since no two-type selection can be created headless |
| KDE tray | real ksni registration | `watcher.py`, a ~50 line StatusNotifierWatcher stand-in (no plasmashell installed) |
| Phase 2: select an entry | real GTK palette sets the real clipboard, read back with `wl-paste` (KDE) or in-shell `St.Clipboard.get_text` (GNOME) | the palette is kept open while selecting, and Enter is replaced by the `__select-clipboard` hook (no keyboard headless) |
| Phase 2: pin / delete | the same functions Ctrl+P / Ctrl+Delete call | `__pin-clipboard` / `__delete-clipboard` hooks |
| Phase 2: tray menu | real ksni dbusmenu, clicked with `com.canonical.dbusmenu.Event` | `watcher.py` stand-in (also on GNOME) |
| Phase 2: paste | notification fallback observed with `dbus-monitor`; real uinput only if `/dev/uinput` is writable | `notifyd.py` stand-in on KDE (KWin has no notification server) |
| KDE screenshot | `spectacle` via KWin ScreenShot2 | `grim` does not work: KWin 5.27 has no wlr-screencopy |
| GNOME screenshot | `org.gnome.Shell.Screenshot` | none |

## GlobalShortcuts portal

Both scripts only record the result of `menew` registering the shortcut. Headless there is no
portal backend implementing `org.freedesktop.portal.GlobalShortcuts` (GNOME 46 and plasma-less
KWin 5.27), so the real result is `Unsupported`; it is reported as `SKIP`, not forced.
