# Menew for Linux

Private, native productivity utility for Linux desktops (GNOME and KDE Plasma on Wayland).

Linux counterpart of [Menew for macOS](https://menew.web.app).

## Principles

- **Zero telemetry, zero analytics**: No data leaves your machine.
- **Minimal dependencies**: Built directly on Rust, GTK4, Libadwaita, and system D-Bus protocols.
- **Wayland-first**: Designed for modern Wayland compositors (GNOME Mutter and KDE KWin).
- **Fully translatable**: UI strings support en, pt, de, es, zh.

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
menew          # first launch: start the daemon hidden (tray only), e.g. for autostart at login.
               # If already running, shows the palette.
menew toggle   # show/hide the palette (bind this to a custom system shortcut on GNOME 45-47)
menew quit     # quit the running daemon
```

The palette also opens from the global shortcut (`Ctrl+Alt+Space`) or the tray menu.

### Desktop tests (headless GNOME and KDE)

```bash
bash scripts/desktop-test/gnome.sh
bash scripts/desktop-test/kde.sh
```

See `scripts/desktop-test/README.md` for packages, what is real vs substituted, and screenshots.

## License

MIT (c) 2026 Matheus Quost
