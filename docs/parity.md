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
| Recent picks first | ✅ | ✅ | R1 | Last 20 items, most recent first, +60 score decaying by 3 (same as the Mac); stored as ids in `~/.local/share/menew/recents.json`. `recents.rs`. |
| Menew actions in the palette | ✅ | 🟡 | R1 | Generic `PaletteItem` / `ItemAction` list in `palette.rs`. Already there: clipboard history, clear history, start at login, start/stop/+5 min focus, generate UUID, copy Unix timestamp, open scripts folder, quit. The actions of later phases (quick actions, windows, screenshots, notes, snippets...) plug into the same list when they exist. The smart items for the copied text (phone, address, color, tracking code, meeting link) show first when the search is empty and by keyword when typing. Clipboard actions: pause or resume, paste queue, paste next. |
| Math (`12*7.5`, `20% of 340`) | ✅ | ✅ | R1 | Own parser like the Mac: `+ - * / ^ x × ÷ % ! √`, `pi`, `e`, `sqrt sin cos tan log ln abs round`, `2(3+1)`, `200+10%`. Decimal comma or point; result uses the language's decimal separator. `calculator.rs`. |
| Unit conversions (`5 km to mi`, `72 f to c`) | ✅ | ✅ | R1 | Same unit list and aliases as the Mac (length, mass, volume, data, time, speed, area, temperature) and the same connectors in 5 languages. `conversions.rs`. |
| Currency conversion (`100 usd to brl`, fetched on ↩) | ✅ | ✅ | R1 | Same 24 currencies and provider (ECB through `api.frankfurter.app`), fetched only on Enter, cached 6 h. The request goes through the system `curl` (no HTTP/TLS crate in the tree). Currency text (`$100.00`, `R$ 100,00`, `100,00 €`) follows hand-written locale rules instead of Foundation's. `currency.rs`. |
| Time zones (`3pm ny to lisbon`) and world clocks | ✅ | 🟡 | R1 | Time zones: same aliases (pt, en, de, es, zh) and cities of the tz database, DST through `date` with `TZ` (needs tzdata and coreutils). World clocks: the list is in `~/.config/menew/config.json` (`world_clocks`) and typing `clocks` shows them in the palette; the card at the top of the Now tab and the settings window come in Phase 11. `time_zones.rs`. |
| Text transforms (case, accents, sort/dedupe, JSON, Base64, URL, counts) with live preview | ✅ | ✅ | R1 | All 20 transforms, with the same keywords in 5 languages and the preview (first 200 characters, `⏎` between lines, none above 100,000 bytes). Applies to the clipboard text read by GTK while the palette is open, else the newest history entry. Word counts use letter and digit runs, not Apple's word breaker. `text_transforms.rs`. |
| Developer tools (JWT decode, UUID, Unix timestamp) | ✅ | ✅ | R1 | JWT (with `exp`/`iat`/`nbf` dates), UUID v4, Unix timestamp now, and a pasted 10 or 13 digit timestamp shown as a date (Enter copies the ISO date). The long date text is written by hand per language and can differ from macOS. `dev_tools.rs`. |
| Quick links (`g`, `gh`, `maps`, `yt`, `wiki`, custom `{q}`), open typed addresses, web search | ✅ | ✅ | R1 | Same six defaults (`maps` opens OpenStreetMap: the Mac uses its native Apple Maps, Linux has no native map app, and copied addresses also open in OpenStreetMap; edit `quick_links` in `config.json` to use another map). Custom links live in `~/.config/menew/config.json` until the settings window (Phase 11). Opens with the default browser. `quick_links.rs`. |
| Emoji and symbols (`emoji rocket`, `:fire`, arrow/degree/euro) | ✅ | ✅ | R1 | Same filter and word table as the Mac; names come from the Unicode data of the build host (`emoji_data.rs`, generated). The Apple logo symbol is left out. Pasting uses the same path as the clipboard (uinput or copy + notification). `emoji.rs`. |
| Timers and alarms (`timer 10m tea`, `alarm 7:30`), countdown in the tray | ✅ | ✅ | R1 | Countdown in the StatusNotifierItem `Title` and `ToolTip` (KDE shows the tooltip; GNOME depends on the AppIndicator extension). At the end: a notification and the sound of the freedesktop theme (`canberra-gtk-play`, else `paplay`, else a beep). In memory only. The Mac's HUD toasts ("Copied: x", "Timer until 9:10") are desktop notifications here. `timers.rs`, `crates/menew/src/alerts.rs`. |
| Focus timer (`focus 50`) | ✅ | 🟡 | R1 | Same flow: 25 min default, `focus N` (1 to 600), +5 min, stop, 5 min break (`focus_breaks` in `config.json`), notification and sound at the end, countdown in the tray. The options that mute the microphone, keep the computer awake or pause the clipboard while focusing are the `FocusHooks` trait, empty today: they come with the quick actions (Phase 9) and the clipboard pause (Phase 5). `focus.rs`. |
| Script commands (shell/Python/Node/Ruby, Raycast headers) | ✅ | ✅ | R1 | Folder `~/.local/share/menew/scripts`, "Open scripts folder" creates it with an example, Raycast (and `@menew.`) headers: title, icon, mode (silent, compact, fullOutput, inline) and `argument1` as the text after the title. Runs with a 2 minute limit, `~/.local/bin` and `/usr/local/bin` added to `PATH`. AppleScript files are ignored (❌: no Linux equivalent). `script_commands.rs`. |
| Snippets in the palette | ✅ | ✅ | R1 | Same items as the Mac (title, preview, keyword `snippet` plus the text). `palette.rs`. |
| Notes in the palette | ✅ | ⬜ | R1 | |
| Quick reminders (`remind ...`) | ✅ | 🔁 | | No system Reminders app: Evolution Data Server (GNOME) / Akonadi (KDE), or Menew's own reminders with notifications. |
| Translation (offline) | ✅ | 🔁 | | Argos Translate / Bergamot (local). |
| Ask AI, natural-language commands (`do ...`), summarize/rewrite | ✅ | 🔁 | | Optional local AI via Ollama; off when not installed. |

