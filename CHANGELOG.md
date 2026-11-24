# Changelog

## Unreleased

### What is new

- **Window snapping** with the Mac's 17 positions, from the palette (`window left`, `corner`, `third`, `center`...) and the shortcuts `Ctrl+Alt+Shift+Arrows` and `Ctrl+Alt+Enter`. On KDE Plasma it works out of the box; on GNOME it needs the Menew extension version 2 (log out and in after updating).
- **The GNOME extension is now `menew-bridge@pururucos.com.br`** (it was `menew-bridge@menew.web.app`). Enable it with `gnome-extensions enable menew-bridge@pururucos.com.br`; if you had enabled the old one, run `gnome-extensions uninstall menew-bridge@menew.web.app` first.
- **Quick actions** in the palette: mute the microphone (`Ctrl+Alt+M`) or the sound, switch the sound output and input, dark mode, keep awake (also `awake 2h`), lock screen, empty trash, color picker, plain text clipboard, eject disks, brightness (also external displays with `ddcutil`), quit or force quit apps (`quit firefox`, `kill firefox`, "Quit all apps" with a never-quit list) and stop a server on a port (`port 3000`).
- **New note** with `Ctrl+Alt+J`.
- `menew run <id>` runs any of these from a terminal or a custom shortcut (`menew run qa.mic`, `menew run win.left`, `menew run note.new`), for GNOME 45 to 47, which have no GlobalShortcuts portal.

### Fixed

- **Security:** the `.deb` update is now downloaded into a new private temporary folder (mode 0700, checked to belong to you) instead of a predictable one that another user of the computer could have prepared, since that file is then installed as root.
- The data folder `~/.local/share/menew` (clipboard history, images, notes, snippets) is now private to you: folders 0700 and files 0600. Existing folders and `clipboard.db` files are fixed at startup.
- `config.json` is written atomically, so a crash never leaves it half written.
- The default log now keeps warnings and errors of every part of Menew, and notifications are logged by id only, never with the copied text they show.
- GNOME extension: its pending timers are removed when it is disabled, and each copy is logged only at debug level. `install.sh` works from any folder.
- The Portuguese description of the app has its accents back, and the metainfo lists 0.1.0-alpha.2 and says which network requests Menew makes (the optional update check and the currency rate).

### Known limitations

- Quitting apps sends a signal: Wayland can't ask an app to close its windows.
- Snapping needs KWin or the GNOME extension; X11 sessions without either can't snap.
- Audio devices, brightness, eject and the new shortcuts were tested only on headless GNOME Shell 46 and KWin 5.27.

## 0.1.0-alpha.2

**Early preview for testers.** Screenshots with annotations, text from the screen, QR codes and the system monitor join the command palette, the clipboard history and notes. If you installed 0.1.0-alpha.1, Menew offers this update by itself.

### What is new

