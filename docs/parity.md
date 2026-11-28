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
| Notes in the palette | ✅ | ✅ | R1 | Same item as the Mac: title, second line as preview, the text as search keywords, kind "Note"; protected and password notes by title only (never their content, which is not even in memory of the palette: only the title is in `notes.json`). Enter opens the note in the notes window, protected ones ask for the password first. Linux extra: actions "New note" and "New passwords note" (the Mac has them as buttons in the notes tab). `palette.rs`, vectors H in `docs/notes-spec.md`. |
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
| Filters (text, links, images, files) | ✅ | ✅ | R1 | Chips All, Text, Links, Images, Files, Pinned, Snippets, Passwords. Alt+Left/Right or click. |
| Paste queue | ✅ | 🟡 | R1 | Same flow (turn on, copy several things, they are pasted in the order copied), with a different step: the Mac listens to every Command+V, **Wayland tells nobody when another app pastes**, so the step is **Paste next** (`Ctrl+Alt+N` portal shortcut, `menew paste-next`, tray item, palette action). With uinput it also presses Ctrl+V; without it the item is on the clipboard and a notification says to press Ctrl+V. Plain Ctrl+V is not intercepted. |
| Smart actions (phone, address, email, tracking code, color, meeting link) | ✅ | 🟡 | R1 | Tracking codes (Correios, UPS), colors (HEX, RGB, HSL) and meeting links use the Mac's rules exactly. Phone numbers, addresses, e-mails and links are found by Apple's closed `NSDataDetector` on the Mac; Linux uses hand-written rules (a short list of street words for addresses). Address opens OpenStreetMap, not Apple Maps. Links also get "Show QR code for the link" (Phase 7). `clip_smart.rs`, vectors in `docs/clipboard-spec.md`. |
| Smart search (dates, types, word roots) | ✅ | ⬜ | | |
| Text inside copied images (OCR) | ✅ | ✅ | R1 | `tesseract` in the background after the copy (one image at a time, a second apart, newest first); English, Portuguese, German, Spanish and Chinese packs when installed. Search finds the text; "Copy text from image" in the item menu. Without tesseract the images are kept and not read (logged, and the action says to install `tesseract-ocr`). |
| Password notes from the clipboard (Touch ID) | ✅ | 🔁 | R1 | The "Passwords" chip lists the password notes by title (search by title, newest first). Enter copies and pastes the password, Alt+Enter the username, same field choice as the Mac in any language (`secret_value`, the six cases of the Mac test are vectors D01 to D06). The Mac asks for Touch ID; Linux asks for the user password through polkit (see "Protected notes"). `notes.rs`, `crates/menew-ui`. |
| Snippets with variables (`{date}`, `{clipboard}`, `{cursor}`, `{input:Name}`) | ✅ | ✅ | R1 | `snippets.json` in the data folder (same keys as the Mac). Shown in the palette and in the Snippets filter; create, edit and delete in a small window; `{input:}` is asked first; the pasted text is not kept in the history. `{cursor}` moves the cursor back with uinput only. `{date}` and friends use hand-written formats per language. |
| Snippet abbreviations expanded while typing | ✅ | ⬜ | | Not in release 1. Needs a global keyboard hook: not possible on Wayland without an input method (IBus). The abbreviation stays in `snippets.json` but is not expanded. |

