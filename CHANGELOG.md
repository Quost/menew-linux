# Changelog

## Unreleased

Not released yet. The first release will come when the core of the Mac app is ported (every R1 row in `docs/parity.md`). What exists so far:

### What works

- **Command palette** with the behaviour of the Mac app: apps and Menew actions, recent picks first, math and unit conversions, currencies (the rate is fetched only when you press Enter), time zones, text transforms of the copied text with a live preview, a JWT decoder, UUID and Unix timestamps, quick links and web search, emoji and symbols, timers and alarms (countdown in the tray, notification and sound at the end), the focus timer and script commands (Raycast headers).
- **Clipboard history**: text, links, images and files with search, filters, pause, a paste queue, smart actions, OCR of images and snippets (see docs/clipboard-spec.md). Pinned entries first, Enter copies, Ctrl+P pins, Ctrl+Delete deletes. Copies flagged by password managers are never saved.
- **Clipboard, Phase 5 (parity with the Mac)**: images and files with previews, filters, pause (15 min, 1 h, until resumed; nothing is read while paused), ignored apps (GNOME), paste queue with "Paste next" (`Ctrl+Alt+N`, `menew paste-next`), smart actions, text of images with `tesseract` (the `.deb` recommends `tesseract-ocr`), snippets with variables, history limits (300 entries, at least 20). Vectors from the Mac code in `docs/clipboard-spec.md`.
- **Notes, Phase 6 (parity with the Mac)**: many notes with `#tags`, search and Markdown (checklists you can tick), the same `notes.json` format as the Mac, protected and password notes in the Secret Service (GNOME Keyring or KWallet) with a polkit password check (fallback to the keyring unlocked at login, shown in the window), password rows with hidden values, copy marked as confidential and cleared after 30 s, notes in the palette, the clipboard Passwords filter (Enter password, Alt+Enter username) and `menew notes`. The `.deb` installs `app.menew.Menew.policy` and recommends `gnome-keyring | kwalletmanager`. Vectors from the Mac code in `docs/notes-spec.md`. Summarize and extract to-dos with AI are not in release 1.
- **Screenshots, text from the screen and QR codes, Phase 7 (parity with the Mac)**: capture an area, a window or the whole screen with `Ctrl+Alt+Shift+S`, `menew capture`, the palette or the tray (the system takes the picture through the screenshot portal and shows its own picker), and the annotation editor with a pen, arrows, rectangles, text, **blur that really pixelates**, undo, copy, save (Desktop, `Screenshot <date>.png`) and drag into another app. **Copy text from the screen** (`Ctrl+Alt+Shift+T`, `menew capture-text`, `tesseract`). **QR codes**: read one from the screen (the content is copied, a link is never opened) and show the QR code of the copied text, a link (new smart action) or your Wi-Fi. Two small pure Rust crates read and make the codes (`rqrr`, `qrcodegen`); `pangocairo` draws the text of the editor. The file the portal writes is deleted right after it is read. Vectors from the Mac code in `docs/screenshots-spec.md`. Not in release 1: pin on screen, screen recording, scrolling capture and asking about the screen.
- **Paste**: if `/dev/uinput` is writable, Menew presses Ctrl+V in the app you came from. Otherwise it copies and shows "Copied. Press Ctrl+V to paste" (the default).
- **Tray icon** with Open palette, Clipboard history, Clear history, Start at login and Quit. Start at login is opt-in.
- UI in English, Portuguese, German, Spanish and Chinese.
- No telemetry, no analytics, no network requests. History lives in a local SQLite file.

### Known limitations

- This was only tested on headless GNOME Shell 46 and KWin 5.27 sessions (Ubuntu 24.04), not on a real desktop yet.
- Global shortcuts (`Ctrl+Alt+Space`, `Ctrl+Alt+V`, `Ctrl+Alt+Shift+S`, `Ctrl+Alt+Shift+T`) and keyboard handling are untested on real hardware. On GNOME 45-47 the GlobalShortcuts portal is missing: create custom shortcuts that run `menew toggle`, `menew clipboard`, `menew capture` and `menew capture-text`.
- The screenshot portal and the picker of the system were not tested on a real desktop (see `docs/desktop-tests` for what the headless sessions could run); dragging the capture out of the editor into another app was not tested either.
- GNOME needs the bundled GNOME Shell extension for clipboard history, and the AppIndicator extension (`gnome-shell-extension-appindicator`) to show the tray icon.
- Wayland only (GNOME and KDE Plasma). X11 is best effort.
- The AppImage does not install the GNOME extension; see the README.

### Install

- **Debian / Ubuntu 24.04+ (.deb)**: `sudo apt install ./menew_<version>_amd64.deb`. The GNOME extension is installed but not enabled: run `gnome-extensions enable menew-bridge@menew.web.app` and log in again.
- **Any distro (AppImage)**: `chmod +x Menew-<version>-x86_64.AppImage && ./Menew-<version>-x86_64.AppImage`.
- Check downloads against `SHA256SUMS`.
