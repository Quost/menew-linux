# Menew for Linux: Project Rules for Agents

Native Linux productivity utility built in Rust (GTK4 + Libadwaita). Separate repository from the macOS version (`../menew`). MIT licensed.

## Core Principles (Strict - Never Violate)

1. **No Telemetry or Analytics, Ever**: No tracking, no phone-home, no silent network requests.
2. **Minimal Network Activity**: No network calls except ones the user explicitly initiates (e.g. currency conversion or updates).
3. **Minimal Dependencies**: Prefer standard system libraries, official GTK/Libadwaita bindings, and direct D-Bus protocols.
4. **Localization (i18n)**: Every single UI string must be translatable across 5 languages: English (`en`), Portuguese (`pt`), German (`de`), Spanish (`es`), and Chinese (`zh`).
5. **English Commits**: Conventional Commits in English (e.g., `feat(palette): ...`, `fix(clipboard): ...`).
6. **No Em-Dash**: Never use the em-dash character in documentation or code comments. Always use the regular hyphen (`-`).
7. **Isolation from Mac repo**: Do not touch `../menew`. The Linux version has its own lifecycle.

## Technical Architecture & Decisions

- **UI Stack**: Rust + GTK4 + Libadwaita (`gtk4`, `libadwaita`). No Tauri, no Electron.
- **Window Management**: Use standard GTK4 / Libadwaita floating windows. Do not depend on `gtk4-layer-shell`, which does not work on GNOME Mutter.
- **Target Desktops**: GNOME and KDE Plasma on Wayland. X11 is best effort only.
- **Global Shortcuts**: Handled via `org.freedesktop.portal.GlobalShortcuts` (`ashpd`). Default shortcut is `Ctrl+Alt+Space` (to avoid conflicting with GNOME's `Super+Space`). If the portal is unavailable (such as on GNOME 45-47), display clear user guidance to configure a custom system shortcut calling `menew toggle`.
- **System Tray**: StatusNotifierItem via `ksni`. Works out of the box on KDE; requires AppIndicator extension on GNOME (`gnome-shell-extension-appindicator`).
- **Clipboard Architecture**:
  - On KDE Plasma: Native Wayland data-control listener (`ext-data-control` / `wlr-data-control`).
  - On GNOME Wayland: Mutter blocks background clipboard reads. Clipboard history requires the Menew GNOME Shell Extension, which communicates with the daemon over D-Bus (`org.menew.ClipboardService`).
- **Virtual Paste Simulation**:
  - `uinput` (`/dev/uinput`) or `org.freedesktop.portal.RemoteDesktop` (`libei`).
- **Storage**: Local SQLite database via `rusqlite` with the `bundled` feature flag.
- **Packaging (Planned)**: AppImage, `.deb`, and AUR first. Flatpak sandboxing is fundamentally incompatible with global background clipboard capture and window activation without extensive overrides.
- **Local AI (Planned)**: Connect via HTTP REST API to an already installed local Ollama instance. Do not bundle heavy C++ runtimes like llama.cpp.

## Workspace Structure

- `crates/menew-core`: Pure Rust core logic, desktop entry scanner, inline math evaluator, SQLite storage, i18n dictionary.
- `crates/menew-ui`: Command palette window and widgets using GTK4 and Libadwaita.
- `crates/menew-tray`: System tray implementation with `ksni`.
- `crates/menew-shortcuts`: Global shortcut listener with `ashpd`.
- `crates/menew-clipboard`: Clipboard service and data-control/D-Bus listeners.
- `crates/menew`: Main coordinator daemon and entry point.
- `gnome-extension`: GNOME Shell extension for clipboard bridging on GNOME Wayland.
- `docs/`: Technical findings and architecture specifications.

## Code Quality Commands

```bash
cargo check
cargo test
cargo clippy
```