## Notes (`Notes.swift`, `NotesViews.swift`, `Markdown.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Many notes, #tags, search, Markdown with checklists | ✅ | ✅ | R1 | Same model and file as the Mac: `notes.json` (keys `id`, `text`, `secure`, `created`, `updated`, `kind`; dates in seconds since 2001 like Swift) in `~/.local/share/menew/notes/`, so a Mac file can be read (a `.menew` backup import comes with Backup, not in R1). Title, preview, `#tags` (not "# Title"), search (case and accent insensitive, also in the title of protected notes), tag chips, newest first. Markdown blocks follow `Markdown.swift` line by line (headings 1 to 3, bullets with indent, numbered, quotes, code fences, rules, checklists you tick in reading mode); inline bold, italic, code, strikethrough and links are written by hand (Foundation does it on the Mac), and only http, https and mailto links open. Window: list + editor + reading mode (`menew notes`). Missing against the Mac: no context menu on the list rows (the actions are in the editor menu), the reading mode choice is not remembered. `markdown.rs`, `notes.rs`, `notes_view.rs`, vectors A, E, F, G, L. |
| Protected notes | ✅ | 🟡 | R1 | The Mac keeps the content in the Keychain and asks for Touch ID. Linux: the content goes to the **Secret Service** (GNOME Keyring or KWallet) with the attributes `application=app.menew.Menew` and `note=<uuid>`; only the first line stays in `notes.json`. Protecting writes to the keyring first: without a keyring nothing is protected and the UI says so (never a plain file). Opening asks for the **user password through polkit** (`org.freedesktop.PolicyKit1.Authority.CheckAuthorization`, action `app.menew.Menew.unlock-note`, `auth_self`, policy `data/app.menew.Menew.policy` installed by the `.deb`). **Where polkit cannot ask (AppImage and dev builds have no policy, or there is no authentication agent) the only protection is the keyring unlocked at login, the unlock goes through, and the notes window shows a notice saying so**: it never pretends to authenticate. Limited because of that, and because a note stays open until the window closes (like the Mac closing the menu). Tested headless (`scripts/desktop-test/phase6.sh`, GNOME and KDE): real `gnome-keyring-daemon`, `secret-tool lookup` finds the content, `grep -r` finds no secret in any plain file or the history. polkit, observed for real: without the policy installed `CheckAuthorization` fails with `Action app.menew.Menew.unlock-note is not registered`; with it installed and no authentication agent it answers `is_authorized=false, is_challenge=true`; in both cases the daemon takes the keyring fallback and logs it. A real password prompt needs a graphical agent and is not testable headless. |
| Password notes (hidden values, copy, cleared after 30 s) | ✅ | ✅ | R1 | Title and name/value rows (same text format as the Mac: first line title, one `name: value` per line, only the first `:` splits), hidden values, an eye to show, a copy button, add and remove rows, switching the type at any time keeps the text. Copy puts the value on the clipboard with the type `x-kde-passwordManagerHint=secret` (the Mac uses `ConcealedType`): the Menew history and other managers skip it. It is cleared after 30 s only if the clipboard still holds that content (the Mac compares `changeCount`; GTK has no counter, so Linux compares the content provider it set). Test hook `MENEW_TEST=1` with `MENEW_SECRET_CLEAR_SECS=3` shortens the delay. |
| Summarize / extract to-dos with AI | ✅ | 🔁 | | Not in release 1 (Ollama, later). The notes menu has no AI items. |

## Screenshots and screen (`Capture.swift`, `TextRecognizer.swift`, `QRCode.swift`, `ScrollCapture.swift`, `ScreenAsk.swift`, `LiveText.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| Capture area / window / screen with a shortcut | ✅ | 🟡 | R1 | `org.freedesktop.portal.Screenshot` (`portal.rs`), `interactive` so the system shows its own picker; `Ctrl+Alt+Shift+S` (GlobalShortcuts portal), `menew capture`, `menew capture-full`, the palette and the tray. The file the portal writes is deleted right after it is read (the Mac deletes its temporary file). A portal failure shows `cap_portal_failed`, there is no silent fallback. **Limited**: the portal itself could not run in the headless sessions (`A portal frontend implementing org.freedesktop.portal.Screenshot was not found`), so the picture came from the test hook `__capture-from` there; the real picker and the shortcut are not tested on a real desktop. The Mac captures the screen with the mouse for "whole screen", the portal chooses the screen. |
| Annotate (pen, arrows, rectangles, text, blur), copy, save, drag | ✅ | ✅ | R1 | `menew-ui/src/screen.rs` `Editor`, rules in `capture.rs`: same line width, font size, arrow head, text outline, 8 colors and thresholds as `Capture.swift`; blur really pixelates (the pixelated image shown inside the box; checked pixel by pixel in the desktop test). Copy puts the image on the clipboard (and so in the history), Save opens a file dialog in the Desktop folder with `Screenshot <date>.png`, drag gives a file `Screenshot.png` and the image. Not verified: the drag itself (needs a pointer) and the file dialog (portal) in the headless sessions. No "Pin" button (not in release 1). The Mac also keeps a copy in Memory (not in release 1). Vectors in `docs/screenshots-spec.md`. |
| Pin a screenshot floating on screen | ✅ | 🟡 | | Not in release 1. A normal always-on-top window is not guaranteed on Wayland. The editor has no "Pin" button. |
| Copy text from the screen (OCR) | ✅ | ✅ | R1 | `Ctrl+Alt+Shift+T`, `menew capture-text`, the palette, the tray, and the "Copy text" button of the editor (reads the capture without the marks). Portal + `tesseract` (`ocr.rs`, reused from Phase 5); messages "Reading text…", "Text copied", "Text copied (N lines)", "No text found" as on the Mac; the picture given to tesseract is a file in the private runtime folder, deleted when read. Tested with a generated image with a known text. |
| QR codes (read from screen, show for text/link/Wi-Fi) | ✅ | 🟡 | R1 | Read: `menew qr-scan`, the palette, the tray; the content of the first code is copied and **a link is never opened** (checked: no log line, no OpenURI call, no browser process). Show: the QR window (text or link, Wi-Fi with name and password in the Mac payload format `WIFI:T:WPA;S:..;P:..;;`), "Copy image", and the smart action "Show QR code for the link" for copied links. `rqrr` reads, `qrcodegen` makes (pure Rust). **Limited**: only QR codes are read (Vision also reads Aztec, Data Matrix, PDF417); the copied image has a 4 module border. |
| Screen recording (video/GIF, with audio) | ✅ | 🔁 | | Not in release 1. ScreenCast portal + PipeWire + GStreamer. |
| Scrolling capture | ✅ | ❌ | | Not in release 1. The Mac scrolls the area itself (up to 40 frames or 16,000 px, needs Accessibility) and opens the joined image in the editor; Linux needs synthetic scrolling of another app, not possible on Wayland without uinput. |
| Ask about the screen (AI) | ✅ | 🔁 | | Not in release 1. The Mac reads the text of an area and answers Explain / Summarize / Translate / "What do I do?" or a typed question with Apple Intelligence; Linux: Ollama with a vision model, later. |
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
| Quit apps / force quit / never-quit list | ✅ | 🟡 | R1 | Running apps are matched to their desktop entries through `/proc`. `quit firefox` (SIGTERM, apps that handle it can ask to save), `kill firefox` (SIGKILL), "Quit all apps" skips the never-quit list (`never_quit` in `config.json`). Wayland has no way to ask an app to close its windows, so it is a signal, not the Mac's polite quit. |
| Ports in use (stop a server) | ✅ | ✅ | R1 | `port 3000`: listening sockets of `/proc/net/tcp{,6}` matched to this user's processes; Enter sends SIGTERM. |
| Presentation mode | ✅ | 🔁 | | Inhibit + do not disturb + pause clipboard. |
| Cleaning mode (lock keyboard, black screens) | ✅ | 🟡 | | Black fullscreen windows yes; locking the keyboard needs an exclusive grab, not available on Wayland. |
| Automations ("when X, do Y") | ✅ | ⬜ | | |

