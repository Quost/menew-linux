#!/usr/bin/env bash
# Phase 5 checks (images, files, filters, pause, smart actions, snippets, paste queue, OCR).
# Sourced by gnome.sh and kde.sh after lib.sh.

db_sql() { sqlite3 "$MENEW_DB_PATH" "$1" 2>/dev/null; }
images_dir() { echo "${MENEW_DB_PATH%.db}.images"; }
count_kind() { db_sql "SELECT COUNT(*) FROM clipboard_entries WHERE kind='$1'"; }

# view_query <filter> [query]: the clipboard list through the test hook, results in the log
view_query() {
    PQ_START="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __clipboard-view "$1" "${2:-}" >/dev/null 2>&1 || true
    wait_for 10 bash -c "tail -n +$((PQ_START + 1)) '$MENEW_LOG' | grep -a -q 'Test hook: clipboard view'"
}
view_has() { log_since "$PQ_START" | grep -a -q -- "$1"; }
view_kinds_only() { # every clip[...] line of the last view is of this kind
    ! log_since "$PQ_START" | grep -a 'Test hook: clip\[' | grep -a -v "\] \[$1\]" | grep -q .
}
act0() { MENEW_TEST=1 "$MENEW_BIN" __palette-activate 0 >/dev/null 2>&1 || true; }
reads_logged() { grep -a -c 'Received clipboard\|Saved clipboard item' "$MENEW_LOG"; }

