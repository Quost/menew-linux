# Clipboard specification and test vectors

The Mac app is the specification (`../menew/Sources/Menew`). This file lists, for the clipboard, what
the Mac code does with a given input and where in the Mac code that comes from. The tables in
"Vectors" are **executed**: `crates/menew-core/tests/clipboard_spec.rs` reads this file, runs every row
through the real Linux code and compares the result. A row that does not match fails the test, and
the test also fails if it executes a different number of rows than the file has. The expected
values were derived by hand from the Mac code, not copied from the Linux output.

In a Markdown table cell `\n` stands for a line break. `-` is "nothing" / "none". Cells never contain a
pipe character.

## Where each part comes from in the Mac code

| Part | Mac file / function | Linux |
|---|---|---|
| What is read from a copy, in which order (files, then text, then image), types that mean "do not keep" | `ClipboardManager.swift` `poll`, `ignoredTypes` | `clip_policy.rs` `plan`, `is_secret_type` |
| Entry kinds, filters (all, text, link, image, file, pinned, snippets), titles, links, colors, code | `ClipboardManager.swift` `ClipItem`, `ClipFilter` | `clip_item.rs` |
| Image file name (first 10 bytes of the SHA-256, in hex) | `ClipboardManager.swift` `poll` | `clip_policy.rs` `image_file_name`, `sha256.rs` |
| Dedupe (same text, same paths, same image file), order, pins, clear keeps pinned, limits (300, at least 20, days) | `ClipboardManager.swift` `insert`, `sameContent`, `prune`, `clear` | `clipboard_db.rs` |
| Search in the text, the app and the text read from images; pinned first | `ClipboardManager.swift` `filtered` | `clipboard_db.rs` `filtered` |
| Pause (15 min, 1 h, until resumed) | `ClipboardManager.swift` `isPaused`, `pause`, `resume` | `clip_policy.rs` `PauseState` |
| Ignored apps | `ClipboardManager.swift` `ignoredApps`, `poll` | `clip_policy.rs` `is_ignored_app`, `should_capture` |
| Smart actions (meeting link, colors, tracking code, phone, WhatsApp, address, e-mail, link) | `SmartClip.swift`, `Meetings.swift` `meetingLink` | `clip_smart.rs` |
| Smart actions in the palette (3 first when the search is empty, the others by keyword) | `Palette.swift` `update` | `palette.rs` `smart_items`, `results` |
| Paste queue | `PasteQueue.swift` | `paste_queue.rs` |
| Snippets, variables, fields, cursor | `Snippets.swift` `expandWithCursor`, `fields`, `apply` | `snippets.rs` |
| Snippets in the palette | `Palette.swift` `commands` | `palette.rs` `commands` |
| Text of images (OCR) | `ClipboardManager.swift` `queueOCR`, `TextRecognizer.swift` | `ocr.rs`, `menew-clipboard` |
| Strings | `L10n.swift` | `strings.rs` |

## Known differences (also in `docs/parity.md`)

- **Phone numbers, addresses, e-mails and links** are found on the Mac by Apple's `NSDataDetector`, which is
  closed. The Linux rules are written by hand (`clip_smart.rs`): the rows marked `approx` below show cases both
  agree on, they do not prove the rules are the same. The meeting link, the colors and the tracking
  codes come from regular expressions and plain code in the Mac source: those rows are `exact`.
- The **address** action opens OpenStreetMap, the Mac opens Apple Maps (`maps.apple.com`).
- The **phone** action opens a `tel:` link with the default handler; the subtitle is "With your phone app"
  instead of "With FaceTime or your iPhone". There is no QR code action for links yet (Phase 7).
- Images are kept as they arrive (`image/png`, `image/jpeg`...) named by the hash of those bytes. The Mac converts
  everything to PNG first.
- A copy of the same image keeps its text read by OCR; on the Mac the entry is replaced and read again.
- The paste queue steps with "Paste next", not with every Ctrl+V (Wayland does not tell when another app pastes).
- The text expander (snippet abbreviations while typing) is not part of this port.
- The `{time}` of English has a plain space before PM; macOS 14 prints a narrow no-break space.
- Counts of characters use Unicode scalar values, the Mac uses grapheme clusters.

## Vectors

### Smart actions (`SmartClip.actions`)

