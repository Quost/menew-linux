# Changelog

## Unreleased

Not released yet. Every R1 row of `docs/parity.md` is now ported; the first tag waits for a pass on real GNOME and KDE desktops. What exists:

### What works

- **Command palette** with the behaviour of the Mac app: apps and Menew actions, recent picks first, math and unit conversions, currencies (the rate is fetched only when you press Enter), time zones, text transforms of the copied text with a live preview, a JWT decoder, UUID and Unix timestamps, quick links and web search, emoji and symbols, timers and alarms (countdown in the tray, notification and sound at the end), the focus timer and script commands (Raycast headers).
- **Clipboard history**: text, links, images and files with search, filters, pause, a paste queue, smart actions, OCR of images and snippets (see docs/clipboard-spec.md). Pinned entries first, Enter copies, Ctrl+P pins, Ctrl+Delete deletes. Copies flagged by password managers are never saved.
- **Clipboard, Phase 5 (parity with the Mac)**: images and files with previews, filters, pause (15 min, 1 h, until resumed; nothing is read while paused), ignored apps (GNOME), paste queue with "Paste next" (`Ctrl+Alt+N`, `menew paste-next`), smart actions, text of images with `tesseract` (the `.deb` recommends `tesseract-ocr`), snippets with variables, history limits (300 entries, at least 20). Vectors from the Mac code in `docs/clipboard-spec.md`.
- **Paste**: if `/dev/uinput` is writable, Menew presses Ctrl+V in the app you came from. Otherwise it copies and shows "Copied. Press Ctrl+V to paste" (the default).
- **Notes**: many notes with #tags, search and Markdown with clickable checklists; protected notes (text in the system keyring through `secret-tool`, opened after a polkit password check) and password notes (hidden fields, copied as a password and cleared after 30 s).
- **Screenshots**: area, window or screen through the Screenshot portal, an editor with arrows, rectangles, pen, text and blur (copy, save, drag out), copy text from the screen, read and show QR codes (also the current Wi-Fi).
- **Windows**: the Mac's positions from the palette and shortcuts, through a KWin script on KDE and the GNOME extension's new `org.menew.Shell` service.
- **Quick actions**: mute the mic (`Ctrl+Alt+M`) or the sound, audio output and input, dark mode, keep awake (also for a time), lock, empty trash, color picker, plain text clipboard, eject disks, brightness (also DDC/CI), quit or force quit apps, stop servers on ports.
- **System monitor**: CPU and memory graphs, GPU, disk, network, battery health, top processes with Quit, metrics next to the tray icon.
- **Focus**: can mute the mic, keep awake and pause the clipboard while it runs.
- **Tray icon**: left click opens the Now / Actions popover with editable favorites; the menu has Open Menew, Open palette, Clipboard history, Clear history, Pause, Paste next, Start at login and Quit. Start at login is opt-in.
- **Menew window** with Clipboard, Notes, Performance and Settings (language picker, turning features off completely, clipboard options, world clocks, focus options, shortcuts), and a **welcome tour** on the first launch.
- **Update check**: opt-in, once a day, GitHub releases only.
- `menew run <id>` runs any action (`qa.mic`, `cap.area`, `win.left`, `note.new`...), for custom shortcuts where the portal is missing.
- UI in English, Portuguese, German, Spanish and Chinese.
- No telemetry, no analytics. The only network requests are the currency rate on Enter and the update check if turned on. History lives in a local SQLite file, notes in `notes.json` and the keyring.

### Known limitations

- This was only tested on headless GNOME Shell 46 and KWin 5.27 sessions (Ubuntu 24.04), not on a real desktop yet.
- Window snapping on GNOME needs the Menew extension version 2 (log out and in after updating it). X11 sessions without KWin or the extension can't snap.
- Quitting apps sends a signal (Wayland can't ask an app to close its windows).
- Global shortcuts (`Ctrl+Alt+Space`, `Ctrl+Alt+V`) and keyboard handling are untested on real hardware. On GNOME 45-47 the GlobalShortcuts portal is missing: create custom shortcuts that run `menew toggle` and `menew clipboard`.
- GNOME needs the bundled GNOME Shell extension for clipboard history, and the AppIndicator extension (`gnome-shell-extension-appindicator`) to show the tray icon.
- Wayland only (GNOME and KDE Plasma). X11 is best effort.
- The AppImage does not install the GNOME extension; see the README.

### Install

- **Debian / Ubuntu 24.04+ (.deb)**: `sudo apt install ./menew_<version>_amd64.deb`. The GNOME extension is installed but not enabled: run `gnome-extensions enable menew-bridge@menew.web.app` and log in again.
- **Any distro (AppImage)**: `chmod +x Menew-<version>-x86_64.AppImage && ./Menew-<version>-x86_64.AppImage`.
- Check downloads against `SHA256SUMS`.
