# Menew for Linux: Spike Technical Findings & Architecture Analysis

This document details the architectural findings, compositor limits, protocol behaviors, and design choices established during the Menew for Linux spike phase.

## 1. Executive Summary

Building a seamless system utility (command palette, clipboard history, global shortcuts, and status tray) on modern Linux desktops requires navigating the distinct architectural approaches of **GNOME (Mutter)** and **KDE Plasma (KWin)** on Wayland.

The primary conclusion of this spike is that **a single unified Wayland protocol does not yet exist for all background utility tasks**. Instead, a hybrid strategy is required:
- On **KDE Plasma**, standard modern protocols (`ext-data-control`, `GlobalShortcuts` portal, native StatusNotifierItem) allow Menew to function natively without extra desktop plugins.
- On **GNOME**, because Mutter declines to support background clipboard protocols (`data-control`) or layer-shell, Menew uses a standard Libadwaita window paired with a lightweight GNOME Shell extension acting as a D-Bus clipboard bridge.

---

## 2. Desktop Environment Comparison Matrix

| Capability | GNOME (Wayland) | KDE Plasma (Wayland) | Menew Strategy |
| :--- | :--- | :--- | :--- |
| **Command Palette Window** | Floating GTK4/Libadwaita window (`xdg-shell`) | Floating window or optional layer-shell | Standard GTK4/Libadwaita window with `xdg-activation-v1` |
| **Layer Shell (`wlr-layer-shell`)** | **Unsupported** (rejected by Mutter) | Supported natively | Avoid relying on layer-shell; use standard window |
| **Background Clipboard Monitoring** | **Blocked** (no data-control protocol) | **Supported** via `ext-data-control-v1` | GNOME Shell extension on GNOME; `wl-paste --watch` helper on KDE |
| **Global Shortcuts** | Supported on GNOME 48+; missing on 45-47 | Supported natively (Plasma 5.27+ and 6) | `ashpd` GlobalShortcuts with CLI fallback (`menew toggle`) |
| **System Tray** | Requires `AppIndicator` extension | Supported natively (StatusNotifierItem) | `ksni` (SNI spec over D-Bus) |
| **Pasting Keystroke Simulation** | Blocked across clients; requires portal/uinput | Blocked across clients; requires portal/uinput | `/dev/uinput` or `RemoteDesktop` portal (`libei`) |

---

## 3. Deep Dive: Clipboard Management on Wayland

### The Wayland Security Model
Under Wayland, client isolation prevents arbitrary background applications from reading clipboard contents. Standard GUI toolkits use `wl_data_device`, which delivers data offers only to the surface currently holding user focus.

### KDE Plasma / wlroots Compositors
KDE Plasma 6 (and 5.27+) implements the `ext-data-control-v1` protocol and the legacy `zwlr_data_control_manager_v1`. These protocols allow authorized background processes to create a selection listener without holding surface focus.
- **Implementation in Spike**: Menew spawns `wl-paste --type text/plain --watch menew __clipboard-push`.
  - Binary-safe: stdin receives the complete copied text, preserving multi-line blocks without splitting lines.
  - MIME-filtered: `--type text/plain` ignores images and non-text formats automatically.
  - Password protection: Before pushing to the daemon, the helper inspects `wl-paste --list-types` for `x-kde-passwordManagerHint` and `org.nspasteboard.ConcealedType`. If present, the entry is discarded immediately.

### GNOME Mutter
Upstream GNOME does not implement `data-control` protocols in Mutter. As a result:
- Standalone external tools (like `wl-paste --watch`) cannot capture selections on standard GNOME Wayland sessions without focused surfaces.
- **The Solution**: GNOME Shell extensions run directly inside the compositor process with unconstrained access to `Meta.Selection` and `St.Clipboard`.
- In this spike, we created `gnome-extension/`, which hooks the `owner-changed` event on `Meta.SelectionType.SELECTION_CLIPBOARD`, inspects `source.get_mimetypes()` to reject password manager entries (`x-kde-passwordManagerHint`, `ConcealedType`), and dispatches the new payload to Menew over session D-Bus (`org.menew.ClipboardService`).
- **Validation note**: The D-Bus bridge design allows push notifications without polling; live timing and compositor behavior will be confirmed during Linux VM testing.

