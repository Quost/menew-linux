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

## Phase 6 (notes)

`phase6.sh` is sourced after `phase5.sh`. It starts `gnome-keyring-daemon --login` in the test session (a throwaway
login keyring unlocked with a test password) and needs `gnome-keyring` and `libsecret-tools` (`secret-tool`); without
them every Secret Service check prints `SKIP` with the reason and is not counted as a pass. The daemon runs with
`MENEW_SECRET_CLEAR_SECS=3` (read only with `MENEW_TEST=1`), so the 30 s clear of a copied password takes 3 s.
Test hooks: `__notes-create <text|secure|passwords> "<text with \n>"` (logs the id), `__notes-open [id]`,
`__password-paste <index> <password|username>` (Enter or Alt+Enter on a row of the Passwords filter).
It checks: a normal note with a `#tag` in `notes.json` and found in the palette; a password note whose value is in no plain
file (`grep -r`) and is in the keyring (`secret-tool lookup application app.menew.Menew note <uuid>`); the Passwords
filter; the copied value on the clipboard, not in the history, cleared after the delay only if still there; polkit
(recorded: the policy is not installed in the test session, so the fallback runs; a real authentication agent
does not exist headless, which is a `SKIP`); and the screenshot `docs/desktop-tests/<de>-notes.png`.

## Phase 7 (screenshots, text from the screen, QR codes)

`phase7.sh` is sourced after `phase6.sh` (`phase7_checks`), with the helper `p7check.py` for the pixel checks. It needs
`imagemagick` (`convert`, `identify`, `compare`) and `tesseract-ocr`. Test hooks (only with `MENEW_TEST=1`):
`__capture-from <edit|text|qr> <png>` (use this image as the capture; the file is kept, a file of the portal is deleted),
`__annotate <in.png> <out.png> <marks>` (the export of the editor without a window; marks are `blur:x1,y1,x2,y2`,
`rect:x1,y1,x2,y2:color`, `arrow:...`, `pen:x,y x,y ...:color`, `text:x,y:color:Text`, joined by `;`),
`__capture-save <in.png> <dir>` (what Save writes: `Screenshot <date>.png` in that folder), `__capture-copy <in.png> <marks>`
(what Copy does), `__qr-image <text> <out.png>` (the image of "Copy image") and `__qr-show [text]` (open the QR window, log its payload).

**The screenshot portal itself**: the script ends by running `menew capture-full` against the real
`org.freedesktop.portal.Screenshot` of the session and records what happened (a picture and the editor, or the error
shown to the user). When no backend can take a picture in the headless session the check is a `SKIP` that says so, and the
pictures of the other checks come from `__capture-from`. The picker of `menew capture` (area or window) needs a person and is
not run. Dragging the image out of the editor needs a pointer and is not run either.

What is checked: OCR of generated images (one line, two lines, a blank image); QR codes made by the app, put on a
1280x800 picture and read back (link, Wi-Fi, none) with the clipboard content, the messages, **no `Opened` line in the daemon
log, no OpenURI call on the session bus, no browser process**, and a positive control that an open from the palette does
show in the same log; the QR window and its screenshot; the export of the editor, checked pixel by pixel by `p7check.py`
(inside a blur box every pixel is the average of its 10x10 block and differs from the original, outside everything is
identical; a rectangle, an arrow, a text and a pen change only pixels near their own coordinates); Save (name and
content), Copy (the annotated image reaches the clipboard history and keeps its blur); the editor window and its
screenshot; the palette, tray and smart action entries.
