# Menew for Linux: Spike Summary

## Overview

The spike phase for **Menew for Linux** is complete. We built a modular Rust Cargo workspace targeting GNOME and KDE Plasma on Wayland, validating all critical architectural risks identified in `HANDOFF.md`.

---

## What Was Done

1. **Cargo Workspace & Modular Architecture**:
   - Structured the repository as a clean Cargo workspace containing 6 crates:
     - `menew`: Main application coordinator and binary.
     - `menew-core`: Pure Rust business logic, `.desktop` scanner, math evaluator, SQLite storage, and multi-language dictionary.
     - `menew-tray`: System tray service via StatusNotifierItem (`ksni`).
     - `menew-shortcuts`: Global shortcut portal listener via `ashpd`.
     - `menew-clipboard`: Wayland data-control listener and GNOME D-Bus receiver.
     - `menew-ui`: GTK4 and Libadwaita command palette window.

2. **System Tray (`ksni`)**:
   - Implemented a StatusNotifierItem tray in `menew-tray`.
   - Includes localized menu items: "Open Command Palette" and "Quit Menew".
   - Communicates with the main GTK loop through asynchronous channels.

3. **Global Shortcut Portal (`ashpd`)**:
   - Implemented `org.freedesktop.portal.GlobalShortcuts` integration in `menew-shortcuts`.
   - Default preferred trigger configured to **`Ctrl+Alt+Space`** (avoiding GNOME's `Super+Space` input-switch collision).
   - Graceful fallback: If the portal is unavailable (such as on GNOME 45-47), Menew outputs clear, actionable setup guidance for creating a custom system shortcut calling `menew toggle`.

4. **Command Palette (`gtk4` + `libadwaita`)**:
   - Built an `adw::ApplicationWindow` palette with custom CSS styling (`.menew-palette`).
   - Scans system and user `.desktop` applications from standard XDG paths (`~/.local/share/applications`, `/usr/share/applications`, Flatpak, Snap).
   - Real-time search ranking by name, keywords, and execution binary.
   - Built-in inline math calculator (e.g. `12 * 7.5`, `20% of 340`, `100 / 4 + 15`).
   - Launching: Cleans desktop field codes (`%u`, `%F`) and spawns processes.
   - Keyboard control: `Escape` closes/hides the palette, `Return` launches the selected app, and arrow keys navigate.

5. **Clipboard PoC & SQLite Storage**:
   - Built a local SQLite store in `menew-core::clipboard_db` using `rusqlite` with the `bundled` feature (no external C library dependency).
   - Supports deduplication (avoiding duplicate spam), full-text search, pinning, and unpinned history clearing.
   - Implemented dual-path capture architecture:
     - **KDE Plasma**: Data-control listener thread using `wlr-data-control` / `ext-data-control`.
     - **GNOME Wayland**: D-Bus service `org.menew.ClipboardService` on session bus.

6. **GNOME Shell Extension Stub**:
   - Created `gnome-extension/` (ESM module for GNOME Shell 45-48).
   - Listens to `Meta.Selection` `owner-changed` signal in Mutter process space.
   - Fetches copied text via `St.Clipboard` and forwards it to the Menew daemon over D-Bus.
   - Includes `metadata.json`, `extension.js`, `install.sh`, and documentation.

7. **Localization (i18n)**:
   - Implemented `menew-core::i18n` with support for English (`en`), Portuguese (`pt`), German (`de`), Spanish (`es`), and Chinese (`zh`).
   - Automatic locale detection from system `LANG` and `LC_ALL` environment variables.

8. **Repository Files & Documentation**:
   - Created `README.md`, `CLAUDE.md`, `.gitignore`, and `LICENSE` (MIT).
   - Authored comprehensive technical documentation in `docs/spike-findings.md`.

---

## What Was Verified

All Rust checks were run on Linux (WSL Ubuntu-24.04, `CARGO_TARGET_DIR=~/target-menew`), where the `cfg(target_os = "linux")` code is actually compiled:

- `cargo check --workspace`: passes, 0 errors, 0 warnings.
- `cargo clippy --workspace --all-targets`: passes, 0 warnings.
- `cargo test --workspace`: 13 tests pass (core 7, clipboard 4, shortcuts 1, tray 1).
- `node --check gnome-extension/extension.js`: syntax OK.

Review-1 fixes: see `.relay/summary.md` for the item-by-item table. This round fixed the remaining compile errors (`format_result` re-export, `set_key_capture_widget` on the `SearchEntry`, missing `zbus` dependency in `menew`) and clippy findings, and added secret-filter tests.

---

## What Was Not Verified (Requires a Real Desktop Session)

- Rendering of GTK4/Libadwaita surfaces on physical Wayland displays.
- Visual focus grabbing behavior when presenting the palette from `xdg-activation-v1`.
- Actual GNOME Shell extension loading on a live GNOME session.
- StatusNotifierItem icon rendering in KDE Plasma and GNOME AppIndicator.
- Global shortcut portal registration, `wl-paste --watch` capture, and D-Bus delivery end to end.
- `cargo run -p menew` was not run in this round (no display in WSL).

---

## Next Steps

1. **Linux VM Testing**:
   - Test on clean Fedora 41 (GNOME 47) and Kubuntu 24.10 (KDE Plasma 6) virtual machines.
2. **Pasting Simulation**:
   - Implement the virtual paste injection helper (`/dev/uinput` or `RemoteDesktop` portal) for automatic Ctrl+V after selecting a clipboard entry.
3. **Palette Clipboard View**:
   - Add a tab or filter mode in the palette to browse and search clipboard history directly.
4. **Packaging Setup**:
   - Create AppImage and `.deb` packaging scripts.