- **Screenshots** (`Ctrl+Alt+Shift+S`, `menew capture`, the palette or the tray): capture an area, a window or the whole screen (the system takes the picture through the screenshot portal and shows its own picker), then annotate it with a pen, arrows, rectangles and text, **blur** sensitive data (the pixels are really pixelated), undo, copy, save to your Desktop as `Screenshot <date>.png`, or drag it into another app. The file the portal writes is deleted right after it is read.
- **Copy text from the screen** (`Ctrl+Alt+Shift+T`, `menew capture-text`): needs `tesseract`.
- **QR codes**: read one from the screen (the content is copied, a link is never opened), and show the QR code of the copied text, a link or your Wi-Fi.
- **System monitor** (`menew monitor`, the palette or the tray): CPU with a graph and each core, GPU (AMD), memory with swap and pressure, disk, network speed, battery with time left, and the top processes by CPU and by memory with one-click quit (SIGTERM, then an explicit Force Quit; system and other users' processes are never offered). It samples only while its window is open. Metrics can also go in the tray (`monitor_tray_items` in `config.json`; text in the panel on KDE only, tooltip on GNOME).

### Not there yet

- Pinning a screenshot on the screen, screen recording, scrolling capture and asking about the screen.
- System monitor history, temperatures and fans, battery health and alerts.
- Window management, quick actions, Memory and on-device AI.

### Known limitations

- The screenshot portal and its picker were not tested on a real desktop yet, and neither was dragging a capture out of the editor into another app. Everything was tested only on headless GNOME Shell 46 and KWin 5.27 sessions (Ubuntu 24.04).
- Global shortcuts need the GlobalShortcuts portal (KDE Plasma, GNOME 48+). On GNOME 45 to 47, create custom shortcuts that run `menew toggle`, `menew clipboard`, `menew paste-next`, `menew capture` and `menew capture-text`.
- On GNOME, the clipboard history needs the bundled GNOME Shell extension, and the tray icon needs the AppIndicator extension.
- Pasting into the app you came from needs write access to `/dev/uinput` (optional, see the README); without it Menew copies and tells you to press Ctrl+V.
- Wayland first; X11 is best effort.

### Install

- **Debian / Ubuntu 24.04+ (.deb)**: `sudo apt install ./menew_<version>_amd64.deb`. The GNOME extension is installed but not enabled: run `gnome-extensions enable menew-bridge@menew.web.app` and log in again.
- **Any distro (AppImage)**: `chmod +x Menew-<version>-x86_64.AppImage && ./Menew-<version>-x86_64.AppImage`.
- Check downloads against `SHA256SUMS`.

## 0.1.0-alpha.1

**Early preview for testers.** This is the first build of Menew for Linux, the port of [Menew for Mac](https://github.com/Quost/menew). It has the command palette, the clipboard history and notes; the rest of the core of the Mac app (screenshots, window management, quick actions, system monitor and the Menew window) is still being ported, see [docs/parity.md](https://github.com/Quost/menew-linux/blob/main/docs/parity.md). Please try it on your GNOME or KDE desktop and [report what breaks](https://github.com/Quost/menew-linux/issues/new/choose).

### What works

- **Command palette** (`Ctrl+Alt+Space` or `menew toggle`): apps and Menew actions with your recent picks first, math and unit conversions, currencies (the rate is fetched only when you press Enter), time zones, transforms of the copied text with a live preview, JWT, UUID and Unix timestamps, quick links and web search, emoji and symbols, timers, alarms and a focus timer with the countdown in the tray, and script commands (Raycast headers work).
- **Clipboard history** (`Ctrl+Alt+V` or `menew clipboard`): text, links, images and files with search, filters and a preview; pins; pause for 15 minutes or an hour; smart actions for phones, addresses, e-mails, tracking codes, colors and meeting links; the text inside copied images is found by search (with `tesseract`); snippets with variables; a paste queue with "Paste next" (`Ctrl+Alt+N`). Copies flagged by password managers are never saved.
- **Notes** (`menew notes`): many notes with `#tags`, search and Markdown with checklists. Protected and password notes keep their content in your keyring (GNOME Keyring or KWallet), never in plain files; a copied password stays out of the history and is cleared after 30 seconds.
- **Automatic updates**, like the Mac app: Menew checks once a day (you can switch it off in the tray), shows a dot on the tray icon when a new version is out, and installs it after you confirm. The AppImage replaces itself; the .deb installs through your password prompt. Every update is checked against an Ed25519 signature before anything is installed. Testers on an alpha get the next alphas.
- **Tray icon**, opt-in start at login, and the interface in English, Portuguese, German, Spanish and Chinese.
- No telemetry and no account. The only network requests are the update check (once a day, can be switched off) and the currency rate, when you press Enter on a conversion.

### Known limitations

- Tested so far only on headless GNOME Shell 46 and KWin 5.27 sessions (Ubuntu 24.04). This is the first time it meets real desktops.
- Global shortcuts need the GlobalShortcuts portal (KDE Plasma, GNOME 48+). On GNOME 45 to 47, create custom shortcuts that run `menew toggle`, `menew clipboard` and `menew paste-next`.
- On GNOME, the clipboard history needs the bundled GNOME Shell extension, and the tray icon needs the AppIndicator extension.
- Pasting into the app you came from needs write access to `/dev/uinput` (optional, see the README); without it Menew copies and tells you to press Ctrl+V.
- Opening a protected note asks for your password through polkit only with the .deb (which installs the policy); with the AppImage the keyring unlocked at login is the protection, and the notes window says so.
- Wayland first; X11 is best effort.

### Install

- **Debian 13 / Ubuntu 24.04+ (.deb)**: `sudo apt install ./menew_0.1.0-alpha.1_amd64.deb`. To use the clipboard on GNOME, enable the extension with `gnome-extensions enable menew-bridge@menew.web.app` and log in again.
- **Any x86_64 distribution (AppImage)**: `chmod +x Menew-0.1.0-alpha.1-x86_64.AppImage && ./Menew-0.1.0-alpha.1-x86_64.AppImage`.
- Check the files against `SHA256SUMS`.

