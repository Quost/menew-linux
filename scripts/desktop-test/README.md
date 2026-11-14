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

Screenshots land in `docs/desktop-tests/gnome-palette.png` and `kde-palette.png`.
The daemon is pointed at a temporary database with the test hook `MENEW_DB_PATH`.

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
| KDE screenshot | `spectacle` via KWin ScreenShot2 | `grim` does not work: KWin 5.27 has no wlr-screencopy |
| GNOME screenshot | `org.gnome.Shell.Screenshot` | none |

## GlobalShortcuts portal

Both scripts only record the result of `menew` registering the shortcut. Headless there is no
portal backend implementing `org.freedesktop.portal.GlobalShortcuts` (GNOME 46 and plasma-less
KWin 5.27), so the real result is `Unsupported`; it is reported as `SKIP`, not forced.
