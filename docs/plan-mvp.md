# Plan: from spike to alpha

The orchestrator (Claude Opus) plans and reviews. Sonnet executes through agent-relay, one relay cycle per phase (at most 3 iterations each).

Decisions made with the maintainer (2026-11-14):
- There are no GNOME/KDE VMs: we test on **headless compositors inside WSL Ubuntu 24.04** (`gnome-shell --headless`, `kwin_wayland --virtual`). The executor may install packages in WSL as root (`wsl -u root apt install ...`).
- Publication goes **up to an alpha on GitHub** (pre-release with AppImage/.deb). The `/linux` page on the site gets built, but **is not pushed** until the maintainer approves.

## Phase 1: real desktop tests (headless) - DONE
Automated scripts in `scripts/desktop-test/` that bring up a GNOME session and a KDE session in WSL and verify, with real evidence:
- the GNOME extension loads and pushes copies to the daemon over D-Bus (a `wl-copy` shows up in SQLite);
- on KDE, the `wl-paste` watcher captures copies (text and multi-line); content marked as a password is ignored;
- the tray registers with a StatusNotifierWatcher;
- the palette opens with `menew toggle` and renders (screenshot);
- the GlobalShortcuts portal: try it and record the result.
Plus CI on GitHub Actions: check/clippy/test on ubuntu-latest.

## Phase 2: functional MVP - DONE
- Clipboard history inside the palette (mode/prefix, search, Enter copies, pin and delete).
- Optional paste via uinput (when the user has permission), with a fallback to just copying plus a notice.
- Tray: open the palette, clear history, quit. Autostart (`.desktop` in `~/.config/autostart`, opt-in).
- i18n for the new strings in all 5 languages.

## Phase 3: packaging (no release yet)
- `.desktop` file, AppStream metainfo, icons.
- `.deb` (cargo-deb) and AppImage, built by GitHub Actions on a tag.
- `CHANGELOG.md` and the release workflow.
- **Changed on 2026-11-14:** the maintainer decided that the first public release must have the **core of the Mac app** (every row marked R1 in `docs/parity.md`). The packaging is kept and tested, but **no tag or release** is made until R1 is complete. The license is GPL-3.0-only, like the Mac app.

## Phases 4 to 11: release 1 ("core" parity) - DONE (2026-11-15), waiting for a real desktop pass
One relay cycle each, reviewed against the Mac app (`../menew`) and `docs/parity.md`:
4. Palette: recent picks, Menew actions, conversions, currency, time zones, text transforms, dev tools, quick links, emoji, timers/alarms/focus, script commands.
5. Clipboard: images and files, filters, pause, ignore apps, paste queue, smart actions, OCR of images, snippets with variables.
6. Notes, with protected and password notes (Secret Service).
7. Screenshots: capture (portal), annotate, OCR, QR.
8. Windows: snapping via a KWin script / the GNOME extension.
9. Quick actions: mic/sound/output, dark mode, keep awake, lock, trash, color picker, plain text, eject, brightness, quit apps, ports.
10. System monitor with top processes and tray metrics.
11. Tray popover (Now / Actions), Menew window, settings, welcome tour, language picker, update check. Then tag the first release.

Done on 2026-11-15 from a Mac, with the Docker image of `scripts/dev/` (Ubuntu 24.04, headless GNOME
Shell 46 and KWin 5.27) instead of WSL. Order used: 9 (quick actions and windows), 6, 7, 10, 11, then
the focus options. Every phase has unit tests in `menew-core` and the release 1 checks in
`scripts/desktop-test/r1-checks.sh` and `r1-kde-checks.sh`.

## Before the first tag

1. A pass on a real GNOME 46+ and a real KDE Plasma 6 desktop: global shortcuts (portal dialog), tray
   left click, paste with uinput, Screenshot portal and color picker, polkit dialog and the keyring
   for protected notes, audio devices, brightness, eject, window snapping, dark mode.
2. Check the AppImage (`scripts/test-appimage-docker.sh`) and the .deb (polkit policy installed).
3. Tag `v0.1.0`; then Phase 12.

## Phase 12: `/linux` page on the site (Mac repo, `../menew`), after release 1
- Following `../menew/CLAUDE.md`: `site/` template + i18n in 5 languages, `build-site.py`, sitemap, hreflang. Linux screenshots come from the headless GNOME with **fake data**.
- A local commit on a branch. **No push to main** (that deploys the site) without approval.
