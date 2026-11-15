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
| Menew actions in the palette | ✅ | ✅ | R1 | Generic `PaletteItem` list in `palette.rs`; the other features add their items through `ItemAction::Command` ids (`qa.*`, `win.*`, `cap.*`, `note.*`, `open.*`) built by the daemon (`crates/menew/src/commands.rs`). The same ids are the global shortcuts and `menew run <id>`. Smart items for the copied text first when the search is empty. |
| Math (`12*7.5`, `20% of 340`) | ✅ | ✅ | R1 | Own parser like the Mac: `+ - * / ^ x × ÷ % ! √`, `pi`, `e`, `sqrt sin cos tan log ln abs round`, `2(3+1)`, `200+10%`. Decimal comma or point; result uses the language's decimal separator. `calculator.rs`. |
| Unit conversions (`5 km to mi`, `72 f to c`) | ✅ | ✅ | R1 | Same unit list and aliases as the Mac (length, mass, volume, data, time, speed, area, temperature) and the same connectors in 5 languages. `conversions.rs`. |
| Currency conversion (`100 usd to brl`, fetched on ↩) | ✅ | ✅ | R1 | Same 24 currencies and provider (ECB through `api.frankfurter.app`), fetched only on Enter, cached 6 h. The request goes through the system `curl` (no HTTP/TLS crate in the tree). Currency text (`$100.00`, `R$ 100,00`, `100,00 €`) follows hand-written locale rules instead of Foundation's. `currency.rs`. |
| Time zones (`3pm ny to lisbon`) and world clocks | ✅ | ✅ | R1 | Same aliases (pt, en, de, es, zh) and cities of the tz database, DST through `date` with `TZ` (needs tzdata and coreutils). World clocks: Settings, `clocks` in the palette and the Now tab of the tray popover. `time_zones.rs`. |
| Text transforms (case, accents, sort/dedupe, JSON, Base64, URL, counts) with live preview | ✅ | ✅ | R1 | All 20 transforms, with the same keywords in 5 languages and the preview (first 200 characters, `⏎` between lines, none above 100,000 bytes). Applies to the clipboard text read by GTK while the palette is open, else the newest history entry. Word counts use letter and digit runs, not Apple's word breaker. `text_transforms.rs`. |
| Developer tools (JWT decode, UUID, Unix timestamp) | ✅ | ✅ | R1 | JWT (with `exp`/`iat`/`nbf` dates), UUID v4, Unix timestamp now, and a pasted 10 or 13 digit timestamp shown as a date (Enter copies the ISO date). The long date text is written by hand per language and can differ from macOS. `dev_tools.rs`. |
| Quick links (`g`, `gh`, `maps`, `yt`, `wiki`, custom `{q}`), open typed addresses, web search | ✅ | ✅ | R1 | Same six defaults (`maps` opens OpenStreetMap: the Mac uses its native Apple Maps, Linux has no native map app, and copied addresses also open in OpenStreetMap; edit `quick_links` in `config.json` to use another map). Custom links live in `~/.config/menew/config.json` until the settings window (Phase 11). Opens with the default browser. `quick_links.rs`. |
| Emoji and symbols (`emoji rocket`, `:fire`, arrow/degree/euro) | ✅ | ✅ | R1 | Same filter and word table as the Mac; names come from the Unicode data of the build host (`emoji_data.rs`, generated). The Apple logo symbol is left out. Pasting uses the same path as the clipboard (uinput or copy + notification). `emoji.rs`. |
| Timers and alarms (`timer 10m tea`, `alarm 7:30`), countdown in the tray | ✅ | ✅ | R1 | Countdown in the StatusNotifierItem `Title` and `ToolTip` (KDE shows the tooltip; GNOME depends on the AppIndicator extension). At the end: a notification and the sound of the freedesktop theme (`canberra-gtk-play`, else `paplay`, else a beep). In memory only. The Mac's HUD toasts ("Copied: x", "Timer until 9:10") are desktop notifications here. `timers.rs`, `crates/menew/src/alerts.rs`. |
| Focus timer (`focus 50`) | ✅ | ✅ | R1 | Same flow: 25 min default, `focus N` (1 to 600), +5 min, stop, 5 min break, notification and sound, countdown in the tray. While focusing it can mute the microphone, keep the computer awake (on by default) and pause the clipboard, like the Mac, and undoes only what it changed (`focus_hooks.rs`). |
| Script commands (shell/Python/Node/Ruby, Raycast headers) | ✅ | ✅ | R1 | Folder `~/.local/share/menew/scripts`, "Open scripts folder" creates it with an example, Raycast (and `@menew.`) headers: title, icon, mode (silent, compact, fullOutput, inline) and `argument1` as the text after the title. Runs with a 2 minute limit, `~/.local/bin` and `/usr/local/bin` added to `PATH`. AppleScript files are ignored (❌: no Linux equivalent). `script_commands.rs`. |
| Snippets in the palette | ✅ | ✅ | R1 | Same items as the Mac (title, preview, keyword `snippet` plus the text). `palette.rs`. |
| Notes in the palette | ✅ | ✅ | R1 | New note (`Ctrl+Alt+J`), new password note, open Notes, and the notes matching the text (protected ones by title only). `notes.rs`. |
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
| Many notes, #tags, search, Markdown with checklists | ✅ | ✅ | R1 | `notes.json` in the data folder, #tags, accent-insensitive search, Markdown styled while typing (headings, bold, italic, code, strike, quotes, `- [ ]` tasks clickable). Notes section of the Menew window. `notes.rs`, `crates/menew-ui/src/notes.rs`. |
| Protected notes | ✅ | ✅ | R1 | Only the title stays in `notes.json`; the text goes to the Secret Service (GNOME Keyring or KWallet) through `secret-tool`. Opening asks for the session password through polkit (`app.menew.Menew.unlock-notes`, `auth_self_keep`, installed by the .deb); without the policy (AppImage, running from source) the keyring lock is the guard. |
| Password notes (hidden values, copy, cleared after 30 s) | ✅ | ✅ | R1 | Title and `key: value` fields with hidden values (peek icon). Copy marks the value with `x-kde-passwordManagerHint` (Menew and other clipboard managers skip it) and clears it after 30 s if nothing else was copied. |
| Summarize / extract to-dos with AI | ✅ | 🔁 | | Ollama. |

