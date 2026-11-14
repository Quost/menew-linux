# Handoff: Menew for Linux, spike phase

You are starting the Linux version of **Menew**, the macOS menu bar app in `../menew` (Swift/SwiftUI, MIT, https://menew.web.app). Read `../menew/README.md` and `../menew/CLAUDE.md` to understand the product and its rules. None of the Swift code can be reused; this is a new Rust codebase in this folder (`menew-linux`, a separate repo).

## Decisions already made (don't reopen them)

- Separate repo: this folder. Run `git init` and use the `main` branch. Don't push anything yet.
- Stack: **Rust + GTK4 + libadwaita** (`gtk4-rs`, `libadwaita-rs`). No Tauri, no Electron.
- Targets: **GNOME and KDE Plasma on Wayland**. X11 is best effort only.
- Rules inherited from the Mac app: **no telemetry or analytics, ever**. No network calls except ones the user explicitly starts. Keep dependencies minimal. Conventional Commits in English. Every UI string must be translatable (en, pt, de, es, zh), even if only `en`/`pt` are filled in for now.
- The `/linux` page on the website comes **later**, once there is a downloadable alpha. Don't touch `../menew`.

## Corrections to the earlier plan (important)

- `gtk4-layer-shell` **doesn't work on GNOME** (Mutter has no wlr-layer-shell). The palette must be a normal window that works on GNOME. layer-shell can be an optional path for KDE/wlroots only.
- **Clipboard on GNOME Wayland:** Mutter doesn't expose `data-control`, and an unfocused app can't read the clipboard. Background clipboard history on GNOME probably needs a **GNOME Shell extension** that talks to the app over D-Bus. On KDE, `ext-data-control`/`wlr-data-control` works. Validate this first, because it decides the architecture.
- **Pasting into the target app** (simulated Ctrl+V) on Wayland needs the RemoteDesktop portal or uinput. Investigate this and document it.
- The tray (StatusNotifierItem, `ksni` crate) needs the AppIndicator extension on GNOME. Ubuntu ships it, Fedora doesn't.
- The `GlobalShortcuts` portal exists on KDE and on GNOME ≥ 48. Super+Space is GNOME's layout-switch shortcut, so use a different default (e.g. Ctrl+Alt+Space, like the Mac app).
- Packaging (later): AppImage + .deb + AUR first. Flatpak's sandbox conflicts with this kind of utility.
- Local AI (later): optional integration with an already installed Ollama. Don't embed llama.cpp.

## Spike scope (what to build now)

Goal: prove the risky pieces work, not deliver features. One Cargo workspace with:

1. **Tray** with a menu (Open palette, Quit) via `ksni`.
2. **Global shortcut** via the `org.freedesktop.portal.GlobalShortcuts` portal (`ashpd` crate). If the portal isn't available, show a clear message.
3. **Palette window** (GTK4/libadwaita): a search field plus a list. It opens and grabs focus on the shortcut, closes with Esc, and lists apps from `.desktop` files (`/usr/share/applications`, `~/.local/share/applications`, Flatpak exports) and launches them.
4. **Clipboard (proof of concept):** on KDE, a data-control listener that keeps history in SQLite (`rusqlite`). On GNOME, write down the conclusion (extension vs. alternative) in `docs/spike-findings.md`. A minimal extension stub is a plus.
5. `docs/spike-findings.md`: what works and what doesn't on each desktop, the limits found, and a recommended MVP architecture.
6. `README.md` (short), `CLAUDE.md` with the project rules above, `.gitignore`, MIT `LICENSE`.

## Environment

The maintainer is on **Windows**. GTK, portals and the tray can't be tested here. Make the code correct for Linux and run `cargo check` if a Linux toolchain is available (e.g. WSL). Otherwise say clearly what has not been compiled or tested. Real testing will happen on a GNOME and a KDE VM.

When you finish, summarize what was done, what wasn't verified, and the next steps.

## Hand back for review (required)

When you finish (or if you get blocked), write your summary to `docs/spike-summary.md` and hand the work back to Claude for review. Run this in a shell:

```sh
cockpit send --tab-id t57 --enter "agy finished the menew-linux spike. Please review ../menew-linux (start with docs/spike-summary.md and docs/spike-findings.md)."
```

If `t57` no longer exists, run `cockpit list-tabs` and use the tab titled "Plano de port do Menew para Linux". Don't commit until the review is done.
