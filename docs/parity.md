# Parity with Menew for Mac

Source of truth: the Mac repo (`../menew`: `README.md`, `Sources/Menew/*.swift`). Every Mac feature is listed here with its Linux status. Update this file whenever a feature changes on either side.

Status: ✅ done · 🟡 partial · ⬜ to port · 🔁 port with a different technology · ❌ no Linux equivalent (explained)

**Release 1 ("core", decided with the maintainer on 2026-11-14)**: every row marked **R1** must be ✅ before the first public release carries the name Menew. Rows without R1 come in later releases.

Licence: GPL-3.0-only, the same as the Mac app; name and icon under `TRADEMARK.md`.

## Command palette (`Palette.swift`, `Calculator.swift`, `TextTransforms.swift`, `QuickLinks.swift`, `Emoji.swift`, `Timers.swift`, `ScriptCommands.swift`, `QuickReminders.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Open with a global shortcut (⌃⌥Space) | ✅ | 🟡 | R1 | GlobalShortcuts portal (KDE, GNOME 48+); fallback `menew toggle` as a system shortcut. Not tested on a real desktop. |
| Find and launch apps | ✅ | ✅ | R1 | `.desktop` files. |
| Recent picks first | ✅ | ⬜ | R1 | |
| Menew actions in the palette | ✅ | ⬜ | R1 | Every Linux action (quick actions, windows, screenshots...) must be reachable. |
| Math (`12*7.5`, `20% of 340`) | ✅ | ✅ | R1 | |
| Unit conversions (`5 km to mi`, `72 f to c`) | ✅ | ⬜ | R1 | |
| Currency conversion (`100 usd to brl`, fetched on ↩) | ✅ | ⬜ | R1 | The only network call, started by the user; same rule as the Mac. |
| Time zones (`3pm ny to lisbon`) and world clocks | ✅ | ⬜ | R1 | |
| Text transforms (case, accents, sort/dedupe, JSON, Base64, URL, counts) with live preview | ✅ | ⬜ | R1 | |
| Developer tools (JWT decode, UUID, Unix timestamp) | ✅ | ⬜ | R1 | |
| Quick links (`g`, `gh`, `maps`, `yt`, `wiki`, custom `{q}`), open typed addresses, web search | ✅ | ⬜ | R1 | |
| Emoji and symbols (`emoji rocket`, `:fire`, arrow/degree/euro) | ✅ | ⬜ | R1 | |
| Timers and alarms (`timer 10m tea`, `alarm 7:30`), countdown in the tray | ✅ | ⬜ | R1 | |
| Focus timer (`focus 50`) | ✅ | ⬜ | R1 | |
| Script commands (shell/Python/Node/Ruby, Raycast headers) | ✅ | ⬜ | R1 | AppleScript ❌. |
| Snippets in the palette | ✅ | ⬜ | R1 | |
| Notes in the palette | ✅ | ⬜ | R1 | |
| Quick reminders (`remind ...`) | ✅ | 🔁 | | No system Reminders app: Evolution Data Server (GNOME) / Akonadi (KDE), or Menew's own reminders with notifications. |
| Translation (offline) | ✅ | 🔁 | | Argos Translate / Bergamot (local). |
| Ask AI, natural-language commands (`do ...`), summarize/rewrite | ✅ | 🔁 | | Optional local AI via Ollama; off when not installed. |

