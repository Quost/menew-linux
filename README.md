# Menew for Linux

Private, native productivity utility for Linux desktops (GNOME and KDE Plasma on Wayland).

Linux port of [Menew for macOS](https://github.com/Quost/menew), in progress.

> **Status: not released yet.** Only part of the Mac app exists here so far (the command palette with its calculations, conversions, text transforms, quick links, emoji, timers, focus and script commands; text clipboard history; tray). The first release will come when the core of the Mac app is ported: see [docs/parity.md](docs/parity.md) for every feature and its Linux status.

## Features

- **Command palette** (`Ctrl+Alt+Space`), with the same behaviour as the Mac app. Type to search; **Ctrl+1** to **Ctrl+9** run the Nth result; your **recent picks come first**.
  - **Apps and Menew actions**: installed apps, clipboard history, clear history, start at login, start or stop a focus session, generate a UUID, copy the Unix timestamp, open the scripts folder, quit.
  - **Math and conversions**: `12*7.5`, `20% of 340`, `2(3+4)`, `sqrt 16`, `5 km to mi`, `72 f to c`, `1,5 gb em mb` (also `20% de 340`, `20% von 340`, `5 km em mi`, `nach`, `en`, `->`). Currencies: `100 usd to brl` fetches the European Central Bank rate **only when you press Enter** on it (see Privacy).
  - **Time zones**: `3pm ny to lisbon`, `15h em Lisboa`, `time in Tokyo`, `hora em Tóquio`, or just a city (`lisboa`). A pasted Unix timestamp (`1793400000`) shows its date; Enter copies the ISO date.
  - **Text transforms** for the copied text, with a live preview: UPPERCASE, lowercase, Title Case, Sentence case, remove accents, remove extra spaces, sort, reverse or dedupe lines, format or minify JSON, Base64 and URL encode and decode, count characters and words, camelCase, snake_case, kebab-case, and decode a JWT.
  - **Quick links**: `g coffee`, `gh menew`, `maps paulista`, `yt ...`, `wiki ...`, `ddg ...`, plus your own in `~/.config/menew/config.json` (`jira ABC-123` with `https://company.atlassian.net/browse/{q}`). A typed address like `github.com/quost` opens directly and anything else can be searched on the web. They open in your browser.
  - **Emoji and symbols**: `emoji rocket`, `emoji coração`, `:fire`, and words like `arrow`, `degree`, `euro` or `check`; Enter pastes (or copies).
  - **Timers and alarms**: `timer 10m tea`, `timer 1h30`, `alarm 7:30 meeting` (also `temporizador`, `alarme`, `wecker`, `alarma`, `闹钟`). The countdown shows in the tray (its title and tooltip), and at the end you get a notification and a sound.
  - **Focus timer**: `focus 50` (or just "Start focus" for 25 minutes), **+5 minutes**, stop, and a 5 minute break at the end.
  - **Script commands**: every file in `~/.local/share/menew/scripts` (shell, Python, Node, Ruby) becomes a command; Raycast script headers (title, icon, mode, argument) work as they are. "Open scripts folder" creates it with an example.
- **Clipboard history** (`Ctrl+Alt+V`, or **Tab** inside the palette): a second palette mode with search.
  - Pinned entries first, then the most recent. Each row shows a one-line preview and a relative age ("2 min").
  - **Enter** copies the entry and hides the palette, **Ctrl+P** pins or unpins it, **Ctrl+Delete** deletes it.
  - If `/dev/uinput` is writable, Menew also presses Ctrl+V in the app you came from. Otherwise it only copies and shows "Copied. Press Ctrl+V to paste".
  - Content marked as a password by a password manager is never saved.
- **Tray icon**: Open palette, Clipboard history, Clear history (keeps pinned entries), Start at login, Quit.
- **Start at login** is opt-in (off by default): it creates `~/.config/autostart/menew.desktop`, and unchecking it in the tray removes the file again.
- UI in English, Portuguese, German, Spanish and Chinese (from `LC_ALL` / `LANG`).

## Principles

- **Zero telemetry, zero analytics**: Menew makes no network request you did not ask for. The only one is the currency rate (see Privacy).
- **Minimal dependencies**: Built directly on Rust, GTK4, Libadwaita, and system D-Bus protocols.
- **Wayland-first**: Designed for modern Wayland compositors (GNOME Mutter and KDE KWin).
- **Fully translatable**: UI strings support en, pt, de, es, zh.

## Privacy

- The clipboard history is stored only in a local SQLite file (`~/.local/share/menew/clipboard.db`). Nothing is uploaded.
- The only network request is a currency rate, sent when you press Enter on a conversion like `100 usd to brl` in the palette: one HTTPS GET to `api.frankfurter.app` (European Central Bank data, the same provider the Mac app uses; it redirects to `api.frankfurter.dev`, which `curl` follows over https only) with the two currency codes and nothing else, made with the system `curl`. Typing never sends anything; the rate is kept in memory for 6 hours.
- Quick links open in your browser; Menew itself does not contact those sites.
- Timers, alarms and the focus session live only in memory (quitting Menew cancels them). Your recent picks are stored in `~/.local/share/menew/recents.json` (item ids only, never copied text), and your scripts, quick links and world clocks in `~/.local/share/menew/scripts` and `~/.config/menew/config.json`.
- Scripts only run when you pick them in the palette.
- Copies flagged by password managers (`x-kde-passwordManagerHint`, `CONCEALED` and similar) are ignored.
- "Clear history" in the tray deletes every unpinned entry.
- The optional paste simulation only writes a Ctrl+V key press to `/dev/uinput`; it never reads keyboard input.

## Architecture

The project is structured as a modular Cargo workspace:

- `menew`: Main application binary and coordinator.
- `menew-core`: Pure Rust business logic (the palette engine and one module per Mac file: calculator, conversions, currency, time zones, text transforms, quick links, emoji, timers, focus, script commands, recents), `.desktop` application scanner, SQLite storage (`rusqlite`), and i18n localization engine. Its palette behaviour is specified by `docs/palette-spec.md`, whose test vectors are run by the tests.
- `menew-ui`: Command palette window built with GTK4 and Libadwaita.
- `menew-tray`: System tray integration via StatusNotifierItem (`ksni`).
- `menew-shortcuts`: Global shortcut listener using `org.freedesktop.portal.GlobalShortcuts` (`ashpd`).
- `menew-clipboard`: Clipboard history manager supporting KDE Wayland data-control and GNOME D-Bus extension bridge.
- `gnome-extension`: Lightweight GNOME Shell extension bridging Wayland clipboard events to the Menew daemon.

## Install

There is no release yet. Once there is, the files will be on the GitHub release page; check them with `SHA256SUMS`. Until then, build from source (see [CONTRIBUTING.md](CONTRIBUTING.md)) or build the packages yourself (`cargo deb -p menew`, `scripts/build-appimage.sh`).

```bash
sha256sum -c SHA256SUMS
```

### .deb (Ubuntu 24.04+, Debian 13)

```bash
sudo apt install ./menew_<version>_amd64.deb
menew --version   # prints the version
menew             # starts hidden in the tray; open the palette with Ctrl+Alt+Space
```

`wl-clipboard` is recommended (used for clipboard history on KDE).

### AppImage (any x86_64 distro)

```bash
chmod +x Menew-<version>-x86_64.AppImage
./Menew-<version>-x86_64.AppImage
```

GTK4 and libadwaita are bundled. If you enable "Start at login" in the tray, the autostart entry points at the AppImage file, so keep it where it is. The AppImage does not install the GNOME extension (see below).

### GNOME: clipboard extension and tray icon

On GNOME Wayland the clipboard history needs the Menew GNOME Shell extension, and the tray icon needs the AppIndicator extension.

- With the .deb the extension is installed in `/usr/share/gnome-shell/extensions/menew-bridge@menew.web.app/` but **not enabled**:

  ```bash
  gnome-extensions enable menew-bridge@menew.web.app
  ```

  Then log out and in again (Wayland cannot reload GNOME Shell).
- With the AppImage, install it from a checkout of this repository: `bash gnome-extension/install.sh`, then enable it as above.
- Tray icon: install the AppIndicator extension and enable it with `gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com`. On Ubuntu/Debian: `sudo apt install gnome-shell-extension-appindicator`. On Fedora: `sudo dnf install gnome-shell-extension-appindicator`.
- GNOME 45-47 has no GlobalShortcuts portal: create two custom shortcuts that run `menew toggle` and `menew clipboard`.

On KDE Plasma nothing extra is needed.

### Optional: paste with uinput

By default Menew copies the entry and shows "Copied. Press Ctrl+V to paste". To let it press Ctrl+V for you it needs write access to `/dev/uinput`. The packages **do not** install this. The rule is in `data/60-menew-uinput.rules`:

```bash
sudo cp data/60-menew-uinput.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger /dev/uinput
sudo usermod -aG input "$USER"    # then log out and in
```

Security risk: any program running as a member of the `input` group can then inject keystrokes into your session. Only enable it if you accept that. To undo it, remove the rule file and the group membership (`sudo gpasswd -d "$USER" input`).

## Building and Testing

Prerequisites on Linux:
- Rust toolchain (edition 2021)
- GTK4 and Libadwaita development headers (`libgtk-4-dev`, `libadwaita-1-dev`)

```bash
# Run unit tests across all workspace crates
cargo test

# Build debug binary
cargo build

# Run application (starts hidden in the tray; the palette is NOT shown on launch)
cargo run -p menew
```

### Commands

```bash
menew            # first launch: start the daemon hidden (tray only), e.g. for autostart at login.
                 # If already running, shows the palette.
menew toggle     # show/hide the palette (bind this to a custom system shortcut on GNOME 45-47)
menew clipboard  # open the palette in clipboard mode (bind this to Ctrl+Alt+V on GNOME 45-47)
menew quit       # quit the running daemon
```

The palette also opens from the global shortcuts or the tray menu.

### Shortcuts

| Where | Keys | Action |
|---|---|---|
| Global (GlobalShortcuts portal) | `Ctrl+Alt+Space` | toggle the palette |
| Global (GlobalShortcuts portal) | `Ctrl+Alt+V` | toggle the palette in clipboard mode |
| Palette | `Tab` | switch between Apps and Clipboard |
| Palette | `Up` / `Down`, `Enter`, `Esc` | move, run or copy, close |
| Clipboard mode | `Ctrl+P` | pin or unpin the selected entry |
| Clipboard mode | `Ctrl+Delete` | delete the selected entry |

The portal is available on KDE Plasma and GNOME 48+. On GNOME 45-47 create two custom shortcuts in
Settings (Keyboard, Custom Shortcuts) that run `menew toggle` and `menew clipboard`.

### Configuration

`~/.config/menew/config.json` (optional; defaults are used when it is missing or invalid):

```json
{ "paste_mode": "auto" }
```

- `"auto"` (default): inject Ctrl+V through `/dev/uinput` when it is writable, otherwise copy and notify.
- `"copy-only"`: never inject; only copy and notify.

For `auto` to paste, the user needs write access to `/dev/uinput` (for example a udev rule that
grants the `input` group access, plus membership in that group). Without it Menew falls back to the notification.

### Desktop tests (headless GNOME and KDE)

```bash
bash scripts/desktop-test/gnome.sh
bash scripts/desktop-test/kde.sh
```

See `scripts/desktop-test/README.md` for packages, what is real vs substituted, and screenshots.
Screenshots with fake data are in `docs/desktop-tests/` (`*-palette.png`, `*-clipboard.png`).

## License

[GPL-3.0](LICENSE) © Matheus Quost.

The name “Menew”, the icon and the website are not part of the license: a fork needs its own name and icon. See [TRADEMARK.md](TRADEMARK.md).
