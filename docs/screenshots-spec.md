# Screenshots, text from the screen and QR codes: specification and test vectors

The Mac app is the specification (`../menew/Sources/Menew`). This file lists what the Mac code does with a given
input and where in the Mac code that comes from. The tables in "Vectors" are **executed**:
`crates/menew-core/tests/screenshots_spec.rs` reads this file, runs every row through the real Linux code and
compares the result. A row that does not match fails the test, and the test also fails if it executes a different
number of rows than the file has. The expected values were derived by hand from the Mac code (or from the QR
standard), not copied from the Linux output.

In a table cell `\n` stands for a line break, `-` is "nothing / none", `(e)` is an empty string and
`REPEAT:a:80` is the letter `a` 80 times. A cell in backticks keeps its spaces and backslashes as they are (only `\n`
is still a line break). Cells never contain a pipe character.

## Where each part comes from in the Mac code

| Part | Mac file / function | Linux |
|---|---|---|
| Capture an area or window (shortcut ⌃⌥⌘S) | `Capture.swift` `ScreenCapture.area` (`screencapture -i -x`) | `menew/src/portal.rs` `capture` (portal `org.freedesktop.portal.Screenshot`, `interactive`), shortcut `Ctrl+Alt+Shift+S`, `menew capture` |
| Capture the whole screen | `ScreenCapture.fullScreen` (the screen with the mouse) | the same portal, not interactive: `menew capture-full`, palette |
| The capture file is deleted when the image is open | `ScreenCapture.run`: `defer { removeItem }` | `menew/src/portal.rs` reads the file and removes it |
| Permission | `ScreenCapture.hasAccess` (Screen Recording) | the portal asks the user; a refusal or failure shows `cap_portal_failed` |
| Annotation editor (pen, arrow, rectangle, text, blur, undo) | `CaptureEditor`, `CaptureCanvas`, `CaptureMark` | `menew-ui/src/screen.rs` `Editor`; rules in `capture.rs` |
| Line width, font size, arrow head, outline | `CaptureDocument.lineWidth`, `fontSize`, `CaptureCanvas` | `capture.rs` `line_width`, `font_size`, `arrow_head_points`, `text_outline` |
| Blur (really pixelated, not a gray box) | `CaptureDocument.init` (`CIFilter.pixellate`), `CaptureCanvas` `.blur` | `capture.rs` `pixelate`, `blur_pixels`; drawn by `screen.rs` |
| Copy, save, drag to another app | `ScreenCapture.copy`, `save`, `CaptureEditor.dragProvider` | `screen.rs` |
| Name and place of the saved file | `ScreenCapture.save` (`NSSavePanel`, Desktop, `Captura <date>.png`) | `capture.rs` `screenshot_file_name`, `default_save_dir` |
| Copy text from the screen (⌃⌥⌘T) | `TextRecognizer.captureScreenText`, `finish` | `screen.rs` `Purpose::Text`, `ocr.rs` (tesseract), `capture.rs` `text_outcome` |
| "Copy text" button of the editor (reads the capture without the annotations) | `CaptureEditor.actions` | `screen.rs` |
| QR: read from the screen, never open the link | `QRCode.scanScreen`, `read` | `screen.rs` `Purpose::Qr`, `qr.rs` `decode_*`, `scan_outcome` |
| QR: show the code of the copied text, a link, the Wi-Fi | `QRCode.show`, `QRView`, `QRModel.payload`, `wifiPayload` | `screen.rs` `QrWindow`, `qr.rs` `encode`, `window_payload`, `wifi_payload` |
| "Show QR code for the link" in the smart actions | `SmartClip.actions` (`id: "qr"`) | `clip_smart.rs` (`SmartRun::ShowQr`) |
| Palette actions | `Palette.swift` `ocr`, `screenshot`, `screenshotfull`, `qrscan`, `qrshow`, `qrwifi` | `palette.rs` `commands` |
| Menu bar items | `App.swift` `captureText`, `scanQR`, `captureScreen` | `menew-tray` (the tray menu) |
| Strings | `L10n.swift` | `strings.rs` |

## Differences (also in `docs/parity.md`)

