# Changelog

## Unreleased

Not released yet. The first release will come when the core of the Mac app is ported (every R1 row in `docs/parity.md`). What exists so far:

### What works

- **Command palette**: search and launch installed apps, and evaluate math inline (type `2*(3+4)` and press Enter to copy the result).
- **Clipboard history**: a second palette mode with search. Pinned entries first, Enter copies, Ctrl+P pins, Ctrl+Delete deletes. Copies flagged by password managers are never saved.
- **Paste**: if `/dev/uinput` is writable, Menew presses Ctrl+V in the app you came from. Otherwise it copies and shows "Copied. Press Ctrl+V to paste" (the default).
- **Tray icon** with Open palette, Clipboard history, Clear history, Start at login and Quit. Start at login is opt-in.
- UI in English, Portuguese, German, Spanish and Chinese.
- No telemetry, no analytics, no network requests. History lives in a local SQLite file.

### Known limitations

- This was only tested on headless GNOME Shell 46 and KWin 5.27 sessions (Ubuntu 24.04), not on a real desktop yet.
- Global shortcuts (`Ctrl+Alt+Space`, `Ctrl+Alt+V`) and keyboard handling are untested on real hardware. On GNOME 45-47 the GlobalShortcuts portal is missing: create custom shortcuts that run `menew toggle` and `menew clipboard`.
- GNOME needs the bundled GNOME Shell extension for clipboard history, and the AppIndicator extension (`gnome-shell-extension-appindicator`) to show the tray icon.
- Wayland only (GNOME and KDE Plasma). X11 is best effort.
- The AppImage does not install the GNOME extension; see the README.

### Install

- **Debian / Ubuntu 24.04+ (.deb)**: `sudo apt install ./menew_<version>_amd64.deb`. The GNOME extension is installed but not enabled: run `gnome-extensions enable menew-bridge@menew.web.app` and log in again.
- **Any distro (AppImage)**: `chmod +x Menew-<version>-x86_64.AppImage && ./Menew-<version>-x86_64.AppImage`.
- Check downloads against `SHA256SUMS`.