## Screenshots and screen (`Capture.swift`, `TextRecognizer.swift`, `QRCode.swift`, `ScrollCapture.swift`, `ScreenAsk.swift`, `LiveText.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Capture area / window / screen with a shortcut | ✅ | ✅ | R1 | `org.freedesktop.portal.Screenshot`: the desktop shows its own picker. `Ctrl+Alt+S` (interactive) and "Capture the whole screen". `menew-desktop`, `crates/menew/src/capture.rs`. |
| Annotate (pen, arrows, rectangles, text, blur), copy, save, drag | ✅ | ✅ | R1 | Arrow, rectangle, pen, text, pixelate blur, five colors, undo (`Ctrl+Z`); Copy (`Ctrl+C`), Save (file dialog, PNG) and drag the image out from the bar under it. `crates/menew-ui/src/annotate.rs`. |
| Pin a screenshot floating on screen | ✅ | 🟡 | | A normal always-on-top window is not guaranteed on Wayland. |
| Copy text from the screen (OCR) | ✅ | ✅ | R1 | Portal capture, then `tesseract` (same languages as the clipboard OCR); the capture is deleted right after. `Ctrl+Alt+O`. Also "Copy text" in the editor. |
| QR codes (read from screen, show for text/link/Wi-Fi) | ✅ | ✅ | R1 | Read from the screen with `zbarimg` (zbar-tools): a link opens and is copied. Show a QR code of the copied text, of `qr <text>`, or of the current Wi-Fi (`nmcli`, `WIFI:` format) with `qrencode`. `capture.rs`. |
| Screen recording (video/GIF, with audio) | ✅ | 🔁 | | ScreenCast portal + PipeWire + GStreamer. |
| Scrolling capture | ✅ | ❌ | | Needs synthetic scrolling of another app; not possible on Wayland without uinput. |
| Ask about the screen (AI) | ✅ | 🔁 | | Ollama with a vision model. |
| Live Text on images, background removal | ✅ | 🔁 | | Tesseract; background removal via a local model, later. |