---

## 4. Deep Dive: Global Keyboard Shortcuts

### The `org.freedesktop.portal.GlobalShortcuts` Portal
The modern, secure Wayland mechanism for global keyboard shortcuts is the XDG Desktop Portal interface (`ashpd` crate).

### Support Across Desktops
- **KDE Plasma**: Supported in Plasma 5.27 and Plasma 6 via `xdg-desktop-portal-kde`. The user receives a system dialog to confirm the shortcut binding.
- **GNOME**: The `GlobalShortcuts` portal backend was introduced in GNOME 48 (`xdg-desktop-portal-gnome`). On GNOME 45, 46, and 47 (such as Ubuntu 24.04 LTS and Fedora 40/41), the portal is unavailable.

### Default Shortcut Strategy
On macOS, Menew uses `Ctrl+Opt+Space`. On Linux:
- `Super+Space` is GNOME's system default for switching keyboard input layouts. Overriding it creates friction.
- Menew sets its default preferred trigger to **`Ctrl+Alt+Space`**.
- When `ashpd` fails on GNOME 45-47, Menew logs clear, actionable instructions guiding the user to add a Custom Shortcut in GNOME Settings (`gnome-control-center` -> Keyboard -> View and Customize Shortcuts -> Custom Shortcuts):
  - **Name**: Menew Command Palette
  - **Command**: `menew toggle`
  - **Shortcut**: `Ctrl+Alt+Space`

---

## 5. Deep Dive: Window Presentation and Layer Shell

### Why `gtk4-layer-shell` is Non-Viable for GNOME
Initial plans often consider `gtk4-layer-shell` (which uses `zwlr_layer_shell_v1`) to create overlay windows like Spotlight. However, GNOME Mutter does not implement `wlr-layer-shell`. Linking against `gtk4-layer-shell` makes the application unable to map surfaces on standard GNOME Wayland sessions.

### The Recommended Architecture
Menew uses a standard `adw::ApplicationWindow`:
- Modal: false, Decorated: false.
- Default size: 640x440 px, styled with custom CSS (`.menew-palette`).
- Top-level Search Entry with auto-focus (`entry.grab_focus()`).
- Keyboard control using `PropagationPhase::Capture` on `EventControllerKey` so that `Escape`, `Return`, and arrow navigation are intercepted before child widget consumption.
- Single-instance lifecycle: GApplication handles `startup` only on the primary instance, and remote invocations (`menew toggle`) trigger `palette.toggle()` via GApplication command-line forwarding.
- Re-entrancy protection: RefCell borrows are dropped before invoking `hide()`, preventing panics from immediate `search-changed` signals.

---

## 6. Deep Dive: System Tray (StatusNotifierItem)

The Linux system tray standard is StatusNotifierItem (SNI), operated over D-Bus:
- Implemented in Rust via the `ksni` crate (`menew-tray`).
- **KDE Plasma**: SNI is native; Menew's tray icon and menu appear immediately in the system tray area.
- **GNOME**: Requires `gnome-shell-extension-appindicator` (pre-installed on Ubuntu; requires `sudo dnf install gnome-shell-extension-appindicator` on Fedora).
- Menew provides dedicated SVG and symbolic icons installed to `hicolor` (`data/icons/hicolor/scalable/apps/menew-symbolic.svg`), preventing missing icon warnings.

---

## 7. Deep Dive: Keystroke Simulation and Pasting (Ctrl+V)

When a user selects an entry in clipboard history, Menew should paste it directly into the target active window, matching the Mac experience.

Under Wayland, an application cannot synthesize input events into an arbitrary client window without specific privileges:
1. **`org.freedesktop.portal.RemoteDesktop` (`libei`)**: Standards-compliant, but triggers a system authorization dialog for remote control.
2. **Linux Kernel `/dev/uinput`**: Fast, reliable across all Wayland and X11 desktops, but requires write permission to `/dev/uinput` via a standard udev rule.
3. **GNOME Shell Extension**: Can synthesize keystrokes directly using Mutter's internal `Clutter.VirtualInputDevice`.

### Recommendation for MVP
Support putting selected content on the system clipboard (`gdk::Clipboard::set_text`), paired with an optional `/dev/uinput` virtual keyboard helper when permissions permit.