Each cell of "Actions" is `id~title~subtitle~action`, entries joined by ` ;; `. "Rules" is `exact` (regular
expression or code of the Mac) or `approx` (the Mac asks `NSDataDetector`). `REPEAT:x:N` is `x` repeated N times.

| ID | Lang | Region | Text | Actions | Rules | Mac source |
|---|---|---|---|---|---|---|
| A01 | en | - | #3B82F6 | color.RGB~Copy as rgb(59, 130, 246)~Copied color: #3B82F6~copy:rgb(59, 130, 246) ;; color.HSL~Copy as hsl(217, 91%, 60%)~Copied color: #3B82F6~copy:hsl(217, 91%, 60%) | exact | SmartClip.detect (HEX equal to the text is skipped), ParsedColor.hsl |
| A02 | en | - | #38f | color.HEX~Copy as #3388FF~Copied color: #38f~copy:#3388FF ;; color.RGB~Copy as rgb(51, 136, 255)~Copied color: #38f~copy:rgb(51, 136, 255) ;; color.HSL~Copy as hsl(215, 100%, 60%)~Copied color: #38f~copy:hsl(215, 100%, 60%) | exact | SmartClip.parseColor (3 digits are doubled) |
| A03 | en | - | rgb(59, 130, 246) | color.HEX~Copy as #3B82F6~Copied color: rgb(59, 130, 246)~copy:#3B82F6 ;; color.HSL~Copy as hsl(217, 91%, 60%)~Copied color: rgb(59, 130, 246)~copy:hsl(217, 91%, 60%) | exact | SmartClip.parseColor (rgb), detect (RGB equal to the text is skipped) |
| A04 | pt | - | #3B82F6 | color.RGB~Copiar como rgb(59, 130, 246)~Cor copiada: #3B82F6~copy:rgb(59, 130, 246) ;; color.HSL~Copiar como hsl(217, 91%, 60%)~Cor copiada: #3B82F6~copy:hsl(217, 91%, 60%) | exact | L10n "Copiar como %@", "Cor copiada: %@" |
| A05 | en | - | rgb(1, 2, 300) | - | exact | SmartClip.parseColor (each value 0...255) |
| A06 | en | - | AB123456789BR | tracking~Track package AB123456789BR~rastreamento.correios.com.br~open:https://rastreamento.correios.com.br/app/index.php?objetos=AB123456789BR | exact | SmartClip.trackingCode, detect (suffix BR is Correios) |
| A07 | en | - | 1z999aa10123456784 | tracking~Track package 1Z999AA10123456784~parcelsapp.com~open:https://parcelsapp.com/en/tracking/1Z999AA10123456784 | exact | SmartClip.trackingCode (1Z + 16, upper-cased) |
| A08 | pt | - | RR 123456789 BR | tracking~Rastrear encomenda RR123456789BR~rastreamento.correios.com.br~open:https://rastreamento.correios.com.br/app/index.php?objetos=RR123456789BR | exact | SmartClip.trackingCode (spaces are removed) |
| A09 | en | - | AB12345678BR | - | exact | SmartClip.trackingCode (9 digits) |
| A10 | en | - | https://us02web.zoom.us/j/123456789?pwd=abc | meeting~Join meeting~us02web.zoom.us~open:https://us02web.zoom.us/j/123456789?pwd=abc | exact | Meetings.meetingLink (zoom.us/j/), detect (a meeting link hides the link action) |
| A11 | en | - | Join: https://meet.google.com/abc-defg-hij thanks | meeting~Join meeting~meet.google.com~open:https://meet.google.com/abc-defg-hij | exact | Meetings.linkPattern (meet.google.com/), found inside a longer text |
| A12 | en | - | https://acme.webex.com/meet/ana | meeting~Join meeting~acme.webex.com~open:https://acme.webex.com/meet/ana | exact | Meetings.linkPattern ([\w-]+\.webex\.com/) |
| A13 | en | - | https://teams.microsoft.com/l/meetup-join/19%3ameeting | meeting~Join meeting~teams.microsoft.com~open:https://teams.microsoft.com/l/meetup-join/19%3ameeting | exact | Meetings.linkPattern (teams) |
| A14 | en | - | http://meet.google.com/abc | link~Open meet.google.com~http://meet.google.com/abc~open:http://meet.google.com/abc | approx | Meetings.linkPattern (only https, so it is not a meeting; the detector still finds a link) |
| A15 | en | - | https://example.com/j/123 | link~Open example.com~https://example.com/j/123~open:https://example.com/j/123 | approx | SmartClip.detect (.link) |
| A16 | en | - | hello@example.com | mail~Write to hello@example.com~In your email app~open:mailto:hello@example.com | approx | SmartClip.detect (mailto) |
| A17 | pt | BR | +55 11 98765-4321 | phone~Ligar para +55 11 98765-4321~No app de telefone~open:tel:+5511987654321 ;; whatsapp~Abrir conversa no WhatsApp~+55 11 98765-4321~open:https://wa.me/5511987654321 | approx | SmartClip.detect (.phoneNumber, wa.me) |
| A18 | pt | BR | (11) 98765-4321 | phone~Ligar para (11) 98765-4321~No app de telefone~open:tel:11987654321 ;; whatsapp~Abrir conversa no WhatsApp~(11) 98765-4321~open:https://wa.me/5511987654321 | approx | SmartClip.countryCodes (BR is 55, put before a number without +) |
| A19 | en | - | (11) 98765-4321 | phone~Call (11) 98765-4321~With your phone app~open:tel:11987654321 | approx | SmartClip.detect (no + and no known region: no WhatsApp) |
| A20 | en | US | (415) 555-2671 | phone~Call (415) 555-2671~With your phone app~open:tel:4155552671 ;; whatsapp~Open chat in WhatsApp~(415) 555-2671~open:https://wa.me/14155552671 | approx | SmartClip.countryCodes (US is 1) |
| A21 | en | - | call me at 11 98765-4321 tomorrow please | - | approx | SmartClip.detect (the number is less than 70% of the text) |
| A22 | en | - | 350 Fifth Avenue, New York, NY 10118 | address~Open in Maps~350 Fifth Avenue, New York, NY 10118~open:https://www.openstreetmap.org/search?query=350%20Fifth%20Avenue,%20New%20York,%20NY%2010118 | approx | SmartClip.detect (.address; the Mac opens Apple Maps) |
| A23 | pt | - | Av. Paulista, 1578 - Bela Vista, São Paulo | address~Abrir no Mapas~Av. Paulista, 1578 - Bela Vista, São Paulo~open:https://www.openstreetmap.org/search?query=Av.%20Paulista,%201578%20-%20Bela%20Vista,%20S%C3%A3o%20Paulo | approx | SmartClip.detect (.address, urlQueryAllowed) |
| A24 | en | - | www.example.org | link~Open www.example.org~http://www.example.org~open:http://www.example.org | approx | SmartClip.detect (.link) |
| A25 | en | - | 2026-11-14 | - | approx | SmartClip.detect (a date is not a phone number) |
| A26 | en | - | 192.168.100.100 | - | approx | SmartClip.detect (an IP address is not a phone number) |
| A27 | en | - | hello | - | exact | SmartClip.detect |
| A28 | en | - | REPEAT:a:401 | - | exact | SmartClip.actions (text.count <= 400) |
| A29 | en | - | REPEAT:a:400 | - | exact | SmartClip.actions (400 is allowed, and it is not a code) |

