# Contributing to Menew for Linux

Thanks for wanting to help! This is the Linux version of [Menew](https://github.com/Quost/menew), written in Rust with GTK4 and libadwaita. The goal is parity with the Mac app wherever Linux allows it: see [docs/parity.md](docs/parity.md).

Please follow our [code of conduct](CODE_OF_CONDUCT.md). Found a security problem? Report it privately as described in [SECURITY.md](SECURITY.md), not in a public issue.

By contributing you agree that your contribution is licensed under the [GPL-3.0](LICENSE), like the rest of the project.

## Getting started

Requirements: a Linux distribution with GTK 4 and libadwaita 1.2+ development packages, and a Rust toolchain.

```bash
# Debian/Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libdbus-1-dev wl-clipboard
git clone https://github.com/Quost/menew-linux
cd menew-linux
cargo run -p menew
```

## Tests and checks

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/desktop-test/gnome.sh   # headless GNOME session
bash scripts/desktop-test/kde.sh     # headless KWin session
```

The desktop tests need the compositors installed; see `scripts/desktop-test/README.md`. CI runs all of them on every push.

## Project layout

A Cargo workspace:

- `crates/menew`: main binary and coordinator.
- `crates/menew-core`: pure Rust logic (the palette engine and one module per Mac file: calculator, conversions, currency, time zones, text transforms, quick links, emoji, timers, focus, script commands, recents), the `.desktop` scanner, SQLite storage (`rusqlite`) and the i18n dictionary. Its palette behaviour is specified by `docs/palette-spec.md`, whose test vectors are run by the tests.
- `crates/menew-ui`: the palette window, GTK4 and libadwaita.
- `crates/menew-tray`: tray icon through StatusNotifierItem (`ksni`).
- `crates/menew-shortcuts`: global shortcuts through `org.freedesktop.portal.GlobalShortcuts` (`ashpd`).
- `crates/menew-clipboard`: clipboard history, KDE Wayland data-control and the GNOME D-Bus bridge.
- `gnome-extension`: the GNOME Shell extension that bridges Wayland clipboard events to the daemon.

## Running it

```bash
cargo build
cargo run -p menew     # starts hidden in the tray; the palette is NOT shown on launch
```

```bash
menew            # first launch: start the daemon hidden (tray only); if already running, shows the palette
menew toggle     # show/hide the palette (bind this to a custom system shortcut on GNOME 45-47)
menew clipboard  # open the palette in clipboard mode (bind this to Ctrl+Alt+V on GNOME 45-47)
menew paste-next # paste the next item of the paste queue
menew quit       # quit the running daemon
menew --version
```

`~/.config/menew/config.json` is optional (defaults are used when it is missing or invalid). `{ "paste_mode": "auto" }` injects Ctrl+V through `/dev/uinput` when it is writable and otherwise copies and notifies; `"copy-only"` never injects.

## Desktop tests (headless GNOME and KDE)

```bash
bash scripts/desktop-test/gnome.sh
bash scripts/desktop-test/kde.sh
```

See `scripts/desktop-test/README.md` for packages, what is real and what is substituted. Their screenshots (fake data) are in `docs/desktop-tests/`.

## README screenshots

The images in `docs/assets/` (English) and `docs/assets/<lang>/` (pt, de, es, zh) are made by the app itself, from fake data only, like the Mac app's `--snapshot`:

```bash
cargo build
bash scripts/snapshots.sh            # all languages, in a headless GNOME session
bash scripts/snapshots.sh en pt      # only some
menew --snapshot out -language de    # one language, in your own session
```

Needs Cantarell, the Adwaita icons, Noto Color Emoji and, for Chinese, a CJK font (`fonts-noto-cjk`). The script checks that nothing of the machine was read or changed. Which screens exist and how to add one: `docs/snapshots.md`. Regenerate them when a screen changes.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org), in English: `feat(clipboard): …`, `fix(palette): …`, `docs: …`, `ci: …`.

## Guidelines

- **Parity first.** Before adding or changing a feature, check how the Mac app does it and update `docs/parity.md`.
- **Keep it native and light.** No new dependencies unless there's a very good reason.
- **Privacy first.** No telemetry, no network calls the user didn't ask for.
- **Every UI string in all 5 languages** (en, pt, de, es, zh) in `crates/menew-core/src/i18n.rs`.
- **Test on GNOME and KDE**, light and dark.
- **One change per pull request**, with a screenshot if it touches the UI (and regenerated `docs/assets` if a screen changed).

## Reporting bugs

Open an issue with your distribution, desktop (GNOME/KDE, Wayland/X11), Menew version (`menew --version`) and the steps to reproduce.