- **How the screen is captured**: the Mac runs `screencapture`. Linux asks the screenshot portal
  (`org.freedesktop.portal.Screenshot`, `interactive=true`), so the **system** shows the area / window / screen
  picker (GNOME and KDE both do). Menew never reads the framebuffer or the X11 root window. The portal saves
  its image in the pictures folder of the user; Menew reads it and **deletes it right away**, as the Mac does with
  its temporary file. If the portal fails there is a message (`cap_portal_failed`), no silent fallback.
- The Mac captures the **screen with the mouse** for the whole screen: the portal decides which screen it is.
- **OCR** is `tesseract` (Vision on the Mac): the same lines, from top to bottom, joined with `\n`. Without
  tesseract the message of Phase 5 (`ocr_missing`) is shown.
- **QR**: reading handles QR codes only (Vision also reads Aztec, Data Matrix, PDF417 and micro QR). It tries the
  image as it is and then inverted (dark pages). The image made by "Copy image" has a white border of 4 modules
  (the Mac copies the bare code and draws the border only in its window); it is 1024 pixels or a little more
  (a whole number of pixels per module), the Mac gives exactly 1024.
- **Blur**: the block size is `max(10, width / 90)` as on the Mac, rounded to whole pixels (`CIPixellate` takes a
  fractional scale), and the color of a block is the average of its pixels, rounded.
- **Retina**: the Mac works in points and exports at the scale of the screen. Linux works in pixels (the portal
  gives pixels); line width and font size use the image size in pixels, which for a 1x screen is the same.
- The saved file name uses the date as `2026-11-17 10.30.15` always; the Mac uses the date format of the region
  (and replaces `/` with `-` and `:` with `.`).
- The file dialog starts in the Desktop folder of the user (XDG), the home folder if there is none.
- Copying a capture does not save it to **Memory** (the Mac keeps a copy there, on by default): Memory is not part
  of release 1. The clipboard history keeps the copied image like any other copied image.
- Not in release 1 (see `docs/parity.md`): pin on screen, screen recording, scrolling capture, ask about the
  screen. The editor has no "Pin" button.

## Vectors

### A. File names (`ScreenCapture.save`, `dragProvider`)

Columns: id, language, date and time (year-month-day hour:minute:second), name of the saved file, name of the dragged file.

| id | lang | when | saved | dragged | Mac source |
|---|---|---|---|---|---|
| A01 | en | 2026-11-17 10:30:15 | Screenshot 2026-11-17 10.30.15.png | Screenshot.png | `L("Captura %@", date) + ".png"`, `L("Captura") + ".png"` |
| A02 | pt | 2026-11-17 10:30:15 | Captura 2026-11-17 10.30.15.png | Captura.png | L10n "Captura %@", "Captura" |
| A03 | de | 2026-11-17 10:30:15 | Bildschirmfoto 2026-11-17 10.30.15.png | Bildschirmfoto.png | L10n |
| A04 | es | 2026-11-17 10:30:15 | Captura 2026-11-17 10.30.15.png | Captura.png | L10n |
| A05 | zh | 2026-11-17 10:30:15 | 截图 2026-11-17 10.30.15.png | 截图.png | L10n |
| A06 | en | 2026-01-05 09:05:03 | Screenshot 2026-01-05 09.05.03.png | Screenshot.png | `.month(.twoDigits).day(.twoDigits)` |

### B. Which marks are kept (`CaptureEditor.drawing(k:).onEnded`, `CaptureMark.rect`)

Columns: id, tool, a, b, number of points (pen), kept, box `x,y,w,h`.

| id | tool | a | b | points | kept | box |
|---|---|---|---|---|---|---|
| B01 | arrow | 10,10 | 14,10 | 0 | no | 10,10,4,0 |
| B02 | arrow | 10,10 | 10,14.01 | 0 | yes | 10,10,0,4.01 |
| B03 | rect | 30,40 | 10,20 | 0 | yes | 10,20,20,20 |
| B04 | blur | 0,0 | 2,3 | 0 | no | 0,0,2,3 |
| B05 | blur | 0,0 | 3,4.5 | 0 | yes | 0,0,3,4.5 |
| B06 | pen | 5,5 | 9,9 | 2 | no | 5,5,4,4 |
| B07 | pen | 5,5 | 9,9 | 3 | yes | 5,5,4,4 |

### C. Sizes (`CaptureDocument`)