### Snippet variables (`SnippetStore.expandWithCursor`)

Now is Saturday 2026-11-14 15:05. "Back" is how many characters the cursor goes back after pasting.

| ID | Lang | Clipboard | Snippet | Result | Back | Mac source |
|---|---|---|---|---|---|---|
| B01 | en | - | {date} | Nov 14, 2026 | 0 | expandWithCursor (.dateTime.day().month().year()) |
| B02 | pt | - | {date} | 14 de nov. de 2026 | 0 | expandWithCursor, locale pt_BR |
| B03 | de | - | {date} | 14. Nov. 2026 | 0 | expandWithCursor, locale de_DE |
| B04 | es | - | {date} | 14 nov 2026 | 0 | expandWithCursor, locale es_ES |
| B05 | zh | - | {date} | 2026年11月14日 | 0 | expandWithCursor, locale zh_Hans_CN |
| B06 | en | - | {time} | 3:05 PM | 0 | expandWithCursor (.hour().minute()) |
| B07 | de | - | {time} | 15:05 | 0 | expandWithCursor |
| B08 | en | - | {weekday} | Saturday | 0 | expandWithCursor (.weekday(.wide)) |
| B09 | pt | - | {weekday} | sábado | 0 | expandWithCursor |
| B10 | de | - | {weekday} | Samstag | 0 | expandWithCursor |
| B11 | zh | - | {weekday} | 星期六 | 0 | expandWithCursor |
| B12 | en | - | {tomorrow} | Nov 15, 2026 | 0 | expandWithCursor (byAdding day 1) |
| B13 | en | x y | a {clipboard} b | a x y b | 0 | expandWithCursor ({clipboard}) |
| B14 | en | {date} | {clipboard} | {date} | 0 | expandWithCursor (the variables come before {clipboard}, so the copied text is not expanded) |
| B15 | en | - | Hi {cursor}, bye | Hi , bye | 5 | expandWithCursor (characters after the first {cursor}) |
| B16 | en | - | a{cursor}b{cursor}c | abc | 10 | expandWithCursor (the count includes the second marker, then it is dropped) |
| B17 | en | - | no variables | no variables | 0 | expandWithCursor |
| B18 | en | - | {date}\n{weekday}, {time} | Nov 14, 2026\nSaturday, 3:05 PM | 0 | expandWithCursor |