## System health (`SystemMonitor.swift`, `MonitorViews.swift`, `SystemHistory.swift`, `Alerts.swift`, `AlertRules.swift`, `BatteryCare.swift`, `BatteryWear.swift`, `Sensors.swift`, `NetUsage.swift`, `WiFi.swift`, `Diagnosis.swift`, `RunawayWatch.swift`, `ScreenTime.swift`, `StartupItems.swift`, `Cleaner.swift`, `Uninstaller.swift`, `Accessories.swift`)

| Feature | Mac | Linux | R1 | Notes |
|---|---|---|---|---|
| CPU, GPU, memory, swap, disk, battery, network live | ✅ | 🟡 | R1 | DONE in Phase 10 (`monitor.rs`, `monitor_view.rs`, vectors in `docs/monitor-spec.md`). `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `statvfs`, `/sys/class/power_supply`. Limited: GPU only on AMD (`gpu_busy_percent`; Intel and NVIDIA report no load in sysfs, so the card is hidden, never an estimate); memory pressure is derived from the available share (no Linux level); battery times are computed from energy and power; the disk read/write speed, per-core efficiency/performance split, uptime and local IP of the Mac cards are not done. |
| Top processes (quit in one click) | ✅ | ✅ | R1 | DONE in Phase 10. By CPU (over the last interval, from `/proc/<pid>/stat`) and by memory, 5 each; SIGTERM in one click, Force Quit (SIGKILL) as a second, explicit button; only the user's own processes are listed and quit (system and other users' refused, also by `menew __quit-process` in the tests). The Mac's "open Activity Monitor" link has no counterpart. |
| Metrics in the tray (choose which) | ✅ | 🟡 | R1 | DONE in Phase 10 with limits. `Title` + `ToolTip` of the StatusNotifierItem with cpu, cpuGraph, ram, net, disk, battery (`temp` needs sensors, R2). KDE Plasma shows the Title as text in the panel; **GNOME's AppIndicator shows only the icon**, so on GNOME the metrics are only in the tooltip. The choice is `monitor_tray_items` in `config.json`, empty by default (opt-in) until the settings window of Phase 11; the Mac default is CPU + RAM. No separate icons per metric. |
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
| Welcome tour | ✅ | ✅ | R1 | `crates/menew-ui/src/welcome.rs`: three pages like `Onboarding.swift` (hello, setup, all set). The Mac's permission page is a setup page here: what Menew did by itself on the first start (`crates/menew/src/setup.rs`: start at login on for new users like `LaunchAtLogin.applyDefault`, the GNOME extension turned on and, without the GlobalShortcuts portal, GNOME custom shortcuts created) with a button where something is missing. Shown once for new users, again from the palette or `menew welcome`. The Mac's module switches and menu bar preview have no Linux counterpart yet (Phase 11 settings). Headless test: `scripts/desktop-test/welcome.sh`. |
| 5 languages, follow system or pick | ✅ | 🟡 | R1 | Strings in 5 languages; no picker yet. |
| Backup / restore (`.menew` file) | ✅ | ⬜ | | Same format as the Mac where possible. |
| Updates | ✅ Sparkle | ✅ | R1 | Same behavior as `Updater.swift`: automatic check on by default with a toggle (`auto_update_check` in `config.json` and a tray checkmark; the settings window comes in Phase 11), every 24 h (+ one 5 minutes after start), a discreet reminder (dot on the tray icon and an "Update available: X" menu item, never a window by itself), "Check for Updates…" in the tray and the palette, a window with the version's notes and Install and Relaunch / Skip This Version / Later, nothing in `--snapshot`. Feed: the GitHub releases listing (pre-releases included for a pre-release build) + the `update.json` of the release, with an Ed25519 signature (Linux-only key) checked together with size and SHA-256 before anything is installed. AppImage: replaced in place and restarted; `.deb`: `pkexec apt-get install -y`; other: tells with the link. Differences: no install on quit, restart without arguments, `menew check-updates [--install]` for terminals. `updates.rs`, `updater.rs`, `docs/updates-spec.md`, `scripts/make-update-json.sh`, `scripts/test-updates-e2e.sh`. The public key in `UPDATE_PUBLIC_KEY` is a placeholder until the real key is made (see CHANGELOG). |
| Live notch | ✅ | ❌ | | No notch; the same alerts go to notifications and the tray. |
| Widgets | ✅ | ❌ | | No cross-desktop widget API (KDE Plasmoids could come later). |
| Siri, Shortcuts, Spotlight | ✅ | 🔁 | | D-Bus actions for scripting; GNOME Shell search provider and KRunner for search. |
| Continuity Camera | ✅ | ❌ | | Apple-only. KDE Connect could be explored later. |
| Focus filters | ✅ | 🔁 | | Follow the desktop's Do Not Disturb. |
| Next meeting with Join | ✅ | 🔁 | | Evolution Data Server (GNOME) / Akonadi (KDE). |
| Liquid Glass design | ✅ | ❌ | | libadwaita styling instead. |

## Release 1 summary

Done: the command palette with the behaviour of the Mac app (apps, recent picks, math, conversions, currency on Enter, time zones, text transforms, dev tools, quick links, emoji, timers and alarms with the countdown in the tray, focus, script commands; spec and test vectors in `docs/palette-spec.md`), text clipboard with search/pins/password filter, tray basics, autostart (on for new users, like the Mac), packaging (.deb verified; AppImage being fixed).

Still open in the palette: Menew actions of the later phases, snippets and notes (Phases 5 and 6), the focus options that need quick actions (Phases 5 and 9), the world clocks card (Phase 11). Not on the Mac R1 list and not done: quick reminders, translation, Ask AI and natural-language commands (`remind`, `lembrar`, `faz`, `mach`, `haz`, `ia`, `ki`).

To build for R1 (grouped into relay phases in `docs/plan-mvp.md`):
1. Palette: DONE in Phase 4 except the Menew actions of later phases, snippets and notes inside it.
2. Clipboard: DONE in Phase 5 (images, files, filters, pause, ignore apps on GNOME, paste queue with "Paste next", smart actions, OCR, snippets). Not verified on a real desktop: that a hidden Menew may set the selection for "Paste next" (the headless test keeps the window open).
3. Notes with protected and password notes.
4. Screenshots: DONE in Phase 7 (capture through the portal, annotation editor with blur, copy text from the screen, QR codes). Not verified on a real desktop: the screenshot portal and its picker, the global shortcuts, dragging out of the editor.
5. Windows: DONE (the Mac's 17 positions through a KWin script and the GNOME extension's `org.menew.Shell`, read back by `scripts/desktop-test/actions.sh`). Not verified on a real desktop: the snapping shortcuts.
6. Quick actions: DONE (mic/sound/output, dark mode, keep awake, lock, trash, color picker, plain text, eject, brightness, quit apps, ports). Not verified on a real desktop: audio devices, brightness, eject.
7. System monitor with top processes and tray metrics: DONE in Phase 10 (see the rows above and `docs/monitor-spec.md`).
8. Tray popover (Now / Actions), Menew window, settings, welcome tour, language picker, update check.
