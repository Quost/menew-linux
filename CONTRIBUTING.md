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

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org), in English: `feat(clipboard): …`, `fix(palette): …`, `docs: …`, `ci: …`.

## Guidelines

- **Parity first.** Before adding or changing a feature, check how the Mac app does it and update `docs/parity.md`.
- **Keep it native and light.** No new dependencies unless there's a very good reason.
- **Privacy first.** No telemetry, no network calls the user didn't ask for.
- **Every UI string in all 5 languages** (en, pt, de, es, zh) in `crates/menew-core/src/i18n.rs`.
- **Test on GNOME and KDE**, light and dark.
- **One change per pull request**, with a screenshot if it touches the UI.

## Reporting bugs

Open an issue with your distribution, desktop (GNOME/KDE, Wayland/X11), Menew version (`menew --version`) and the steps to reproduce.