# needs from the calling script: DE, de_copy, de_clipboard_is, de_screenshot, de_copy_png <file>,
# de_copy_uris <text>, de_pause_probe
phase5_checks() {
    local png="$TEST_ROOT/ocr.png" shot="$REPO_ROOT/docs/desktop-tests/$DE-clipboard-images.png" f want
    convert -background white -fill black -pointsize 48 label:"MENEW OCR" "$png"

    # (a) a PNG copy is an image entry with a file on disk
    de_copy_png "$png"
    if wait_for 15 bash -c "[ \"\$(sqlite3 '$MENEW_DB_PATH' \"SELECT COUNT(*) FROM clipboard_entries WHERE kind='image'\")\" -ge 1 ]"; then
        pass "a copied PNG is an image entry in SQLite ($(db_sql "SELECT text_content || ' / ' || image_file FROM clipboard_entries WHERE kind='image' LIMIT 1"))"
    else fail "the copied PNG did not reach SQLite"; fi
    f="$(db_sql "SELECT image_file FROM clipboard_entries WHERE kind='image' ORDER BY id DESC LIMIT 1")"
    if [ -n "$f" ] && [ -s "$(images_dir)/$f" ]; then pass "the image is a file in the images folder: $(stat -c '%s bytes %n' "$(images_dir)/$f")"; else fail "no image file at $(images_dir)/$f"; fi
    de_copy_png "$png"; sleep 2
    if [ "$(count_kind image)" = "1" ]; then pass "the same image copied twice is one entry"; else fail "the same image made $(count_kind image) entries"; fi

    # (b) OCR finds the known word
    if wait_for 90 bash -c "sqlite3 '$MENEW_DB_PATH' \"SELECT ocr_text FROM clipboard_entries WHERE kind='image'\" | grep -q MENEW"; then
        pass "OCR read the image: '$(db_sql "SELECT replace(ocr_text, char(10), ' ') FROM clipboard_entries WHERE kind='image' LIMIT 1")'"
    else
        fail "OCR did not find MENEW (ocr_text='$(db_sql "SELECT ocr_text FROM clipboard_entries WHERE kind='image' LIMIT 1")')"; grep -a 'OCR' "$MENEW_LOG" | tail -3
    fi

    # (c) files
    de_copy_uris "file:///etc/hostname"
    if wait_for 15 bash -c "[ \"\$(sqlite3 '$MENEW_DB_PATH' \"SELECT text_content FROM clipboard_entries WHERE kind='file'\")\" = '/etc/hostname' ]"; then pass "a text/uri-list copy is a file entry (/etc/hostname)"; else fail "the uri-list copy is not a file entry"; fi

    # (d) filters and search
    de_copy "https://example.com/phase5"; wait_for 10 db_has "https://example.com/phase5" || fail "link not saved"
    de_copy "plain phase5 text"; wait_for 10 db_has "plain phase5 text" || fail "text not saved"
    if view_query image && view_has 'filter=image .* -> 1 entries' && view_kinds_only image; then pass "the Images filter shows only the image"; else fail "the Images filter is wrong"; log_since "$PQ_START" | grep -a 'Test hook' | head -5; fi
    if view_query file && view_has 'filter=file .* -> 1 entries' && view_kinds_only file; then pass "the Files filter shows only the file"; else fail "the Files filter is wrong"; fi
    if view_query link && view_has 'filter=link .* -> 1 entries'; then pass "the Links filter shows only the link"; else fail "the Links filter is wrong"; log_since "$PQ_START" | grep -a 'Test hook' | head -5; fi
    if view_query all MENEW && view_has '\[image\]'; then pass "searching MENEW finds the image by the text inside it"; else fail "the search did not find the image by its OCR text"; fi

    # (e) smart actions in the palette
    de_copy "+55 11 98765-4321"; sleep 2
    if palette_query "call" && palette_has "open:tel:+5511987654321"; then pass "palette 'call' offers the phone action for a copied number"; else fail "no phone action"; palette_show; fi
    de_copy "hello@example.com"; sleep 2
    if palette_query "write" && palette_has "open:mailto:hello@example.com"; then pass "palette 'write' offers the e-mail action"; else fail "no e-mail action"; palette_show; fi
    de_copy "#3B82F6"; sleep 2
    if palette_query "color" && palette_has "copy:rgb(59, 130, 246)"; then pass "palette 'color' offers the RGB copy of a copied HEX color"; else fail "no color action"; palette_show; fi

    # (f) snippet with {date}
    want="$(LC_ALL=C date +'%b %-d, %Y')"
    palette_query "today"
    if palette_has "paste-snippet:"; then
        act0
        if wait_for 10 de_clipboard_is "$want"; then pass "the snippet with {date} put '$want' on the clipboard"; else fail "clipboard is not '$want' after the {date} snippet"; fi
        sleep 3
        if db_has "$want"; then fail "the expanded snippet entered the history"; else pass "the expanded snippet was not kept in the history"; fi
    else fail "no snippet found for 'today'"; palette_show; fi

    # (g) pause: nothing is read while paused
    palette_query "pause" || fail "no answer for 'pause'"
    palette_has "pause-clipboard:60" || { fail "no pause action"; palette_show; }
    act0
    if wait_for 10 grep -a -q 'Clipboard capture paused' "$MENEW_LOG"; then pass "the pause action paused the capture (daemon log)"; else fail "no pause line in the log"; fi
    sleep 3
    local before after
    before="$(reads_logged)"
    de_copy "copied-while-paused-$DE"; sleep 4
    if db_has "copied-while-paused-$DE"; then fail "a copy made during the pause was saved"; else pass "a copy during the pause was not saved"; fi
    after="$(reads_logged)"
    if [ "$before" = "$after" ]; then pass "no clipboard read reached the daemon during the pause ('Received clipboard' + 'Saved clipboard item' log lines: $before before, $after after)"; else fail "the daemon logged reads during the pause ($before -> $after)"; fi
    de_pause_probe
    palette_query "resume" || true
    act0
    if wait_for 10 grep -a -q 'Clipboard capture resumed' "$MENEW_LOG"; then pass "resume put the capture back (daemon log)"; else fail "no resume line"; fi
    sleep 3
    de_copy "copied-after-resume-$DE"
    if wait_for 15 db_has "copied-after-resume-$DE"; then pass "after resuming, copies are kept again"; else fail "copy after resume not saved"; fi

    # (h) paste queue: two copies, then two steps
    palette_query "paste queue" || true
    if palette_has "toggle-queue"; then act0; else fail "no queue action"; fi
    if wait_for 10 grep -a -q 'Paste queue on' "$MENEW_LOG"; then pass "the paste queue is on"; else fail "the paste queue did not turn on"; fi
    sleep 1
    de_copy "queue-first-$DE"; wait_for 10 db_has "queue-first-$DE"; sleep 2
    de_copy "queue-second-$DE"; wait_for 10 db_has "queue-second-$DE"; sleep 2
    if [ "$(grep -a -c 'Paste queue: copied entry' "$MENEW_LOG")" = "2" ]; then pass "both copies joined the queue"; else fail "the copies did not join the queue ($(grep -a -c 'Paste queue: copied entry' "$MENEW_LOG"))"; fi
    "$MENEW_BIN" paste-next >/dev/null 2>&1 || true
    if wait_for 10 de_clipboard_is "queue-first-$DE"; then pass "paste-next put the first copied item on the clipboard"; else fail "paste-next did not give the first item"; fi
    sleep 2
    "$MENEW_BIN" paste-next >/dev/null 2>&1 || true
    if wait_for 10 de_clipboard_is "queue-second-$DE"; then pass "the second paste-next gave the second item"; else fail "the second paste-next did not give the second item"; fi
    grep -a 'Paste queue: next item' "$MENEW_LOG" | sed 's/\x1b\[[0-9;]*m//g' | sed 's/^/INFO: /'
    if grep -a -q 'Notification sent: Queue done' "$MENEW_LOG"; then pass "the queue ended with 'Queue done'"; else fail "no 'Queue done' notification"; fi

    # (i) image copy marked as a password, refused by the daemon
    image_hint_dbus_test

    # (j) screenshot
    view_query all || true
    sleep 2
    rm -f "$shot"; de_screenshot "$shot"
    if [ -s "$shot" ] && [ "$(distinct_colors "$shot")" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-clipboard-images.png ($(stat -c %s "$shot") bytes)"; else fail "no usable clipboard-images screenshot"; fi
}

# an image copy marked as a password, through the real D-Bus method of the daemon (all offered types are sent along)
image_hint_dbus_test() {
    local hex before out files_before
    convert -background white -fill black -pointsize 40 label:"SECRET IMAGE" "$TEST_ROOT/secret.png"
    hex="$(xxd -p "$TEST_ROOT/secret.png" | tr -d '\n' | sed 's/\(..\)/0x\1,/g; s/,$//')"
    before="$(count_kind image)"; files_before="$(ls "$(images_dir)" | wc -l)"
    out="$(gdbus call --session --dest org.menew.ClipboardService --object-path /org/menew/Clipboard --method org.menew.ClipboardService.PushClipboardData "image/png" "@ay [$hex]" "" "" "['image/png', 'x-kde-passwordManagerHint']" 2>&1 | cut -c1-120)"
    info "PushClipboardData(image with hint) -> $out"
    sleep 1
    if [ "$(count_kind image)" = "$before" ] && [ "$(ls "$(images_dir)" | wc -l)" = "$files_before" ] && echo "$out" | grep -q false; then pass "an image copy carrying a password hint is refused by the daemon (false, no new entry, no file written: $files_before files)"; else fail "the image with a password hint was accepted"; fi
}