---

## 8. Decisions Recorded

1. **i18n Evolution**: The hand-written string lookup in `menew-core::i18n` satisfies the spike requirements for 5 languages (en, pt, de, es, zh), but will be migrated to GNU gettext (`gettext-rs`) or Mozilla Fluent (`fluent-rs`) prior to the alpha release to allow standard translation workflows.
2. **Packaging**: AppImage, `.deb`, and AUR are the primary targets; Flatpak sandboxing is deferred due to system-wide clipboard and input requirements.
3. **Local AI**: Connect to an already installed Ollama instance over HTTP REST (`localhost:11434`) rather than embedding heavyweight C++ engines.


## GNOME Shell version support (extension)

`gnome-extension/metadata.json` declares `shell-version` 45 to 51 (GNOME 50 and 51 added after review). The extension uses `Meta.Selection` `owner-changed` (with the `source` argument), `source.get_mimetypes()` and `St.Clipboard.get_text`. These APIs were **not confirmed against the GNOME 50/51 gnome-shell or mutter sources** in this round (no network lookup or live session was done); they are used the same way as in GNOME 45-49. Treat 50/51 support as unverified until the extension is loaded on a real GNOME 50/51 session. References to check: gnome-shell `js/ui` and mutter `src/meta/meta-selection.h` / `meta-selection-source.h` in the 50 and 51 branches.

## Clipboard watcher lifecycle (KDE)

The `wl-paste --watch` child is owned by `WatcherGuard`, which kills and reaps it on drop. The primary instance drops it in `connect_shutdown`, and any other exit path drops it with the process state. A SIGKILL of the daemon cannot run `Drop`, so a watcher could still be orphaned in that case (not handled).

The helper `__clipboard-push` checks MIME types with a second `wl-paste --list-types` call, which can race with a new copy (acceptable for the spike).


## Phase 1 desktop tests

Headless sessions in WSL Ubuntu 24.04 (GNOME Shell 46 `--headless --virtual-monitor`, KWin 5.27 `--virtual`). Scripts: `scripts/desktop-test/`. Packages installed in WSL: gnome-shell, gnome-shell-extensions, mutter, kwin-wayland, kde-spectacle, qtwayland5, wl-clipboard, xdg-desktop-portal(-gnome/-kde), sqlite3, grim, imagemagick, dbus-user-session, dbus-x11, libglib2.0-bin, python3-yaml, procps, psmisc (plus their dependencies).

Results:
- **GNOME**: the extension loads on GNOME Shell 46 and the clipboard path (extension -> `org.menew.ClipboardService` -> SQLite) works. Palette opens with `menew toggle` and renders.
- **KDE**: `wl-paste --watch` captures single and multi-line text exactly (1 entry for a multi-line copy); tray registers; no `wl-paste` left after `menew quit`; palette renders.
- **GlobalShortcuts portal**: `Unsupported` on both (no backend implementing `org.freedesktop.portal.GlobalShortcuts` headless; the fallback guidance is printed). Not verified on a real GNOME 48+ or Plasma 6 session.
- **Tray on GNOME**: `Could not start tray service`, because plain GNOME has no StatusNotifierWatcher (expected, needs the AppIndicator extension).

Product bugs found by the tests: **none** in the runtime code. The only product change is the test hook `MENEW_DB_PATH` (overrides the database path in `ClipboardDatabase::default_db_path`).

Environment limitations (not product bugs):
1. Headless GNOME Shell has no keyboard, so `wl-copy`/`wl-paste` fail ("This seat has no keyboard") or hang; real `wl-copy` is therefore untested on GNOME. The test copies from inside the shell.
2. `gnome-extensions` CLI does not work headless (`org.gnome.Shell.Extensions` is not exported); state is read via `org.gnome.Shell.Eval`.
3. `grim` fails on KWin 5.27 (no wlr-screencopy); `spectacle` with `QT_QPA_PLATFORM=wayland` works.
4. A real two-type (text + password hint) selection could not be created headless; GNOME verifies the extension's hint branch, KDE verifies the helper with a stub `wl-paste`.
5. The headless shell starts in the Overview; the script hides it before the screenshot.
6. Still unverified: the extension on GNOME 45, 47-51 (only 46 tested).