### Snippet fields (`SnippetStore.fields`, `apply`)

"Values" is `Name=value;Name=value`.

| ID | Snippet | Fields | Values | Applied | Mac source |
|---|---|---|---|---|---|
| C01 | Dear {input:Name}, {input: Name }. Re: {input:Topic} {input:} | Name,Topic | Name=Ana;Topic=x | Dear Ana, Ana. Re: x {input:} | fields (unique, trimmed, `[^}]+`), apply (`\{input:\s*Name\s*\}`) |
| C02 | no fields {date} | - | - | no fields {date} | fields |
| C03 | {input:A}{input:B}{input:A} | A,B | A=1;B=2 | 121 | fields (order of appearance), apply (all occurrences) |

### Entries, filters and titles (`ClipItem`, `ClipFilter`)

"Kind" is text, image or file. "Filters" lists the filters that match the entry (the Mac's all, text, link, image, file, pinned). "Color" is the swatch of the
list as `r,g,b`.

| ID | Lang | Kind | Pinned | Content | Filters | Title | Color | Mac source |
|---|---|---|---|---|---|---|---|---|
| D01 | en | text | no | hello | all,text | hello | - | ClipFilter.matches |
| D02 | en | text | no | https://github.com/Quost/menew | all,link | https://github.com/Quost/menew | - | ClipItem.url |
| D03 | en | text | no | see https://a.b | all,text | see https://a.b | - | ClipItem.url (the whole text must be the link) |
| D04 | en | text | no | ftp://a.b | all,text | ftp://a.b | - | ClipItem.url (only http and https) |
| D05 | en | text | no | https://github.com/a b | all,text | https://github.com/a b | - | ClipItem.url (no spaces) |
| D06 | en | image | no | Imagem 640×480 | all,image | Image 640×480 | - | ClipRow ("Imagem" is translated) |
| D07 | de | image | no | Imagem 640×480 | all,image | Bild 640×480 | - | ClipRow, L10n "Imagem" |
| D08 | en | file | no | /home/u/docs/report.pdf | all,file | report.pdf | - | ClipRow.fileTitle |
| D09 | en | file | no | /home/u/docs/report.pdf\n/home/u/a.txt\n/home/u/b.txt | all,file | report.pdf and 2 more | - | ClipRow.fileTitle ("%@ e mais %d") |
| D10 | pt | file | no | /home/u/docs/report.pdf\n/home/u/a.txt | all,file | report.pdf e mais 1 | - | ClipRow.fileTitle |
| D11 | en | text | yes | hello | all,text,pinned | hello | - | ClipFilter.pinned |
| D12 | en | text | no | \n  \n  hello  \nworld | all,text | hello | - | ClipItem.title (first line with content) |
| D13 | en | text | no | #6C5CE7 | all,text | #6C5CE7 | 108,92,231 | ClipItem.hexColor |
| D14 | en | text | no | 1e90ff | all,text | 1e90ff | 30,144,255 | ClipItem.hexColor (the # is optional) |
| D15 | en | text | no | #12345 | all,text | #12345 | - | ClipItem.hexColor (6 digits) |

### What is read from a copy (`ClipboardManager.poll`)

"Types" are the types the copy offers. "Plan" is the types to try, in order, as `kind:type`, or `skip` when the copy is marked as a password.

| ID | Types | Plan | Mac source |
|---|---|---|---|
| E01 | text/plain | text:text/plain | poll (string) |
| E02 | image/png | image:image/png | poll (image) |
| E03 | text/uri-list,text/plain | files:text/uri-list,text:text/plain | poll (file URLs come before the string) |
| E04 | image/png,text/html | image:image/png | poll |
| E05 | image/png,x-kde-passwordManagerHint | skip | poll (ignoredTypes, also for images) |
| E06 | text/plain;charset=utf-8,text/plain,UTF8_STRING | text:text/plain;charset=utf-8 | poll |
| E07 | text/uri-list,x-special/gnome-copied-files,text/plain | files:text/uri-list,text:text/plain | poll |
| E08 | text/plain,org.nspasteboard.TransientType | skip | ignoredTypes (TransientType) |
| E09 | text/uri-list,org.nspasteboard.ConcealedType | skip | ignoredTypes (ConcealedType, also for files) |
| E10 | image/jpeg,image/png | image:image/png | poll |
| E11 | text/html | - | poll (nothing to keep) |
| E12 | text/plain,com.agilebits.onepassword | skip | ignoredTypes (1Password) |

### File addresses (`readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true])`)

| ID | text/uri-list | Paths | Mac source |
|---|---|---|---|
| F01 | file:///home/u/a%20b.txt | /home/u/a b.txt | poll (paths joined with a line break) |
| F02 | # c\nfile:///tmp/a.png\nfile:///tmp/b.png | /tmp/a.png\n/tmp/b.png | poll |
| F03 | https://example.com/x | - | urlReadingFileURLsOnly |

### Image names (`ClipboardManager.poll`)

The name is the first 10 bytes of the SHA-256 of the image data, in hex, plus the extension.

| ID | Data | Type | Name | Mac source |
|---|---|---|---|---|
| G01 | abc | image/png | ba7816bf8f01cfea4141.png | poll (SHA256.hash(data:).prefix(10)) |
| G02 | abc | image/jpeg | ba7816bf8f01cfea4141.jpg | poll (the Mac converts to PNG; Linux keeps the type) |
| G03 | (empty) | image/png | e3b0c44298fc1c149afb.png | poll |

### Order, dedupe, pins and limits (`insert`, `sameContent`, `clear`, `prune`)

Operations are separated by `;`: `t:X` copy text X, `f:X` copy a file at path X, `i:X` copy the image whose file is X, `pin:X` pin or unpin the entry
X, `clear` clear the history, `n:K` copy K different texts, `prune:M` apply the limit of M entries. The result is the list as shown (pinned first, then the newest),
each as `kind:value` (`t`, `f`, `i`), with `*` after a pinned entry, or `#N` for the number of entries.

| ID | Operations | Result | Mac source |
|---|---|---|---|
| H01 | t:A;t:B;t:A | t:A,t:B | insert (the older twin leaves, the new one goes to the top) |
| H02 | t:A;pin:A;t:B;t:A | t:A*,t:B | insert (the pin is kept) |
| H03 | i:x.png;i:y.png;i:x.png | i:x.png,i:y.png | sameContent (images by file) |
| H04 | f:/a;t:/a | t:/a,f:/a | sameContent (the kind must be the same) |
| H05 | t:A;t:B;pin:B;clear | t:B* | clear(keepPinned: true) |
| H06 | n:25;prune:5 | #20 | prune (max(maxItems, 20)) |
| H07 | n:25;pin:n0;prune:20 | #21 | prune (pinned entries do not count and never leave) |
| H08 | n:300;prune:300 | #300 | prune (300 is the default) |
| H09 | n:301;prune:300 | #300 | prune |

### Pause (`ClipboardManager.isPaused`, `pause`, `resume`)

Operations: `pause:M@T` pause M minutes at time T seconds (`-` is until resumed), `resume`. "At" is the time of the question.

| ID | Operations | At | Paused | Mac source |
|---|---|---|---|---|
| I01 | pause:15@1000 | 1899 | yes | isPaused (pausedUntil > now) |
| I02 | pause:15@1000 | 1900 | no | isPaused |
| I03 | pause:60@0 | 3599 | yes | pause(minutes: 60) |
| I04 | pause:-@0 | 1000000000 | yes | pause(minutes: nil) |
| I05 | pause:-@0;resume | 5 | no | resume |
| I06 | pause:-@0;pause:15@100 | 500 | yes | pause (a timed pause replaces "until resumed") |
| I07 | pause:-@0;pause:15@100 | 1000 | no | pause |

### Ignored apps (`ignoredApps`)

The Mac compares bundle identifiers exactly; Linux compares application ids and window classes without case and without `.desktop`.

| ID | List | App | Capture | Mac source |
|---|---|---|---|---|
| J01 | org.keepassxc.KeePassXC | org.keepassxc.KeePassXC | ignored | poll (ignoredApps.contains) |
| J02 | org.keepassxc.KeePassXC | ORG.keepassxc.keepassxc | ignored | linux: case does not matter |
| J03 | Bitwarden.desktop | bitwarden | ignored | linux: `.desktop` does not matter |
| J04 | org.keepassxc.KeePassXC | firefox | yes | poll |
| J05 | org.keepassxc.KeePassXC | - | yes | poll (the app is unknown, so the copy is kept) |
| J06 | - | firefox | yes | poll (empty list) |

### Paste queue (`PasteQueue`)

Operations: `start`, `stop`, `cap:X` something copied, `add:X` "Add to paste queue", `rm:X`, `next` the step. The log has one word per operation:
`on`, `off`, `+X=n` joined (n in the queue), `-X` not joined, `rm`, `no` (not in the queue), `>X(n)` pasted with n left, `none`. "State" is `off` or the queue.

| ID | Operations | Log | State | Mac source |
|---|---|---|---|---|
| K01 | start;cap:A;cap:B;next;next | on,+A=1,+B=2,>A(1),>B(0) | off | start, captured, pasted (the last one stops the queue) |
| K02 | cap:A | -A | off | captured (only when on) |
| K03 | add:A | +A=1 | on[A] | add (starts the queue) |
| K04 | add:A;rm:A | +A=1,rm | off | remove (the last one stops the queue) |
| K05 | start;cap:A;cap:B;rm:A;next | on,+A=1,+B=2,rm,>B(0) | off | remove, pasted |
| K06 | start;cap:A;stop | on,+A=1,off | off | stop (the items are forgotten) |
| K07 | start;next | on,none | on[] | pasted (nothing to paste) |
| K08 | start;cap:A;rm:Z | on,+A=1,no | on[A] | remove (not in the queue) |

### In the palette (`Palette.update`)

"Flags" is `-`, `paused` (capture paused) or `queue` (queue on with 2 items). The palette has one snippet, id `s1`, title `Hello`, text `Hi {input:Name}`. "Items" are
the ids that must come first, in order (`...` ends the list: other items may follow). With "Not" the ids that must not be there.

| ID | Lang | Clipboard | Flags | Input | Items | Not | Mac source |
|---|---|---|---|---|---|---|---|
| P01 | pt | +55 11 98765-4321 | - | (empty) | smart.phone,smart.whatsapp,... | - | update (smart items first, 3 at most, when the search is empty) |
| P02 | en | #3B82F6 | - | (empty) | smart.color.RGB,smart.color.HSL,... | - | update |
| P03 | en | #3B82F6 | - | color | smart.color.RGB,smart.color.HSL | - | update (typed: by keywords) |
| P04 | en | #3B82F6 | - | zzz | - | smart.color.RGB | update |
| P05 | en | AB123456789BR | - | track | smart.tracking | - | update |
| P06 | en | - | - | hello | snippet.s1 | - | commands (snippets), Palette.update |
| P07 | en | - | - | snippet | snippet.s1 | - | commands (keywords start with "snippet") |
| P08 | en | - | - | pause | action.clippause | action.clipresume | commands (clippause) |
| P09 | en | - | paused | pause | action.clipresume | action.clippause | commands (clipresume) |
| P10 | en | - | queue | queue | action.queue,action.pastenext | - | linux: the step of the queue is an action |
| P11 | en | - | - | queue | action.queue | action.pastenext | linux |