Columns: id, image width, image height, line width, font size, arrow head length, rectangle corner radius, blur block size.
(Line width is `max(3, min / 160)`, font size `max(16, min / 28)`, head `lw * 4.5`, radius `lw * 1.5`, block `max(10, width / 90)` rounded.)

| id | width | height | lw | font | head | radius | block |
|---|---|---|---|---|---|---|---|
| C01 | 1000 | 800 | 5 | 28.5714 | 22.5 | 7.5 | 11 |
| C02 | 200 | 100 | 3 | 16 | 13.5 | 4.5 | 10 |
| C03 | 3200 | 1600 | 10 | 57.1429 | 45 | 15 | 36 |
| C04 | 1920 | 1080 | 6.75 | 38.5714 | 30.375 | 10.125 | 21 |
| C05 | 900 | 500 | 3.125 | 17.8571 | 14.0625 | 4.6875 | 10 |
| C06 | 3840 | 2160 | 13.5 | 77.1429 | 60.75 | 20.25 | 43 |

### D. Arrow head (`CaptureCanvas`, `.arrow`)

The two strokes start at the tip `b` and go back at `angle + 0.83 pi` and `angle - 0.83 pi`, `lw * 4.5` long.
Columns: id, a, b, lw, end of the first stroke, end of the second stroke (to 0.01).

| id | a | b | lw | first | second |
|---|---|---|---|---|---|
| D01 | 0,0 | 100,0 | 4 | 84.507,9.163 | 84.507,-9.163 |
| D02 | 0,0 | 0,100 | 4 | -9.163,84.507 | 9.163,84.507 |

### E. Colors, outline and typed text (`CaptureEditor.colors`, `CaptureCanvas` `.text`, `commitText`)

Columns: id, what, input, result.

| id | what | input | result |
|---|---|---|---|
| E01 | colors | - | 255,59,48 255,149,0 255,204,0 52,199,89 0,122,255 175,82,222 0,0,0 255,255,255 |
| E02 | default color | - | 255,59,48 |
| E03 | default tool | - | arrow |
| E04 | tools | - | pen arrow rect text blur |
| E05 | outline | 0,0,0 | 255,255,255 1 |
| E06 | outline | 255,59,48 | 0,0,0 0.55 |
| E07 | outline | 255,255,255 | 0,0,0 0.55 |
| E08 | outline offset | 3 | 1 |
| E09 | outline offset | 13.5 | 4.5 |
| E10 | typed text | `  Look here \n` | Look here |
| E11 | typed text | `   ` | - |
| E12 | typed text | (e) | - |

### F. Blur

The test image is 120 x 90 pixels, 4 bytes per pixel, pixel `(x, y)` = `(x * 2 % 256, y * 3 % 256, (x + y) % 256, 255)`;
the blocks are 10 x 10 (`pixelate_scale(120)`), counted from the top left corner. Columns: id, marks (`x1,y1,x2,y2`
joined with `;`), the pixel boxes (`x0,y0,x1,y1`, end excluded, joined with `;`) that must show the pixelated image (`-`
none, `all` the whole image); **every other pixel must be identical to the original**. In every row the pixels
of a box must differ from the original in at least 90% of the cases (the image is not flat), and be equal to the
average of their 10 x 10 block.

| id | marks | covered | Mac source |
|---|---|---|---|
| F01 | 20,20,60,50 | 20,20,60,50 | `.blur`: the pixelated image drawn inside the box |
| F02 | 60,50,20,20 | 20,20,60,50 | `CaptureMark.rect` (a dragged from the lower right) |
| F03 | 20.4,20.4,59.6,49.6 | 20,20,60,50 | the box is moved out to whole pixels |
| F04 | -10,-10,500,500 | all | the box is cut to the image |
| F05 | 200,200,300,300 | - | a box outside the image changes nothing |
| F06 | 0,0,30,30;90,60,120,90 | 0,0,30,30;90,60,120,90 | several blur marks |
| F07 | 25,25,26,26 | 25,25,26,26 | a one pixel box is a pixel of its block average |

Other rules of the blur:

| id | what | input | result | Mac source |
|---|---|---|---|---|
| G01 | block size of an image 1800 pixels wide | pixelate a 1800 x 20 image with a pixel gradient | the first block is 20 pixels wide: pixels 0 to 19 of row 0 are equal, pixel 20 is different | `Float(max(10, CGFloat(cg.width) / 90))` |
| G02 | a pixelated image pixelated again | the F image | the same image (the blocks are aligned) | |
| G03 | average rounded | 100 pixels, one 255 and 99 at 0 | 3 | approx: `CIPixellate` is not documented to average |
| G04 | average rounded | 100 pixels, fifty 255 and fifty 0 | 128 | approx |

### H. Wi-Fi payload (`QRCode.wifiPayload`)

`\ ; , : "` are escaped with a backslash; no password is `T:nopass`. Columns: id, network name, password, payload.

| id | name | password | payload |
|---|---|---|---|
| H01 | Casa | secret | `WIFI:T:WPA;S:Casa;P:secret;;` |
| H02 | Casa | (e) | `WIFI:T:nopass;S:Casa;;` |
| H03 | `a;b:c,d\e"f` | `p;w` | `WIFI:T:WPA;S:a\;b\:c\,d\\e\"f;P:p\;w;;` |
| H04 | Café 5G | senha | `WIFI:T:WPA;S:Café 5G;P:senha;;` |

### I. What the QR window shows (`QRModel.payload`)

Columns: id, mode (`text` or `wifi`), text, network name, password, payload (empty: the window shows the empty frame).

| id | mode | text | name | password | payload |
|---|---|---|---|---|---|
| I01 | text | `  https://a.b/c  ` | - | - | `https://a.b/c` |
| I02 | text | (e) | - | - | (e) |
| I03 | wifi | ignored | (e) | secret | (e) |
| I04 | wifi | ignored | X | (e) | `WIFI:T:nopass;S:X;;` |
| I05 | text | `  a\nb  ` | - | - | `a\nb` |

### J. A code made here is read back, and its size

The code is made at level M (`correctionLevel = "M"`), drawn with a border of 4 modules and read again. Columns:
id, text, modules per side (version of the standard, level M, byte mode), module size for 1024 pixels, side of the image.
The size of the code comes from the capacity table of ISO/IEC 18004 (level M, byte mode: version 1 holds 14 bytes, 2
holds 26, 3 holds 42, 4 holds 62, 5 holds 84, 10 holds 213, 11 holds 251; a version has `17 + 4 * v` modules).

| id | text | modules | scale for 1024 | side |
|---|---|---|---|---|
| J01 | Hello | 21 | 36 | 1044 |
| J02 | `https://example.com/a?b=c` | 25 | 32 | 1056 |
| J03 | `WIFI:T:WPA;S:Casa;P:secret;;` | 29 | 28 | 1036 |
| J04 | ação 你好 | 21 | 36 | 1044 |
| J05 | REPEAT:y:300 | 69 | 14 | 1078 |
| J06 | `0123456789` | 21 | 36 | 1044 |

The lengths in bytes: J02 is 25 (version 2), J03 is 28 (version 3), J04 is 13 in UTF-8 (version 1), J05 is 300 (version 13,
which holds 331; version 12 holds 287). J06 is numeric mode, so version 1 (it holds 34 digits).

### K. What the scan does with what it found (`QRCode.scanScreen`)

The first code is the one used. A link (the scheme starts with `http`, no spaces) is **only copied**: the
result has no "open" step. Columns: id, codes found (joined with `¶`), outcome, text for the clipboard.

| id | found | outcome | clipboard |
|---|---|---|---|
| K01 | `https://example.com` | link | `https://example.com` |
| K02 | `http://a.b/c?d=e` | link | `http://a.b/c?d=e` |
| K03 | `WIFI:T:WPA;S:Casa;P:secret;;` | content | `WIFI:T:WPA;S:Casa;P:secret;;` |
| K04 | `mailto:a@b.c` | content | `mailto:a@b.c` |
| K05 | `hello world` | content | `hello world` |
| K06 | `ftp://example.com` | content | `ftp://example.com` |
| K07 | (e) | none | - |
| K08 | `a¶https://x.org` | content | `a` |
| K09 | `httpfoo://x` | link | `httpfoo://x` |
| K10 | `https://exa mple.com` | content | `https://exa mple.com` |
| K11 | `javascript:alert(1)` | content | `javascript:alert(1)` |