## Clipboard (`ClipboardManager.swift`, `ClipboardViews.swift`, `PasteQueue.swift`, `SmartClip.swift`, `SmartClipSearch.swift`, `Snippets.swift`, `TextExpander.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Text history with search, pins | ✅ | ✅ | R1 | KDE via `wl-paste --watch`, GNOME via extension. |
| Images and files in the history, rich preview | ✅ | ⬜ | R1 | |
| Dedicated clipboard shortcut (⌘⇧V on Mac) | ✅ | 🟡 | R1 | `Ctrl+Alt+V` via portal; untested on a real desktop. |
| Paste automatically after picking | ✅ | 🟡 | R1 | uinput (opt-in udev rule) or copy + notification. Untested on a real desktop. |
| Skip passwords | ✅ | ✅ | R1 | `x-kde-passwordManagerHint`. |
| Ignore chosen apps | ✅ | ⬜ | R1 | Wayland doesn't say which app copied; the GNOME extension can (focused window), KDE best effort. |
| Pause for 15 min / 1 h | ✅ | ⬜ | R1 | |
| Filters (text, links, images, files) | ✅ | ⬜ | R1 | |
| Paste queue | ✅ | ⬜ | R1 | |
| Smart actions (phone, address, email, tracking code, color, meeting link) | ✅ | ⬜ | R1 | |
| Smart search (dates, types, word roots) | ✅ | ⬜ | | |
| Text inside copied images (OCR) | ✅ | 🔁 | R1 | Tesseract. |
| Password notes from the clipboard (Touch ID) | ✅ | 🔁 | | Secret Service (GNOME Keyring / KWallet) + polkit. |
| Snippets with variables (`{date}`, `{clipboard}`, `{cursor}`, `{input:Name}`) | ✅ | ⬜ | R1 | |
| Snippet abbreviations expanded while typing | ✅ | 🔁 | | Needs a global keyboard hook: not possible on Wayland without uinput/IBus; plan an IBus engine or skip. |

## Notes (`Notes.swift`, `NotesViews.swift`, `Markdown.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Many notes, #tags, search, Markdown with checklists | ✅ | ⬜ | R1 | |
| Protected notes | ✅ | 🔁 | R1 | Secret Service instead of the Keychain; unlock with the session password (polkit). |
| Password notes (hidden values, copy, cleared after 30 s) | ✅ | 🔁 | R1 | |
| Summarize / extract to-dos with AI | ✅ | 🔁 | | Ollama. |

## Screenshots and screen (`Capture.swift`, `TextRecognizer.swift`, `QRCode.swift`, `ScrollCapture.swift`, `ScreenAsk.swift`, `LiveText.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Capture area / window / screen with a shortcut | ✅ | 🔁 | R1 | `org.freedesktop.portal.Screenshot` (interactive). |
| Annotate (pen, arrows, rectangles, text, blur), copy, save, drag | ✅ | ⬜ | R1 | |
| Pin a screenshot floating on screen | ✅ | 🟡 | | A normal always-on-top window is not guaranteed on Wayland. |
| Copy text from the screen (OCR) | ✅ | 🔁 | R1 | Portal + Tesseract. |
| QR codes (read from screen, show for text/link/Wi-Fi) | ✅ | ⬜ | R1 | |
| Screen recording (video/GIF, with audio) | ✅ | 🔁 | | ScreenCast portal + PipeWire + GStreamer. |
| Scrolling capture | ✅ | ❌ | | Needs synthetic scrolling of another app; not possible on Wayland without uinput. |
| Ask about the screen (AI) | ✅ | 🔁 | | Ollama with a vision model. |
| Live Text on images, background removal | ✅ | 🔁 | | Tesseract; background removal via a local model, later. |

## Windows and files (`Windows.swift`, `DragSnap.swift`, `Shelf.swift`, `ImageTools.swift`, `PDFTools.swift`, `Organizer.swift`, `Profiles.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Snap the front window (halves, corners, thirds, full, other display) | ✅ | 🔁 | R1 | Wayland apps can't move other windows. KWin: scripting over D-Bus. GNOME: through the Menew extension. X11: EWMH. |
| Drag to snap with preview | ✅ | ❌ | | Both desktops already do edge snapping natively. |
| Quick shelf (shake to open, drag out) | ✅ | 🟡 | | A shelf window is possible; "shake while dragging" is not detectable on Wayland. |
| Image tools (convert, resize, rotate, strip metadata) | ✅ | ⬜ | | |
| PDF tools (combine, compress, rotate, split, extract text) | ✅ | ⬜ | | |
| Transcribe audio/video files | ✅ | 🔁 | | whisper.cpp. |
| Organize files with AI | ✅ | 🔁 | | Ollama; by type without it. |
| Work profiles (apps + window positions) | ✅ | 🔁 | | Launching apps yes; restoring positions only on KWin/extension. |

