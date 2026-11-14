# Menew for Linux

Private, native productivity utility for Linux desktops (GNOME and KDE Plasma on Wayland).

Linux counterpart of [Menew for macOS](https://menew.web.app).

## Features

- **Command palette** (`Ctrl+Alt+Space`): search and launch installed apps, and evaluate math inline (type `2*(3+4)` and press Enter to copy the result).
- **Clipboard history** (`Ctrl+Alt+V`, or **Tab** inside the palette): a second palette mode with search.
  - Pinned entries first, then the most recent. Each row shows a one-line preview and a relative age ("2 min").
  - **Enter** copies the entry and hides the palette, **Ctrl+P** pins or unpins it, **Ctrl+Delete** deletes it.
  - If `/dev/uinput` is writable, Menew also presses Ctrl+V in the app you came from. Otherwise it only copies and shows "Copied. Press Ctrl+V to paste".
  - Content marked as a password by a password manager is never saved.
- **Tray icon**: Open palette, Clipboard history, Clear history (keeps pinned entries), Start at login, Quit.
- **Start at login** is opt-in (off by default): it creates `~/.config/autostart/menew.desktop`, and unchecking it in the tray removes the file again.
- UI in English, Portuguese, German, Spanish and Chinese (from `LC_ALL` / `LANG`).

## Principles

- **Zero telemetry, zero analytics**: No data leaves your machine. Menew makes no network requests.
- **Minimal dependencies**: Built directly on Rust, GTK4, Libadwaita, and system D-Bus protocols.
- **Wayland-first**: Designed for modern Wayland compositors (GNOME Mutter and KDE KWin).
- **Fully translatable**: UI strings support en, pt, de, es, zh.

## Privacy

- The clipboard history is stored only in a local SQLite file (`~/.local/share/menew/clipboard.db`). Nothing is uploaded.
- Copies flagged by password managers (`x-kde-passwordManagerHint`, `CONCEALED` and similar) are ignored.
- "Clear history" in the tray deletes every unpinned entry.
- The optional paste simulation only writes a Ctrl+V key press to `/dev/uinput`; it never reads keyboard input.

## Architecture

The project is structured as a modular Cargo workspace:

- `menew`: Main application binary and coordinator.
- `menew-core`: Pure Rust business logic, `.desktop` application scanner, inline math evaluator, SQLite storage (`rusqlite`), and i18n localization engine.
- `menew-ui`: Command palette window built with GTK4 and Libadwaita.
- `menew-tray`: System tray integration via StatusNotifierItem (`ksni`).
- `menew-shortcuts`: Global shortcut listener using `org.freedesktop.portal.GlobalShortcuts` (`ashpd`).
- `menew-clipboard`: Clipboard history manager supporting KDE Wayland data-control and GNOME D-Bus extension bridge.
- `gnome-extension`: Lightweight GNOME Shell extension bridging Wayland clipboard events to the Menew daemon.

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
