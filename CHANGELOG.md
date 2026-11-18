# Changelog

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