(K09 is how the Mac behaves: `hasPrefix("http")`.) The rule that nothing is opened is also checked as a guard: the
file `crates/menew-ui/src/screen.rs` must not call `launch_default_for_uri`, `open_uri`, `xdg-open` or `Command`
(test `the_scan_code_never_opens_anything`), and the desktop test runs a scan of a link with fake `xdg-open` and
browser programs and the OpenURI portal watched (`scripts/desktop-test/phase7.sh`).

### L. What the text reading does (`TextRecognizer.finish`)

Columns: id, recognized text, outcome, text for the clipboard, number of lines in the message.

| id | recognized | outcome | clipboard | lines |
|---|---|---|---|---|
| L01 | `hello\nworld` | copy | `hello\nworld` | 2 |
| L02 | `  one line \n` | copy | `one line` | 1 |
| L03 | (e) | none | - | - |
| L04 | `   \n  ` | none | - | - |
| L05 | `a\n\nb` | copy | `a\n\nb` | 2 |
| L06 | `a\nb\nc` | copy | `a\nb\nc` | 3 |

### M. The palette (`Palette.commands`)

Columns: id, language, query, item id, title, subtitle, action, shortcut.

| id | lang | query | item | title | subtitle | action | shortcut |
|---|---|---|---|---|---|---|---|
| M01 | en | screenshot | action.screenshot | Capture screen | Area or window, with annotations | capture | Ctrl+Alt+Shift+S |
| M02 | en | whole screen | action.screenshotfull | Capture the whole screen | - | capture-full | - |
| M03 | en | text from screen | action.ocr | Copy text from screen | - | capture-text | Ctrl+Alt+Shift+T |
| M04 | en | qr scan | action.qrscan | Read QR code from screen | - | qr-scan | - |
| M05 | en | qr show | action.qrshow | Show QR code of copied text | To open on your phone | qr-show | - |
| M06 | en | wifi qr | action.qrwifi | Wi-Fi QR code | - | qr-wifi | - |
| M07 | pt | Capturar tela | action.screenshot | Capturar tela | Área ou janela, com anotações | capture | Ctrl+Alt+Shift+S |
| M08 | de | Bildschirm aufnehmen | action.screenshot | Bildschirm aufnehmen | Bereich oder Fenster, mit Anmerkungen | capture | Ctrl+Alt+Shift+S |
| M09 | es | Capturar pantalla | action.screenshot | Capturar pantalla | Área o ventana, con anotaciones | capture | Ctrl+Alt+Shift+S |
| M10 | zh | 截屏 | action.screenshot | 截屏 | 区域或窗口，可标注 | capture | Ctrl+Alt+Shift+S |
| M11 | pt | Ler QR code da tela | action.qrscan | Ler QR code da tela | - | qr-scan | - |

### N. Where the new strings come from

Every string of the capture, the editor and the QR window is in the five languages and, when it exists on the Mac, is
the text of `L10n.swift` (test `n_every_new_string_is_in_the_five_languages`, which reads the keys listed here).

| id | key | English | Português | Mac key |
|---|---|---|---|---|
| N01 | cap_copied | Screenshot copied | Captura copiada | Captura copiada |
| N02 | cap_saved | Screenshot saved | Captura salva | Captura salva |
| N03 | cap_save_failed | Couldn't save the screenshot | Não deu para salvar a captura | Não deu para salvar a captura |
| N04 | qr_none | No QR code found | Nenhum QR code encontrado | Nenhum QR code encontrado |
| N05 | qr_link_copied | QR link copied | Link do QR copiado | Link do QR copiado |
| N06 | qr_content_copied | QR content copied | Conteúdo do QR copiado | Conteúdo do QR copiado |
| N07 | qr_image_copied | QR code copied | QR code copiado | QR code copiado |
| N08 | cap_tool_blur | Blur (hides sensitive data) | Borrar (esconde dados sensíveis) | Borrar (esconde dados sensíveis) |
| N09 | cap_drag | Drag into another app | Arraste para outro app | Arraste para outro app |
| N10 | qr_hint_wifi | Point your phone's camera to join the network. | Aponte a câmera do celular para entrar na rede. | Aponte a câmera do celular para entrar na rede. |