## Quick actions (`ActionsView.swift`, `Toolbox.swift`, `AppControl.swift`, `Ports.swift`, `Presentation.swift`, `Cleaning.swift`, `Hardware.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Mute microphone (global shortcut) / mute sound | ✅ | 🔁 | R1 | PipeWire/PulseAudio (`wpctl`). |
| Switch audio output and input | ✅ | 🔁 | R1 | PipeWire. |
| Dark mode toggle | ✅ | 🔁 | R1 | GNOME gsettings `color-scheme`; KDE `plasma-apply-colorscheme`. |
| Keep awake (for a time / until / while an app runs) | ✅ | 🔁 | R1 | `org.freedesktop.ScreenSaver.Inhibit` / portal Inhibit. |
| Lock screen | ✅ | 🔁 | R1 | `loginctl lock-session`. |
| Empty trash | ✅ | 🔁 | R1 | `gio trash --empty`. |
| Color picker | ✅ | 🔁 | R1 | `org.freedesktop.portal.Screenshot.PickColor`. |
| Plain-text clipboard | ✅ | ⬜ | R1 | |
| Eject disks | ✅ | 🔁 | R1 | UDisks2 over D-Bus. |
| Hidden files / desktop icons / hide Dock | ✅ | 🟡 | | Depends on the desktop; GNOME has no desktop icons by default. |
| Display brightness (built-in + DDC/CI) | ✅ | 🔁 | R1 | `org.freedesktop.login1.Session.SetBrightness` (backlight) + ddcutil (external). |
| Quit apps / force quit / never-quit list | ✅ | 🟡 | R1 | Close via the toolkit is not possible on Wayland; quit by signal (SIGTERM first, apps can ask to save only if they handle it). Force quit: SIGKILL. |
| Ports in use (stop a server) | ✅ | ⬜ | R1 | `/proc/net/tcp` + process. |
| Presentation mode | ✅ | 🔁 | | Inhibit + do not disturb + pause clipboard. |
| Cleaning mode (lock keyboard, black screens) | ✅ | 🟡 | | Black fullscreen windows yes; locking the keyboard needs an exclusive grab, not available on Wayland. |
| Automations ("when X, do Y") | ✅ | ⬜ | | |

## System health (`SystemMonitor.swift`, `MonitorViews.swift`, `SystemHistory.swift`, `Alerts.swift`, `AlertRules.swift`, `BatteryCare.swift`, `BatteryWear.swift`, `Sensors.swift`, `NetUsage.swift`, `WiFi.swift`, `Diagnosis.swift`, `RunawayWatch.swift`, `ScreenTime.swift`, `StartupItems.swift`, `Cleaner.swift`, `Uninstaller.swift`, `Accessories.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| CPU, GPU, memory, swap, disk, battery, network live | ✅ | 🔁 | R1 | `/proc`, `/sys`, UPower. GPU: vendor-specific (amdgpu/i915 sysfs, `nvidia-smi`). |
| Top processes (quit in one click) | ✅ | 🔁 | R1 | |
| Metrics in the tray (choose which) | ✅ | 🟡 | R1 | SNI tray shows one icon + tooltip/title; text in the panel only on KDE. |
| History 24 h / 7 d / 30 d with the apps that used the most | ✅ | ⬜ | | |
| Temperatures and fans | ✅ | 🔁 | | hwmon. |
| Battery health and wear history | ✅ | 🔁 | | UPower / `/sys/class/power_supply`. |
| Alerts (disk full, low battery, memory, CPU, heat) and custom rules | ✅ | ⬜ | | |
| "Why is my computer slow?" | ✅ | ⬜ | | |
| Internet (live speed, local and public IP, speed test) | ✅ | 🔁 | | The speed test needs a server; Mac uses Apple's `networkQuality`. Decide a provider or drop it. |
| Apps using the network | ✅ | 🔁 | | `/proc/net` + `ss`, or nethogs-like. |
| Wi-Fi details | ✅ | 🔁 | | NetworkManager over D-Bus. |
| Device batteries (headphones, mouse, keyboard) | ✅ | 🔁 | | UPower devices (BlueZ). |
| Screen time (per app, categories) | ✅ | 🟡 | | Wayland doesn't expose the focused app; only via the GNOME extension / KWin script. |
| Startup and background items | ✅ | 🔁 | | `~/.config/autostart`, systemd user units. |
| Free up space (caches, logs, large and duplicate files) | ✅ | 🔁 | | Same rules as the Mac: user files go to the trash, only caches/logs are deleted. |
| Uninstall apps and their leftovers | ✅ | 🟡 | | Package managers differ; Flatpak leftovers are feasible. |

