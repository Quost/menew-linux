#!/usr/bin/env bash
# Phase 7 checks (screenshots, annotation editor, text from the screen, QR codes).
# Sourced by gnome.sh and kde.sh after phase6.sh. Needs from the calling script: DE, de_copy, de_clipboard_is,
# de_screenshot, MENEW_LOG, TEST_ROOT, REPO_ROOT, MENEW_DB_PATH, and the helpers of lib.sh and phase5.sh.
#
# What is real and what is substituted (also in scripts/desktop-test/README.md):
# - the screenshot portal itself (org.freedesktop.portal.Screenshot) is tried at the end with `menew capture-full`
#   and its result is recorded; if no portal backend can take a picture in this headless session the picture is
#   fed through the test hook `MENEW_TEST=1 menew __capture-from <edit|text|qr> <png>` and the portal is NOT tested;
# - everything after the picture (OCR, QR reading and making, the editor and its export, the copy to the clipboard,
#   the save with the name of the Mac, the palette and tray entries) is the real code of the daemon.

P7_START=0
p7_mark() { P7_START="$(log_lines)"; }
p7_log_has() { log_since "$P7_START" | grep -a -F -q -- "$1"; }
p7_wait() { # p7_wait <seconds> <text>: the text shows up in the daemon log after p7_mark
    local i
    for ((i = 0; i < $1 * 4; i++)); do
        if p7_log_has "$2"; then return 0; fi
        sleep 0.25
    done
    return 1
}
p7_hook() { MENEW_TEST=1 "$MENEW_BIN" "$@" >/dev/null 2>&1 || true; }
p7_images() { count_kind image; }
# the number of history rows; sqlite3 can be busy for a moment (exit 5), so it is asked again
p7_rows() { local i n; for i in 1 2 3 4 5 6 7 8; do n="$(sqlite3 "$MENEW_DB_PATH" "SELECT COUNT(*) FROM clipboard_entries" 2>/dev/null)" && { echo "$n"; return 0; }; sleep 0.5; done; echo -1; }

