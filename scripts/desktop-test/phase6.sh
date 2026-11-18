#!/usr/bin/env bash
# Phase 6 checks (notes: files, Secret Service, palette, Passwords filter, confidential copy, polkit).
# Sourced by gnome.sh and kde.sh after lib.sh and phase5.sh. Needs from the caller: DE, de_copy,
# de_clipboard_is, de_screenshot, and the daemon started with MENEW_TEST=1 MENEW_SECRET_CLEAR_SECS=3.

NOTES_DIR() { echo "$XDG_DATA_HOME/menew/notes"; }
VALUE_PW='Zq9-unique-secret-8841'
VALUE_SECURE='Hh7-diary-secret-5530'

# note_create <kind> <text>: the hook creates the note, the id comes back from the daemon log
note_create() {
    local start; start="$(log_lines)"
    MENEW_TEST=1 "$MENEW_BIN" __notes-create "$1" "$2" >"$TEST_ROOT/hook.out" 2>&1
    echo "rc=$? env: $(env | grep -E '^(DBUS_SESSION|GNOME_KEYRING|SSH_AUTH|MENEW)' | tr '\n' ' ')" >> "$TEST_ROOT/hook.out"
    wait_for 15 bash -c "tail -n +$((start + 1)) '$MENEW_LOG' | grep -a -q 'Test hook: notes-create'" || return 1
    log_since "$start" | grep -a -o 'Test hook: note created id=[0-9A-F-]*' | head -1 | sed 's/.*id=//'
}

start_keyring() {
    command -v gnome-keyring-daemon >/dev/null 2>&1 || return 1
    command -v secret-tool >/dev/null 2>&1 || return 1
    # a throwaway login keyring in the temporary HOME, unlocked with a test password
    eval "$(printf 'menew-test-password' | gnome-keyring-daemon --daemonize --login --components=secrets 2>/dev/null)" || true
    wait_for 15 gdbus call --session --dest org.freedesktop.secrets --object-path /org/freedesktop/secrets \
        --method org.freedesktop.DBus.Peer.Ping
}