## Recordings, memory, AI (`Recorder.swift`, `Recordings*.swift`, `Transcribe.swift`, `LiveCaptions.swift`, `Memory*.swift`, `AI*.swift`, `Rewriter.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Recordings (computer audio + mic, separate tracks) | ✅ | 🔁 | | PipeWire. |
| Offline transcription with "Me"/"Others" | ✅ | 🔁 | | whisper.cpp. |
| Minutes (summary, decisions, tasks) | ✅ | 🔁 | | Ollama. |
| Live captions (+ translation) | ✅ | 🔁 | | whisper.cpp streaming. |
| Memory (capture, OCR, source, search) | ✅ | 🔁 | | Portal screenshot + Tesseract + SQLite FTS. Browser tab URL is not available. |
| Ask your memory, knowledge graph | ✅ | 🔁 | | Ollama. |
| Rewrite selected text anywhere (⌃⌥⌘R) | ✅ | 🟡 | | Reading the selection: primary selection works; replacing it needs uinput paste. |

## Made for macOS → made for Linux

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Menu bar popover ("Now", "Actions" tabs) | ✅ | 🔁 | R1 | SNI menus can't host custom views. A small popover-like window opened from the tray (left click) with the same tabs. |
| Menew window (sidebar: Overview, Clipboard, Notes, Performance, Settings...) | ✅ | 🔁 | R1 | libadwaita `NavigationSplitView`. |
| Settings (incl. turning features off completely) | ✅ | ⬜ | R1 | |
| Welcome tour | ✅ | ⬜ | R1 | |
| 5 languages, follow system or pick | ✅ | 🟡 | R1 | Strings in 5 languages; no picker yet. |
| Backup / restore (`.menew` file) | ✅ | ⬜ | | Same format as the Mac where possible. |
| Updates | ✅ Sparkle | 🔁 | R1 | Package managers / AppImage update info; an opt-in check of GitHub releases (same rule as the Mac's update check). |
| Live notch | ✅ | ❌ | | No notch; the same alerts go to notifications and the tray. |
| Widgets | ✅ | ❌ | | No cross-desktop widget API (KDE Plasmoids could come later). |
| Siri, Shortcuts, Spotlight | ✅ | 🔁 | | D-Bus actions for scripting; GNOME Shell search provider and KRunner for search. |
| Continuity Camera | ✅ | ❌ | | Apple-only. KDE Connect could be explored later. |
| Focus filters | ✅ | 🔁 | | Follow the desktop's Do Not Disturb. |
| Next meeting with Join | ✅ | 🔁 | | Evolution Data Server (GNOME) / Akonadi (KDE). |
| Liquid Glass design | ✅ | ❌ | | libadwaita styling instead. |

## Release 1 summary

Done: app search, math, text clipboard with search/pins/password filter, tray basics, opt-in autostart, packaging (.deb verified; AppImage being fixed).

To build for R1 (grouped into relay phases in `docs/plan-mvp.md`):
1. Palette: recent picks, Menew actions, conversions, currency, time zones, text transforms, dev tools, quick links, emoji, timers/alarms/focus, script commands, snippets and notes inside it.
2. Clipboard: images and files, filters, pause, ignore apps (GNOME), paste queue, smart actions, OCR of images, snippets with variables.
3. Notes with protected and password notes.
4. Screenshots: capture, annotate, OCR, QR.
5. Windows: snapping via KWin script / GNOME extension.
6. Quick actions: mic/sound/output, dark mode, keep awake, lock, trash, color picker, plain text, eject, brightness, quit apps, ports.
7. System monitor with top processes and tray metrics.
8. Tray popover (Now / Actions), Menew window, settings, welcome tour, language picker, update check.
