# Plan: from spike to alpha

The orchestrator (Claude Opus) plans and reviews. Sonnet executes through agent-relay, one relay cycle per phase (at most 3 iterations each).

Decisions made with the maintainer (2026-11-14):
- There are no GNOME/KDE VMs: we test on **headless compositors inside WSL Ubuntu 24.04** (`gnome-shell --headless`, `kwin_wayland --virtual`). The executor may install packages in WSL as root (`wsl -u root apt install ...`).
- Publication goes **up to an alpha on GitHub** (pre-release with AppImage/.deb). The `/linux` page on the site gets built, but **is not pushed** until the maintainer approves.

## Phase 1: real desktop tests (headless)
Automated scripts in `scripts/desktop-test/` that bring up a GNOME session and a KDE session in WSL and verify, with real evidence:
- the GNOME extension loads and pushes copies to the daemon over D-Bus (a `wl-copy` shows up in SQLite);
- on KDE, the `wl-paste` watcher captures copies (text and multi-line); content marked as a password is ignored;
- the tray registers with a StatusNotifierWatcher;
- the palette opens with `menew toggle` and renders (screenshot);
- the GlobalShortcuts portal: try it and record the result.
Plus CI on GitHub Actions: check/clippy/test on ubuntu-latest.

## Phase 2: functional MVP
- Clipboard history inside the palette (mode/prefix, search, Enter copies, pin and delete).
- Optional paste via uinput (when the user has permission), with a fallback to just copying plus a notice.
- Tray: open the palette, clear history, quit. Autostart (`.desktop` in `~/.config/autostart`, opt-in).
- i18n for the new strings in all 5 languages.

## Phase 3: packaging and alpha
- `.desktop` file, AppStream metainfo, icons.
- `.deb` (cargo-deb) and AppImage, built by GitHub Actions on a tag.
- `CHANGELOG.md`; tag `v0.1.0-alpha.1` as a GitHub **pre-release**.

## Phase 4: `/linux` page on the site (Mac repo, `../menew`)
- Following `../menew/CLAUDE.md`: `site/` template + i18n in 5 languages, `build-site.py`, sitemap, hreflang. Linux screenshots come from the headless GNOME with **fake data**.
- A local commit on a branch. **No push to main** (that deploys the site) without approval.