phase7_checks() {
    local T="$TEST_ROOT/p7" W="$TEST_ROOT/p7-watch" f out before rows
    mkdir -p "$T" "$W"
    local link='https://example.com/phase7?x=1&y=2' wifi='WIFI:T:WPA;S:Casa;P:secret;;'

    # ---- fixtures: images made with ImageMagick, QR codes made by the app ----
    convert -font DejaVu-Sans -background white -fill black -pointsize 64 label:"MENEW CAPTURE" "$T/text1.png"
    convert -font DejaVu-Sans -background white -fill black -pointsize 64 label:$'FIRST LINE\nSECOND LINE' "$T/text2.png"
    convert -size 600x300 xc:white "$T/blank.png"
    convert -size 400x300 gradient:'#d02020-#2030d0' -seed 7 -attenuate 0.6 +noise Gaussian \
        \( -background none -fill white -pointsize 36 label:"SECRET 1234" \) -geometry +20+80 -composite -depth 8 -alpha off "PNG24:$T/base.png"
    if [ -s "$T/text1.png" ] && [ -s "$T/base.png" ]; then pass "phase 7 fixtures made (text images, a noisy 400x300 picture)"; else fail "ImageMagick could not make the phase 7 fixtures"; return; fi

    # ---- (a) copy text from the screen: OCR of a generated image with a known text ----
    p7_mark
    p7_hook __capture-from text "$T/text1.png"
    if wait_for 90 de_clipboard_is "MENEW CAPTURE"; then pass "text from the screen: 'MENEW CAPTURE' read by tesseract is on the clipboard"; else fail "text from the screen: the clipboard does not hold 'MENEW CAPTURE'"; log_since "$P7_START" | grep -a -E 'OCR|Notification|Text from' | head -5; fi
    if p7_log_has "Notification sent: Reading text" && p7_log_has "Notification sent: Text copied"; then pass "the messages 'Reading text…' then 'Text copied' were sent"; else fail "the OCR messages are missing"; fi
    if wait_for 15 db_has "MENEW CAPTURE"; then pass "the copied text is in the clipboard history like any copied text"; else fail "the copied text did not reach the history"; fi
    if ls "$XDG_RUNTIME_DIR"/menew/ocr-*.png >/dev/null 2>&1; then fail "a temporary image of the OCR is still in $XDG_RUNTIME_DIR/menew"; else pass "no temporary image of the capture is left behind"; fi
    if [ -s "$T/text1.png" ]; then pass "the test hook kept its own file (only files of the portal are deleted)"; else fail "the hook deleted the fixture"; fi

    p7_mark
    p7_hook __capture-from text "$T/text2.png"
    if p7_wait 90 "Notification sent: Text copied (2 lines)"; then pass "two lines of text: the message says 'Text copied (2 lines)'"; else fail "no 'Text copied (2 lines)' message"; log_since "$P7_START" | grep -a -E 'OCR|Notification' | head -5; fi
    if [ "$(db_sql "SELECT COUNT(*) FROM clipboard_entries WHERE text_content LIKE 'FIRST LINE%SECOND LINE'")" -ge 1 ] 2>/dev/null || wait_for 15 bash -c "[ \"\$(sqlite3 '$MENEW_DB_PATH' \"SELECT COUNT(*) FROM clipboard_entries WHERE text_content LIKE 'FIRST LINE%SECOND LINE'\")\" -ge 1 ]"; then
        pass "both lines are one copied text: '$(db_sql "SELECT replace(text_content, char(10), '/') FROM clipboard_entries WHERE text_content LIKE 'FIRST LINE%SECOND LINE' LIMIT 1")'"
    else fail "the two lines did not reach the history as one text"; fi

    rows="$(p7_rows)"
    p7_mark
    p7_hook __capture-from text "$T/blank.png"
    if p7_wait 60 "Notification sent: No text found"; then pass "an image without text says 'No text found'"; else fail "no 'No text found' message"; fi
    sleep 2
    if [ "$(p7_rows)" = "$rows" ]; then pass "...and the clipboard is left alone (no new history entry)"; else fail "a blank image changed the clipboard history"; fi

    # ---- (b) QR codes: made by the app, read from a screen-sized picture ----
    p7_hook __qr-image "$link" "$T/qr-link.png"
    p7_hook __qr-image "$wifi" "$T/qr-wifi.png"
    if wait_for 10 test -s "$T/qr-link.png" && wait_for 10 test -s "$T/qr-wifi.png"; then
        pass "the app made the QR images ($(identify -format '%wx%h' "$T/qr-link.png") for the link, $(identify -format '%wx%h' "$T/qr-wifi.png") for the Wi-Fi)"
    else fail "the QR images were not made"; return; fi
    case "$(identify -format '%w' "$T/qr-link.png")" in 10[2-9][0-9]) pass "the image of the code is 1024 pixels or a little more";; *) fail "unexpected QR image size";; esac
    convert -size 1280x800 gradient:'#223355-#88aacc' "$T/qr-link.png" -gravity center -composite "$T/screen-link.png"
    convert -size 1280x800 gradient:'#335522-#aacc88' "$T/qr-wifi.png" -gravity center -composite "$T/screen-wifi.png"

    # nothing may open an address: watch the session bus for portal/launcher calls and the daemon log for its own open
    dbus-monitor --session "interface='org.freedesktop.portal.OpenURI'" "member='OpenURI'" "member='OpenFile'" "interface='org.freedesktop.impl.portal.AppChooser'" >"$W/open.log" 2>&1 &
    track $!
    sleep 1
    p7_mark
    p7_hook __capture-from qr "$T/screen-link.png"
    if wait_for 30 de_clipboard_is "$link"; then pass "QR scan: the content of the code (a link) is on the clipboard: $link"; else fail "QR scan: the clipboard does not hold the link"; log_since "$P7_START" | grep -a -E 'QR|Notification' | head; fi
    if p7_log_has "QR scan: 1 code(s) found, outcome link copied" && p7_log_has "Notification sent: QR link copied"; then pass "the scan said 'QR link copied' (daemon log)"; else fail "no 'QR link copied' in the log"; fi
    sleep 3
    if log_since "$P7_START" | grep -a -q -E 'Opened |Could not open'; then fail "the scan tried to open an address"; log_since "$P7_START" | grep -a -E 'Opened |Could not open' | head -3; else pass "the scan did NOT open the link (no 'Opened' or 'Could not open' line in the daemon log)"; fi
    if grep -a -q -E 'OpenURI|OpenFile|AppChooser' "$W/open.log"; then fail "a launcher call went over the session bus during the scan"; grep -a -E 'OpenURI|OpenFile|AppChooser' "$W/open.log" | head -3; else pass "no OpenURI / OpenFile call on the session bus (dbus-monitor, $(wc -l < "$W/open.log") lines of output, none a launcher call)"; fi
    if pgrep -u "$(id -u)" -f 'xdg-open|firefox|chromium|epiphany|falkon|konqueror|www-browser' >/dev/null; then fail "a browser or xdg-open process is running"; else pass "no browser or xdg-open process was started"; fi

    # the same watch must see an open when one is asked for (positive control), so the checks above mean something
    de_copy "https://example.com/p7-control"
    wait_for 10 db_has "https://example.com/p7-control" || true
    sleep 1
    palette_query "" || true
    if palette_has "open:https://example.com/p7-control"; then
        p7_mark
        MENEW_TEST=1 "$MENEW_BIN" __palette-activate 0 >/dev/null 2>&1 || true
        if p7_wait 10 "Opened https://example.com/p7-control" || p7_wait 2 "Could not open https://example.com/p7-control"; then
            pass "control: opening a link from the palette does show in the same log ($(log_since "$P7_START" | grep -a -E 'Opened |Could not open' | head -1 | cut -c1-120))"
        else fail "control: the palette open of a link left no line in the log, so the check above proves nothing"; fi
    else
        skip "control: the empty palette did not offer the open action for the copied link (the log check above is weaker); palette showed: $(log_since "$PQ_START" | grep -a 'Test hook: result' | head -3 | tr '\n' '|' | cut -c1-200)"
    fi

    p7_mark
    p7_hook __capture-from qr "$T/screen-wifi.png"
    if wait_for 30 de_clipboard_is "$wifi"; then pass "QR scan of a Wi-Fi code: the content is copied as it is ($wifi)"; else fail "QR scan: the clipboard does not hold the Wi-Fi payload"; fi
    if p7_wait 10 "outcome content copied" && p7_log_has "Notification sent: QR content copied"; then pass "...and the message is 'QR content copied'"; else fail "no 'QR content copied' message"; fi

    p7_mark
    p7_hook __capture-from qr "$T/blank.png"
    if p7_wait 30 "Notification sent: No QR code found"; then pass "a picture without a code says 'No QR code found'"; else fail "no 'No QR code found' message"; fi

    # the window that shows a code
    p7_mark
    p7_hook __qr-show "https://example.com/show-p7"
    if p7_wait 15 'QR window payload=Some("https://example.com/show-p7")'; then pass "the QR window opened with the code of a link (payload logged)"; else fail "the QR window did not open with the link"; log_since "$P7_START" | grep -a QR | head -3; fi
    local qrshot="$REPO_ROOT/docs/desktop-tests/$DE-qr.png"
    rm -f "$qrshot"; sleep 1; de_screenshot "$qrshot"
    if [ -s "$qrshot" ] && [ "$(distinct_colors "$qrshot")" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-qr.png ($(stat -c %s "$qrshot") bytes)"; else fail "no usable QR window screenshot"; fi
    de_copy "text-for-the-qr-p7"
    wait_for 10 db_has "text-for-the-qr-p7" || true
    sleep 1
    p7_mark
    p7_hook __qr-show
    if p7_wait 15 'QR window payload=Some("text-for-the-qr-p7")'; then pass "without a text the QR window takes the copied text"; else fail "the QR window did not take the copied text"; log_since "$P7_START" | grep -a QR | head -3; fi
    # the image made for the code is read back by the scan (the loop of 'show the code' and 'read the code')
    p7_hook __qr-image "text-for-the-qr-p7" "$T/qr-text.png"
    wait_for 10 test -s "$T/qr-text.png" && convert -size 1000x700 xc:white "$T/qr-text.png" -gravity southeast -composite "$T/screen-text.png"
    p7_mark
    p7_hook __capture-from qr "$T/screen-text.png"
    if wait_for 30 de_clipboard_is "text-for-the-qr-p7"; then pass "the image of 'show the code' is read back by 'read the code' (round trip through the UI code)"; else fail "round trip failed"; fi

    # ---- (c) the editor: export with a blur, other marks, save, copy ----
    p7_mark
    p7_hook __annotate "$T/base.png" "$T/blur.png" "blur:20,20,220,120"
    if wait_for 20 test -s "$T/blur.png" && p7_wait 5 "Test hook: annotate ok"; then pass "the export of the editor wrote $T/blur.png ($(identify -format '%wx%h %m' "$T/blur.png"))"; else fail "the annotate hook wrote no image"; log_since "$P7_START" | grep -a -i annotate | head -3; fi
    if out="$(python3 "$HERE/p7check.py" blur "$T/base.png" "$T/blur.png" 20,20,220,120)"; then pass "$out"; else fail "$out"; fi

    p7_hook __annotate "$T/base.png" "$T/rect.png" "blur:20,20,220,120;rect:250,150,380,280:red"
    wait_for 20 test -s "$T/rect.png"
    if out="$(python3 "$HERE/p7check.py" blur "$T/base.png" "$T/rect.png" 20,20,220,120 245,145,386,286)"; then pass "blur and a rectangle together: $out"; else fail "blur and a rectangle: $out"; fi
    if out="$(python3 "$HERE/p7check.py" marks "$T/base.png" "$T/rect.png" 1500 20,20,220,120 246,146,384,284 probe 250,215,255,59,48,6 380,215,255,59,48,6)"; then pass "the red rectangle is where it should be: $out"; else fail "rectangle check: $out"; fi

    p7_hook __annotate "$T/base.png" "$T/shapes.png" "arrow:20,250,200,150:blue;text:30,30:black:Hello;pen:300,20 320,40 340,20 360,40:green"
    wait_for 20 test -s "$T/shapes.png"
    if out="$(python3 "$HERE/p7check.py" marks "$T/base.png" "$T/shapes.png" 400 15,145,205,255 24,24,130,62 295,15,365,45)"; then pass "arrow, text and pen only change pixels near their own coordinates: $out"; else fail "shapes check: $out"; fi

    mkdir -p "$T/saved"
    p7_mark
    p7_hook __capture-save "$T/base.png" "$T/saved"
    sleep 2
    f="$(ls "$T/saved" | head -1)"
    if echo "$f" | grep -q -E '^Screenshot [0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}\.[0-9]{2}\.[0-9]{2}\.png$'; then pass "Save writes the Mac's name: '$f'"; else fail "the saved file is called '$f'"; fi
    if [ -n "$f" ] && [ "$(identify -format '%m %wx%h' "$T/saved/$f")" = "PNG 400x300" ] && [ "$(compare -metric AE "$T/base.png" "$T/saved/$f" null: 2>&1)" = "0" ]; then pass "...a PNG of 400x300 identical to the capture without marks (compare AE = 0)"; else fail "the saved file is not the capture ($(compare -metric AE "$T/base.png" "$T/saved/$f" null: 2>&1))"; fi

    # Copy: the capture with its blur goes to the clipboard and, like any copied image, to the history
    before="$(p7_images)" || before=0
    p7_mark
    p7_hook __capture-copy "$T/base.png" "blur:20,20,220,120"
    if wait_for 60 bash -c "[ \"\$(sqlite3 '$MENEW_DB_PATH' \"SELECT COUNT(*) FROM clipboard_entries WHERE kind='image'\")\" -gt $before ]"; then
        f="$(db_sql "SELECT image_file FROM clipboard_entries WHERE kind='image' ORDER BY id DESC LIMIT 1")" || f=""
        pass "Copy: the annotated capture reached the clipboard history as an image ($f, $(identify -format '%m %wx%h' "$(images_dir)/$f"))"
        if out="$(python3 "$HERE/p7check.py" blur "$T/base.png" "$(images_dir)/$f" 20,20,220,120)"; then pass "...and its blur is intact after the clipboard round trip: $out"; else fail "the copied image: $out"; fi
    else fail "Copy: no new image entry in the history"; log_since "$P7_START" | grep -a -E 'capture|Capture|Notification' | head -5; fi
    if p7_log_has "Notification sent: Screenshot copied"; then pass "the message 'Screenshot copied' was sent"; else fail "no 'Screenshot copied' message"; fi

    # the editor window itself
    p7_mark
    p7_hook __capture-from edit "$T/base.png"
    if p7_wait 15 "Capture editor shown: 400 x 300 pixels, 0 marks"; then pass "the editor window opened on the capture (400 x 300, no marks)"; else fail "the editor did not open"; log_since "$P7_START" | grep -a -i -E 'capture|error|warn' | head -5; fi
    local edshot="$REPO_ROOT/docs/desktop-tests/$DE-capture-editor.png"
    rm -f "$edshot"; sleep 2; de_screenshot "$edshot"
    if [ -s "$edshot" ] && [ "$(distinct_colors "$edshot")" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-capture-editor.png ($(stat -c %s "$edshot") bytes)"; else fail "no usable editor screenshot"; fi

    # ---- (d) the palette and the tray ----
    palette_query "screenshot" || true
    if palette_has "Capture screen | Area or window, with annotations | capture"; then pass "palette 'screenshot' offers 'Capture screen' (action capture)"; else fail "no capture item in the palette"; palette_show; fi
    palette_query "text from screen" || true
    if palette_has "Copy text from screen"; then pass "palette 'text from screen' offers 'Copy text from screen'"; else fail "no OCR item in the palette"; palette_show; fi
    palette_query "qr" || true
    if palette_has "Read QR code from screen" && palette_has "Show QR code of copied text" && palette_has "Wi-Fi QR code"; then pass "palette 'qr' offers read, show and Wi-Fi"; else fail "the QR items are not all in the palette"; palette_show; fi
    de_copy "https://example.com/p7-smart"
    wait_for 10 db_has "https://example.com/p7-smart" || true
    sleep 1
    palette_query "qr code for the link" || true
    if palette_has "qr-show:https://example.com/p7-smart"; then pass "a copied link gets the action 'Show QR code for the link' in the palette"; else fail "no QR action for a copied link"; palette_show; fi

    local dest found
    dest="$(tray_item_dest)"
    if [ -n "$dest" ]; then
        for f in 'Capture screen' 'Copy text from screen' 'Read QR code from screen'; do
            if tray_menu_find "$dest" "$f" >/dev/null; then pass "tray menu has '$f'"; else fail "tray menu lacks '$f'"; fi
        done
    else fail "no tray item registered"; fi

    # ---- (e) the screenshot portal itself ----
    # a headless session has no portal running: start the frontend and the backend of the desktop by hand and record it
    local desk=GNOME
    [ "$DE" = kde ] && desk=KDE
    if ! gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop --method org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
        if [ "$DE" = gnome ] && [ -x /usr/libexec/xdg-desktop-portal-gnome ]; then
            XDG_CURRENT_DESKTOP=GNOME /usr/libexec/xdg-desktop-portal-gnome -r >"$W/portal-backend.log" 2>&1 &
            track $!
        fi
        XDG_CURRENT_DESKTOP=$desk /usr/libexec/xdg-desktop-portal -r >"$W/portal.log" 2>&1 &
        track $!
        sleep 3
        if gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop --method org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
            info "xdg-desktop-portal started by hand in the test session (XDG_CURRENT_DESKTOP=$desk)"
        else
            info "xdg-desktop-portal could not be started: $(tail -3 "$W/portal.log" 2>/dev/null | tr '\n' '|' | cut -c1-300)"
        fi
    fi
    p7_mark
    "$MENEW_BIN" capture-full >/dev/null 2>&1 || true
    if p7_wait 40 "Capture editor shown"; then
        pass "the real screenshot portal took a picture: $(log_since "$P7_START" | grep -a 'Capture editor shown' | head -1 | cut -c1-120)"
    elif p7_wait 1 "Capture failed"; then
        skip "the screenshot portal could not take a picture in this headless session, so it was NOT tested: $(log_since "$P7_START" | grep -a 'Capture failed' | head -1 | cut -c1-240)"
    else
        skip "the screenshot portal gave no answer in 40 s in this headless session, so it was NOT tested (the pictures above came from the test hook)"
    fi
    if [ -n "$dest" ]; then
        found="$(tray_menu_find "$dest" 'Read QR code from screen')" || found=""
        if [ -n "$found" ]; then
            p7_mark
            tray_menu_click "$dest" "${found%% *}" >/dev/null 2>&1 || true
            if p7_wait 10 "Tray: QR code from the screen requested"; then pass "clicking 'Read QR code from screen' in the tray reached the daemon (daemon log)"; else fail "the tray click did not reach the daemon"; fi
        fi
    fi
}