## Windows and files (`Windows.swift`, `DragSnap.swift`, `Shelf.swift`, `ImageTools.swift`, `PDFTools.swift`, `Organizer.swift`, `Profiles.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Snap the front window (halves, corners, thirds, full, other display) | ✅ | ✅ | R1 | The Mac's 17 positions from the palette and `Ctrl+Alt+Shift+Arrows` / `Ctrl+Alt+Enter`. KWin: a script loaded over `org.kde.kwin.Scripting` (Plasma 5.27 and 6). GNOME: the Menew extension exports `org.menew.Shell` (`SnapFocused`, `MaximizeFocused`, `CenterFocused`, `MoveFocusedToNextMonitor`). Headless tests read the geometry back on both. X11 without either: not done. |
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
| Mute microphone (global shortcut) / mute sound | ✅ | ✅ | R1 | `pactl` (PipeWire or PulseAudio), `wpctl` as fallback. `Ctrl+Alt+M`. `quick_actions.rs`. |
| Switch audio output and input | ✅ | ✅ | R1 | `pactl -f json list sinks/sources`, `set-default-sink/source`; one palette item per other device. |
| Dark mode toggle | ✅ | ✅ | R1 | GNOME: `gsettings` `color-scheme` (and the GTK 3 theme name). KDE: `plasma-apply-colorscheme BreezeDark/BreezeLight`, state read from `kdeglobals`. |
| Keep awake (for a time / until / while an app runs) | ✅ | ✅ | R1 | `systemd-inhibit --what=idle:sleep` held while on; `awake 2h` / `acordado 90m` for a time; turns itself off. "While an app runs" is not ported. |
| Lock screen | ✅ | ✅ | R1 | `loginctl lock-session`, then `org.freedesktop.ScreenSaver.Lock`, then `xdg-screensaver lock`. |
| Empty trash | ✅ | ✅ | R1 | Confirmation first (like the Mac), then `gio trash --empty`; says so when it is already empty. |
| Color picker | ✅ | ✅ | R1 | `org.freedesktop.portal.Screenshot.PickColor`; the `#RRGGBB` is copied. |
| Plain-text clipboard | ✅ | ✅ | R1 | Reads the clipboard text and puts it back as plain text only. |
| Eject disks | ✅ | ✅ | R1 | Mounted partitions of removable or hot-plugged disks from `lsblk -J`; `udisksctl unmount` (or `gio mount -u`), then `udisksctl power-off`. |
| Hidden files / desktop icons / hide Dock | ✅ | 🟡 | | Depends on the desktop; GNOME has no desktop icons by default. |
| Display brightness (built-in + DDC/CI) | ✅ | ✅ | R1 | `brightness 40`: built-in panel through logind `SetBrightness` (no root), external displays through `ddcutil setvcp 10` when installed. |
| Quit apps / force quit / never-quit list | ✅ | 🟡 | R1 | Running apps are matched to their desktop entries through `/proc`. `quit firefox` (SIGTERM, apps that handle it can ask to save), `kill firefox` (SIGKILL), "Quit all apps" with the never-quit list in Settings. Wayland has no way to ask an app to close its windows, so it is a signal, not the Mac's polite quit. |
| Ports in use (stop a server) | ✅ | ✅ | R1 | `port 3000`: listening sockets of `/proc/net/tcp{,6}` matched to this user's processes; Enter sends SIGTERM. |
| Presentation mode | ✅ | 🔁 | | Inhibit + do not disturb + pause clipboard. |
| Cleaning mode (lock keyboard, black screens) | ✅ | 🟡 | | Black fullscreen windows yes; locking the keyboard needs an exclusive grab, not available on Wayland. |
| Automations ("when X, do Y") | ✅ | ⬜ | | |