## Clipboard (`ClipboardManager.swift`, `ClipboardViews.swift`, `PasteQueue.swift`, `SmartClip.swift`, `SmartClipSearch.swift`, `Snippets.swift`, `TextExpander.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Text history with search, pins | ✅ | ✅ | R1 | KDE via `wl-paste --watch`, GNOME via extension. |
| Images and files in the history, rich preview | ✅ | ✅ | R1 | Images are kept as files in `~/.local/share/menew/clipboard/` (named by the SHA-256 of the data, like the Mac), files as paths from `text/uri-list`. List rows have a thumbnail or the file icon, and a preview pane shows the image, the files, a color swatch, the link or the text with its stats. Images keep their original type (the Mac converts to PNG). 20 MB limit per copy (the Mac has none). `clip_item.rs`, `clipboard_db.rs`, `crates/menew-ui`. |
| Dedicated clipboard shortcut (⌘⇧V on Mac) | ✅ | 🟡 | R1 | `Ctrl+Alt+V` via portal; untested on a real desktop. |
| Paste automatically after picking | ✅ | 🟡 | R1 | uinput (opt-in udev rule) or copy + notification. Untested on a real desktop. |
| Skip passwords | ✅ | ✅ | R1 | Any type with `password`, `secret`, `ConcealedType`, `TransientType`, `AutoGeneratedType` or `onepassword` in its name, for text, images and files (`clip_policy.rs`). |
| Ignore chosen apps | ✅ | 🟡 | R1 | List in `config.json` (`clipboard_ignored_apps`, application id or window class). **GNOME: works** (the extension sends the focused app and the daemon decides). **KDE: not applied**: Wayland does not say which app copied and `wl-paste --watch` does not either. |
| Pause for 15 min / 1 h | ✅ | ✅ | R1 | Palette menu button, palette action (1 h), tray check item. While paused **nothing is read**: the `wl-paste` watcher is stopped on KDE, and on GNOME the extension asks `ShouldCapture` before reading and the daemon says no. State in `~/.local/share/menew/state.json`. |
| Filters (text, links, images, files) | ✅ | ✅ | R1 | Chips All, Text, Links, Images, Files, Pinned, Snippets (the Mac's "Passwords" comes with the notes, Phase 6). Alt+Left/Right or click. |
| Paste queue | ✅ | 🟡 | R1 | Same flow (turn on, copy several things, they are pasted in the order copied), with a different step: the Mac listens to every Command+V, **Wayland tells nobody when another app pastes**, so the step is **Paste next** (`Ctrl+Alt+N` portal shortcut, `menew paste-next`, tray item, palette action). With uinput it also presses Ctrl+V; without it the item is on the clipboard and a notification says to press Ctrl+V. Plain Ctrl+V is not intercepted. |
| Smart actions (phone, address, email, tracking code, color, meeting link) | ✅ | 🟡 | R1 | Tracking codes (Correios, UPS), colors (HEX, RGB, HSL) and meeting links use the Mac's rules exactly. Phone numbers, addresses, e-mails and links are found by Apple's closed `NSDataDetector` on the Mac; Linux uses hand-written rules (a short list of street words for addresses). Address opens OpenStreetMap, not Apple Maps. No QR action yet (Phase 7). `clip_smart.rs`, vectors in `docs/clipboard-spec.md`. |
| Smart search (dates, types, word roots) | ✅ | ⬜ | | |
| Text inside copied images (OCR) | ✅ | ✅ | R1 | `tesseract` in the background after the copy (one image at a time, a second apart, newest first); English, Portuguese, German, Spanish and Chinese packs when installed. Search finds the text; "Copy text from image" in the item menu. Without tesseract the images are kept and not read (logged, and the action says to install `tesseract-ocr`). |
| Password notes from the clipboard (Touch ID) | ✅ | 🔁 | | Secret Service (GNOME Keyring / KWallet) + polkit. |
| Snippets with variables (`{date}`, `{clipboard}`, `{cursor}`, `{input:Name}`) | ✅ | ✅ | R1 | `snippets.json` in the data folder (same keys as the Mac). Shown in the palette and in the Snippets filter; create, edit and delete in a small window; `{input:}` is asked first; the pasted text is not kept in the history. `{cursor}` moves the cursor back with uinput only. `{date}` and friends use hand-written formats per language. |
| Snippet abbreviations expanded while typing | ✅ | ⬜ | | Not in release 1. Needs a global keyboard hook: not possible on Wayland without an input method (IBus). The abbreviation stays in `snippets.json` but is not expanded. |

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

Done: the command palette with the behaviour of the Mac app (apps, recent picks, math, conversions, currency on Enter, time zones, text transforms, dev tools, quick links, emoji, timers and alarms with the countdown in the tray, focus, script commands; spec and test vectors in `docs/palette-spec.md`), text clipboard with search/pins/password filter, tray basics, opt-in autostart, packaging (.deb verified; AppImage being fixed).

Still open in the palette: Menew actions of the later phases, snippets and notes (Phases 5 and 6), the focus options that need quick actions (Phases 5 and 9), the world clocks card (Phase 11). Not on the Mac R1 list and not done: quick reminders, translation, Ask AI and natural-language commands (`remind`, `lembrar`, `faz`, `mach`, `haz`, `ia`, `ki`).

To build for R1 (grouped into relay phases in `docs/plan-mvp.md`):
1. Palette: DONE in Phase 4 except the Menew actions of later phases, snippets and notes inside it.
2. Clipboard: DONE in Phase 5 (images, files, filters, pause, ignore apps on GNOME, paste queue with "Paste next", smart actions, OCR, snippets). Not verified on a real desktop: that a hidden Menew may set the selection for "Paste next" (the headless test keeps the window open).
3. Notes with protected and password notes.
4. Screenshots: capture, annotate, OCR, QR.
5. Windows: snapping via KWin script / GNOME extension.
6. Quick actions: mic/sound/output, dark mode, keep awake, lock, trash, color picker, plain text, eject, brightness, quit apps, ports.
7. System monitor with top processes and tray metrics.
8. Tray popover (Now / Actions), Menew window, settings, welcome tour, language picker, update check.
