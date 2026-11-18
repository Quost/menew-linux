# Changelog

## Unreleased

Not released yet. The first release will come when the core of the Mac app is ported (every R1 row in `docs/parity.md`). What exists so far:

### What works

- **Command palette** with the behaviour of the Mac app: apps and Menew actions, recent picks first, math and unit conversions, currencies (the rate is fetched only when you press Enter), time zones, text transforms of the copied text with a live preview, a JWT decoder, UUID and Unix timestamps, quick links and web search, emoji and symbols, timers and alarms (countdown in the tray, notification and sound at the end), the focus timer and script commands (Raycast headers).
- **Clipboard history**: text, links, images and files with search, filters, pause, a paste queue, smart actions, OCR of images and snippets (see docs/clipboard-spec.md). Pinned entries first, Enter copies, Ctrl+P pins, Ctrl+Delete deletes. Copies flagged by password managers are never saved.
- **Clipboard, Phase 5 (parity with the Mac)**: images and files with previews, filters, pause (15 min, 1 h, until resumed; nothing is read while paused), ignored apps (GNOME), paste queue with "Paste next" (`Ctrl+Alt+N`, `menew paste-next`), smart actions, text of images with `tesseract` (the `.deb` recommends `tesseract-ocr`), snippets with variables, history limits (300 entries, at least 20). Vectors from the Mac code in `docs/clipboard-spec.md`.
- **Notes, Phase 6 (parity with the Mac)**: many notes with `#tags`, search and Markdown (checklists you can tick), the same `notes.json` format as the Mac, protected and password notes in the Secret Service (GNOME Keyring or KWallet) with a polkit password check (fallback to the keyring unlocked at login, shown in the window), password rows with hidden values, copy marked as confidential and cleared after 30 s, notes in the palette, the clipboard Passwords filter (Enter password, Alt+Enter username) and `menew notes`. The `.deb` installs `app.menew.Menew.policy` and recommends `gnome-keyring | kwalletmanager`. Vectors from the Mac code in `docs/notes-spec.md`. Summarize and extract to-dos with AI are not in release 1.
- **Updates, Phase 7b (parity with the Mac's Sparkle)**: automatic check on by default (switch in the tray or `auto_update_check` in `config.json`), every 24 hours and once 5 minutes after start; a found version only puts a dot on the tray icon and an "Update available" item in its menu. "Check for Updates…" (tray, palette, `menew check-updates`) opens a window with the notes of the version and Install and Relaunch / Skip This Version / Later. The AppImage is replaced in place and restarted; the `.deb` is installed with `pkexec apt-get install -y`; a copy built from source is only told, with the link. Every update is verified (size, SHA-256 and an Ed25519 signature against the key compiled into Menew) before anything is installed. The check reads the GitHub release listing and `update.json` and sends nothing but `User-Agent: Menew/<version>`; it is the only network request besides the currency rate, and it is off in the screenshot mode. See `docs/updates-spec.md`.
- **Paste**: if `/dev/uinput` is writable, Menew presses Ctrl+V in the app you came from. Otherwise it copies and shows "Copied. Press Ctrl+V to paste" (the default).
- **Tray icon** with Open palette, Clipboard history, Clear history, Start at login and Quit. Start at login is opt-in.
- UI in English, Portuguese, German, Spanish and Chinese.
- No telemetry, no analytics. The only network requests are the currency rate (when you press Enter on a conversion) and the update check (on by default, can be switched off). History lives in a local SQLite file.

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