phase6_checks() {
    local shot="$REPO_ROOT/docs/desktop-tests/$DE-notes.png" id pwid secid keyring=1

    if start_keyring; then
        pass "gnome-keyring-daemon is running in the test session with an unlocked login keyring (org.freedesktop.secrets answers)"
    else
        keyring=0
        skip "no Secret Service in this session (gnome-keyring-daemon or secret-tool missing, or it did not start): the Secret Service checks below are NOT run"
    fi

    # (a) a normal note with a #tag lives in the plain file and is found in the palette
    id="$(note_create text 'Shopping list phase6\n#phase6tag\nmilk and bread')" || id=""
    if [ -n "$id" ]; then pass "a normal note was created ($id)"; else fail "the notes-create hook did not answer"; info "daemon log tail: $(tail -n 8 "$MENEW_LOG" | sed 's/\x1b\[[0-9;]*m//g' | cut -c12-200 | tr '\n' '|')"; info "hook output: $(sed 's/\x1b\[[0-9;]*m//g' "$TEST_ROOT/hook.out" | tail -n 4 | tr '\n' '|')"; return; fi
    if wait_for 5 grep -q "$id" "$(NOTES_DIR)/notes.json"; then pass "the note is in $(NOTES_DIR)/notes.json"; else fail "the note is not in notes.json"; fi
    if palette_query "phase6tag" && palette_has "open-note:$id"; then pass "palette 'phase6tag' finds the note by its #tag (open-note:$id)"; else fail "the palette did not find the note by its tag"; palette_show; fi
    if palette_query "shopping" && palette_has "[Note] Shopping list phase6 | phase6tag"; then pass "palette 'shopping' shows [Note] Shopping list phase6 with its second line (the Mac strips the leading # of the preview)"; else fail "palette 'shopping' did not show the note as expected"; palette_show; fi
    if palette_query "new note" && palette_has "new-note"; then pass "palette 'new note' offers the new-note action"; else fail "no new-note action"; fi

    # (b) a password note: the value is in the keyring and in no plain file
    if [ "$keyring" = "1" ]; then
        pwid="$(note_create passwords "Wi-Fi Demo\nUsername: demo\nPassword: $VALUE_PW")" || pwid=""
        if [ -n "$pwid" ]; then pass "a password note was created ($pwid)"; else fail "the password note was not created"; return; fi
        info "grep -r over the notes folder, the data folder and the history database:"
        if grep -r -l -F -- "$VALUE_PW" "$(NOTES_DIR)" "$XDG_DATA_HOME/menew" "$MENEW_DB_PATH" 2>/dev/null | grep -q .; then
            fail "the password value is in a plain file: $(grep -r -l -F -- "$VALUE_PW" "$(NOTES_DIR)" "$XDG_DATA_HOME/menew" "$MENEW_DB_PATH" 2>/dev/null | tr '\n' ' ')"
        else
            pass "grep -r -F '$VALUE_PW' over $(NOTES_DIR), $XDG_DATA_HOME/menew and the history database found nothing"
        fi
        info "grep -r -l -F '$VALUE_PW' <notes dir> <data dir> <db> printed: '$(grep -r -l -F -- "$VALUE_PW" "$(NOTES_DIR)" "$XDG_DATA_HOME/menew" "$MENEW_DB_PATH" 2>/dev/null | tr '\n' ' ')' (exit $(grep -r -q -F -- "$VALUE_PW" "$(NOTES_DIR)" "$XDG_DATA_HOME/menew" "$MENEW_DB_PATH" 2>/dev/null; echo $?); 1 = no match)"
        info "notes.json: $(cat "$(NOTES_DIR)/notes.json")"
        local looked; looked="$(secret-tool lookup application app.menew.Menew note "$pwid" 2>/dev/null || true)"
        info "secret-tool lookup application app.menew.Menew note $pwid printed: '$looked'"
        info "secret-tool search --all application app.menew.Menew: $(secret-tool search --all application app.menew.Menew 2>&1 | grep -a -E '^(label|attribute\.|secret)' | tr '\n' ' ')"
        if printf '%s' "$looked" | grep -q -F -- "$VALUE_PW"; then pass "secret-tool lookup application app.menew.Menew note $pwid returns the note with the value"; else fail "secret-tool lookup did not return the value (got: '${looked:0:60}')"; fi
        if palette_query "demo" && palette_has "open-note:$pwid" && palette_has "Protected content"; then pass "palette 'demo' (a word of the title) lists the password note with 'Protected content' only"; else fail "palette item for the password note is not as expected"; palette_show; fi
        if palette_query "Zq9-unique" && ! palette_has "open-note:"; then pass "palette 'Zq9-unique' (the value) finds no note: the content is not searchable"; else fail "the palette found a note by its secret value"; palette_show; fi

        # a protected note keeps only its title in the file too
        secid="$(note_create secure "Secret diary\nline with $VALUE_SECURE")" || secid=""
        if [ -n "$secid" ] && ! grep -r -l -F -- "$VALUE_SECURE" "$(NOTES_DIR)" "$XDG_DATA_HOME/menew" 2>/dev/null | grep -q . \
            && secret-tool lookup application app.menew.Menew note "$secid" 2>/dev/null | grep -q -F -- "$VALUE_SECURE"; then
            pass "a protected note: its text is in the keyring (secret-tool) and not in any plain file"
        else fail "the protected note is not stored as expected"; fi
    else
        skip "password note, grep -r over the plain files and secret-tool lookup: NOT run without a Secret Service"
    fi

    # (c) the Passwords filter, the confidential copy and the clear after the delay
    if [ "$keyring" = "1" ]; then
        PQ_START="$(log_lines)"
        MENEW_TEST=1 "$MENEW_BIN" __clipboard-view passwords "" >/dev/null 2>&1 || true
        wait_for 10 bash -c "tail -n +$((PQ_START + 1)) '$MENEW_LOG' | grep -a -q 'Test hook: clipboard view'" || true
        if view_has 'filter=passwords .* -> 1 entries' && view_has 'password\[0\] Wi-Fi Demo'; then pass "the Passwords filter lists only the password note, by title"; else fail "the Passwords filter is wrong"; log_since "$PQ_START" | grep -a 'Test hook' | head -4; fi

        local start; start="$(log_lines)"
        MENEW_TEST=1 "$MENEW_BIN" __password-paste 0 password >/dev/null 2>&1 || true
        if wait_for 15 de_clipboard_is "$VALUE_PW"; then pass "Enter on the password row put the password on the clipboard"; else fail "the clipboard does not hold the password"; log_since "$start" | grep -a 'Notes\|Test hook' | head -5; fi
        info "unlock path: $(log_since "$start" | grep -a 'Notes: unlock' | head -1)"
        sleep 4
        if db_has "$VALUE_PW"; then
            fail "the copied password entered the clipboard history"
            info "shell errors: $(grep -a -E 'JS ERROR|Error|error' "${SHELL_LOG:-/dev/null}" 2>/dev/null | grep -a -i menew | tail -3 | cut -c1-220 | tr '\n' '|')"
            info "what the bridge saw: $(grep -a -A4 'Ignoring' "${SHELL_LOG:-/dev/null}" 2>/dev/null | tail -9 | cut -c30-200 | tr '\n' '|')"
            info "daemon: $(log_since "$start" | grep -a -E 'Received clipboard|Saved clipboard|secret' | tail -3 | tr '\n' '|')"
        else pass "the copied password did not enter the history (type x-kde-passwordManagerHint)"; fi
        if log_since "$start" | grep -a -q 'was cleared from the clipboard'; then pass "after the test delay (3 s) the daemon cleared the clipboard"; else fail "no 'cleared from the clipboard' line"; fi
        if ! de_clipboard_is "$VALUE_PW"; then pass "the clipboard no longer holds the password"; else fail "the password is still on the clipboard after the delay"; fi

        # cleared only when it still holds the secret
        start="$(log_lines)"
        MENEW_TEST=1 "$MENEW_BIN" __password-paste 0 password >/dev/null 2>&1 || true
        wait_for 15 de_clipboard_is "$VALUE_PW" || fail "second copy of the password did not reach the clipboard"
        de_copy "something-else-$DE"
        sleep 6
        if log_since "$start" | grep -a -q 'left alone' && de_clipboard_is "something-else-$DE"; then pass "a newer copy was left alone: the clipboard still holds 'something-else-$DE'"; else fail "the newer copy was cleared or the log line is missing"; fi

        start="$(log_lines)"
        MENEW_TEST=1 "$MENEW_BIN" __password-paste 0 username >/dev/null 2>&1 || true
        if wait_for 15 de_clipboard_is "demo"; then pass "Alt+Enter (username) put 'demo' on the clipboard"; else fail "the username was not copied"; fi
    else
        skip "Passwords filter, confidential copy and the clear after the delay: NOT run without a Secret Service (the copy path needs the note content)"
    fi

    # (d) polkit: record what happens here
    info "polkit action installed here: $(pkaction --action-id app.menew.Menew.unlock-note 2>&1 | head -1 || true)"
    if [ "$keyring" = "1" ]; then
        if grep -a -q 'Notes: unlock of .* -> KeyringOnly' "$MENEW_LOG"; then
            pass "polkit cannot ask in this session ($(grep -a -o 'polkit could not ask ([^)]*)' "$MENEW_LOG" | head -1)): the fallback (keyring unlocked at login) ran and the daemon logged it"
        elif grep -a -q 'Notes: unlock of .* -> Polkit' "$MENEW_LOG"; then
            pass "polkit authorized the unlock"
        else
            fail "no unlock result in the daemon log"
        fi
    fi
    skip "polkit with an authentication agent (a real password prompt): no agent exists in a headless session, not run"

    # (e) the screenshot, with the fake notes above (open the normal note)
    MENEW_TEST=1 "$MENEW_BIN" __notes-open "$id" >/dev/null 2>&1 || true
    sleep 3
    rm -f "$shot"; de_screenshot "$shot"
    if [ -s "$shot" ] && [ "$(distinct_colors "$shot")" -gt 20 ]; then pass "screenshot docs/desktop-tests/$DE-notes.png ($(stat -c %s "$shot") bytes)"; else fail "no usable notes screenshot"; fi
}