## System health (`SystemMonitor.swift`, `MonitorViews.swift`, `SystemHistory.swift`, `Alerts.swift`, `AlertRules.swift`, `BatteryCare.swift`, `BatteryWear.swift`, `Sensors.swift`, `NetUsage.swift`, `WiFi.swift`, `Diagnosis.swift`, `RunawayWatch.swift`, `ScreenTime.swift`, `StartupItems.swift`, `Cleaner.swift`, `Uninstaller.swift`, `Accessories.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| CPU, GPU, memory, swap, disk, battery, network live | ✅ | ✅ | R1 | `/proc/stat`, `/proc/meminfo`, `/proc/diskstats`, `/proc/net/dev`, `statvfs`, `/sys/class/power_supply` (health, cycles, watts), GPU from `gpu_busy_percent` (amdgpu, Intel Xe; NVIDIA not shown). Performance section with live graphs. `sysmon.rs`. |
| Top processes (quit in one click) | ✅ | ✅ | R1 | The five using the most CPU and memory (this user's), with Quit. |
| Metrics in the tray (choose which) | ✅ | 🟡 | R1 | Choose CPU, memory, GPU, disk, network, battery in the Performance section. They go to the SNI `Title` and tooltip: KDE can show the text next to the icon, GNOME (AppIndicator) only in the tooltip. |
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
| Menu bar popover ("Now", "Actions" tabs) | ✅ | ✅ | R1 | Left click on the tray icon (or `menew popover`): a small window with Now (search box for the palette, health, running timers and keep awake, world clocks, favorites) and Actions (favorites with Edit, then every action in groups). `crates/menew-ui/src/popover.rs`. |
| Menew window (sidebar: Overview, Clipboard, Notes, Performance, Settings...) | ✅ | ✅ | R1 | libadwaita split view: Clipboard, Notes, Performance, Settings. The Mac's Overview, Memory, Recordings and AI sections come with those features. |
| Settings (incl. turning features off completely) | ✅ | ✅ | R1 | Language, start at login, update check, clipboard / quick actions / notes off completely (with the clipboard off it is not read), clipboard options, world clocks, focus options, never-quit list, the shortcuts with their `menew run` commands, the GNOME extension state. Saved to `config.json` at once. |
| Welcome tour | ✅ | ✅ | R1 | First launch: what Menew does, the shortcut, what the clipboard needs on this desktop, the features to keep, start at login and the update check (off by default). |
| 5 languages, follow system or pick | ✅ | ✅ | R1 | Language picker in Settings (or the system's); restarting applies it everywhere. |
| Backup / restore (`.menew` file) | ✅ | ⬜ | | Same format as the Mac where possible. |
| Updates | ✅ Sparkle | ✅ | R1 | Opt-in daily check of `api.github.com/repos/Quost/menew-linux/releases/latest` with `curl`; "Check now" in Settings; installing stays with the package manager or the AppImage. |
| Live notch | ✅ | ❌ | | No notch; the same alerts go to notifications and the tray. |
| Widgets | ✅ | ❌ | | No cross-desktop widget API (KDE Plasmoids could come later). |
| Siri, Shortcuts, Spotlight | ✅ | 🔁 | | D-Bus actions for scripting; GNOME Shell search provider and KRunner for search. |
| Continuity Camera | ✅ | ❌ | | Apple-only. KDE Connect could be explored later. |
| Focus filters | ✅ | 🔁 | | Follow the desktop's Do Not Disturb. |
| Next meeting with Join | ✅ | 🔁 | | Evolution Data Server (GNOME) / Akonadi (KDE). |
| Liquid Glass design | ✅ | ❌ | | libadwaita styling instead. |

## Added on the Mac after this matrix was written (checked 2026-11-15)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Favorites on top of the Actions tab (`Favorites.swift`) | ✅ | ✅ | R1 | The Mac's default eight with Linux ids, editable in the popover (`favorites` in `config.json`). |
| World clocks card and settings (`WorldClock.swift`) | ✅ | ✅ | R1 | Now tab of the popover and Settings. |
| Copy system report (`SystemReport.swift`) | ✅ | ⬜ | | Text for support: versions, desktop, session type, memory, disks. |
| Share passwords as `.menewsecret` files (`SecretShare.swift`) | ✅ | ⬜ | | X25519 + a Secret Service key. Needs a crypto crate: decide before porting. |
| AI agents hub: sessions, plan limits, usage (`Agent*.swift`) | ✅ | ⬜ | | Reads `~/.claude`, `~/.codex`, `~/.gemini` logs, which exist on Linux too. Release 2 candidate. |
| MCP server (`MCPServer.swift`, `MCPHTTP.swift`) | ✅ | ⬜ | | `menew --mcp` over stdio; release 2 with the agents hub. |
| One metric per tray icon (`MenuBarPro.swift`) | ✅ | 🟡 | | Several SNI items are possible; GNOME AppIndicator shows them, KDE too. Later. |
| Next meeting with Join (`Meetings.swift`) | ✅ | 🔁 | | Same as the row above: EDS / Akonadi. |
| Demo mode, snapshots and video frames (`DemoMode.swift`, `Snapshot.swift`, `VideoFrames.swift`) | ✅ | 🟡 | | The headless desktop tests already make screenshots with fake data. |
| Send feedback as a prefilled GitHub issue (`Feedback.swift`) | ✅ | ⬜ | | Opens the browser only. |
| Shortcuts app actions (`ShortcutActions.swift`) | ✅ | 🔁 | | D-Bus actions / CLI subcommands. |
| Migration from MeuMenu (`Migration.swift`) | ✅ | ❌ | | Never existed on Linux. |

## Release 1 summary

Every R1 row is ✅ except the ones that stay 🟡 because of the platform, not missing work:
- **Global shortcuts** (palette and clipboard): the GlobalShortcuts portal exists on KDE and GNOME 48+; GNOME 45-47 needs custom shortcuts running `menew toggle` / `menew run <id>` (shown in Settings). Not tried on a real desktop yet.
- **Paste after picking** and **paste queue**: Wayland gives no way to press Ctrl+V for another app except uinput (opt-in udev rule), and does not say when another app pastes, so the queue step is "Paste next".
- **Ignore chosen apps** on KDE, **smart actions** (hand-written detectors instead of Apple's), **quit apps** (signals, not a polite quit) and **metrics in the tray** on GNOME (tooltip only).

Tested on headless GNOME Shell 46 and KWin 5.27 (Ubuntu 24.04), in CI and in the Docker image of `scripts/dev/` (`scripts/desktop-test/`, including `r1-checks.sh` and `r1-kde-checks.sh`). **Before the first tag**: a pass on a real GNOME and a real KDE Plasma desktop (shortcuts, tray click, paste, portals, polkit dialog, keyring), then the AppImage check and the `/linux` page of the site.

Not in release 1 (rows without R1): quick reminders, translation, Ask AI and natural-language commands, smart search, password notes from the clipboard, snippet abbreviations, screen recording, pinned screenshots, image and PDF tools, transcription, profiles, presentation and cleaning modes, automations, the history and alerts of the system monitor, temperatures, network details, screen time, cleaning and uninstalling, recordings, memory, AI agents and the MCP server, backups, and the newer Mac features listed above.
