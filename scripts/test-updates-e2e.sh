#!/usr/bin/env bash
# End-to-end test of the in-app updater, no real network: a local `python3 -m http.server` serves a fake GitHub
# API listing, an `update.json` written and signed by scripts/make-update-json.sh with a TEST key, and "new"
# artifacts. The app is pointed at it with MENEW_UPDATE_FEED / MENEW_UPDATE_PUBKEY (honoured only together).
#
#   cargo build -p menew && bash scripts/test-updates-e2e.sh        # run in WSL / Linux
#   MENEW_BIN=/path/to/menew  GUI=0 bash scripts/test-updates-e2e.sh  # GUI=0 skips the daemon part
#
# What is simulated: the server (local, plain http), the "AppImage" (a file named Menew.AppImage that is a
# tiny script, not a real AppImage), the "new version" (a script that records that it was started),
# `pkexec` (a script on PATH that records its arguments), and `dpkg -S` (the install kind is forced with
# MENEW_UPDATE_INSTALL_KIND=deb). Prints PASS / FAIL per check and exits non-zero if any failed.
set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${MENEW_BIN:-$HOME/target-updater/debug/menew}"
GUI="${GUI:-1}"
[ -x "$BIN" ] || { echo "missing $BIN (cargo build -p menew)"; exit 2; }
T="$(mktemp -d /tmp/menew-upd-XXXXXX)"
FAILS=0
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; chmod -R u+w "$T" 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }
check() { # description, command...
    local d=$1; shift
    if "$@"; then pass "$d"; else fail "$d"; fi
}
sha() { sha256sum "$1" | cut -d' ' -f1; }

NEW=0.1.0-alpha.2
mkdir -p "$T/srv/v$NEW" "$T/app" "$T/keys" "$T/bin"

# ---- keys: the good test key and an attacker key -------------------------------------------------
openssl genpkey -algorithm ed25519 -out "$T/keys/good.pem" 2>/dev/null
openssl genpkey -algorithm ed25519 -out "$T/keys/evil.pem" 2>/dev/null
PUB="$(openssl pkey -in "$T/keys/good.pem" -pubout -outform DER | tail -c 32 | base64 -w0)"

# ---- the "new version": a script that records it was started --------------------------------------
MARKER="$T/started.log"
cat > "$T/Menew-$NEW-x86_64.AppImage" <<EOF
#!/bin/sh
echo "NEW-VERSION-STARTED args=[\$*] pid=\$\$" >> "$MARKER"
EOF
head -c 5000 /dev/urandom > "$T/menew_${NEW}_amd64.deb"
printf '### What works\n\n- **Updates**: the notes of the version\n' > "$T/notes.md"

# ---- the server ----------------------------------------------------------------------------------
PORT=$((20000 + RANDOM % 20000))
BASE="http://127.0.0.1:$PORT"
publish() { # key-file: (re)write update.json signed with that key into srv/
    cp "$T/Menew-$NEW-x86_64.AppImage" "$T/menew_${NEW}_amd64.deb" "$T/srv/v$NEW/"
    bash "$REPO/scripts/make-update-json.sh" "$NEW" "$T/srv/v$NEW" "$1" "$BASE/v$NEW" "$T/notes.md" 2026-11-20 >/dev/null 2>&1
}
listing() { # tags...
    local first=1 out="["
    for tag in "$@"; do
        [ $first = 1 ] || out+=","
        first=0
        out+="{\"tag_name\":\"$tag\",\"draft\":false,\"prerelease\":true,\"assets\":[{\"name\":\"update.json\",\"browser_download_url\":\"$BASE/$tag/update.json\"}]}"
    done
    echo "$out]"
}
mkdir -p "$T/srv/api"
listing "v$NEW" v0.1.0-alpha.1 > "$T/srv/api/releases.json"
listing v0.1.0-alpha.1 > "$T/srv/api/uptodate.json"
publish "$T/keys/good.pem"
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$T/srv" >"$T/server.log" 2>&1 &
PIDS+=($!)
for _ in $(seq 50); do curl -sf "$BASE/api/releases.json" >/dev/null && break; sleep 0.1; done

export MENEW_UPDATE_FEED="$BASE/api/releases.json" MENEW_UPDATE_PUBKEY="$PUB"
export XDG_CONFIG_HOME="$T/cfg"

new_old_appimage() { # fresh "installed" AppImage, prints its path
    local dir=${1:-$T/app}
    mkdir -p "$dir"
    printf '#!/bin/sh\necho OLD\n' > "$dir/Menew.AppImage"
    chmod 755 "$dir/Menew.AppImage"
    echo "$dir/Menew.AppImage"
}
install_cli() { # appimage path, extra env... -> runs `menew check-updates --install`
    local target=$1; shift
    env APPIMAGE="$target" "$@" "$BIN" check-updates --install
}

echo "== 1. the check"
out="$("$BIN" check-updates 2>&1)"; rc=$?
echo "$out" | sed 's/^/    /'
check "finds $NEW (exit 0)" test $rc = 0 -a -n "$(echo "$out" | grep -F 'Update available: 0.1.0-alpha.2')"
check "shows the notes as plain text" grep -qF "• Updates: the notes of the version" <<<"$out"
out="$(MENEW_UPDATE_FEED="$BASE/api/uptodate.json" "$BIN" check-updates 2>&1)"; rc=$?
echo "    $out"
check "up to date when nothing newer is listed (exit 0)" test $rc = 0 -a -n "$(echo "$out" | grep -F 'up to date')"

echo "== 2. AppImage: valid signature replaces the file"
A="$(new_old_appimage)"
before="$(sha "$A")"; want="$(sha "$T/Menew-$NEW-x86_64.AppImage")"
out="$(install_cli "$A" 2>&1)"; rc=$?
echo "$out" | sed 's/^/    /'
after="$(sha "$A")"
echo "    sha256 before: $before"; echo "    sha256 after:  $after"; echo "    sha256 served: $want"
check "exit 0" test $rc = 0
check "the file now is the served one" test "$after" = "$want"
check "the file is executable" test -x "$A"
check "no download leftover next to it" test -z "$(ls -A "$T/app" | grep -v '^Menew.AppImage$')"
rm -f "$MARKER"; "$A"
check "the replaced file runs" grep -q NEW-VERSION-STARTED "$MARKER"

echo "== 3. AppImage: bad signatures and tampered files are refused, the file stays intact"
refuse() { # description, extra setup is done by the caller; runs the install and checks the file
    local desc=$1 A; A="$(new_old_appimage "$T/app-$RANDOM")"
    local b; b="$(sha "$A")"
    out="$(install_cli "$A" 2>&1)"; rc=$?
    echo "    [$desc] exit=$rc: $(echo "$out" | tail -1)"
    echo "    [$desc] sha256 before: $b"; echo "    [$desc] sha256 after:  $(sha "$A")"
    check "$desc: exit 1" test $rc = 1
    check "$desc: file untouched" test "$(sha "$A")" = "$b"
    check "$desc: no leftover" test -z "$(ls -A "$(dirname "$A")" | grep -v '^Menew.AppImage$')"
}
publish "$T/keys/evil.pem"; refuse "signed by another key"
publish "$T/keys/good.pem"
cp "$T/srv/v$NEW/update.json" "$T/good-update.json"
jq '.artifacts.appimage.signature = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="' "$T/good-update.json" > "$T/srv/v$NEW/update.json"
refuse "garbage signature"
jq --arg v "0.1.0-alpha.1" '.version = $v' "$T/good-update.json" > "$T/srv/v$NEW/update.json"
refuse "manifest version differs from the release (feed mismatch)"
cp "$T/good-update.json" "$T/srv/v$NEW/update.json"
cp "$T/srv/v$NEW/Menew-$NEW-x86_64.AppImage" "$T/orig.AppImage"
printf '#!/bin/sh\necho EVIL\n' > "$T/srv/v$NEW/Menew-$NEW-x86_64.AppImage"
refuse "file replaced by another of a different size"
size=$(stat -c %s "$T/orig.AppImage")
head -c "$size" /dev/zero | tr '\0' 'x' > "$T/srv/v$NEW/Menew-$NEW-x86_64.AppImage"
refuse "file replaced by another of the same size (sha256)"
cp "$T/orig.AppImage" "$T/srv/v$NEW/Menew-$NEW-x86_64.AppImage"
jq '.artifacts.appimage.signature = .artifacts.deb.signature' "$T/good-update.json" > "$T/srv/v$NEW/update.json"
refuse "the signature of the .deb used for the AppImage"
cp "$T/good-update.json" "$T/srv/v$NEW/update.json"
A="$(new_old_appimage "$T/app-pk")"; b="$(sha "$A")"
out="$(MENEW_UPDATE_PUBKEY="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" install_cli "$A" 2>&1)"; rc=$?
echo "    [the app holds another public key] exit=$rc: $(echo "$out" | tail -1)"
check "the app holds another public key: refused, file untouched" test $rc = 1 -a "$(sha "$A")" = "$b"

echo "== 4. offline or broken feed: an error exit, never a crash, and nothing touched"
out="$(MENEW_UPDATE_FEED="$BASE/api/missing.json" "$BIN" check-updates 2>&1)"; rc=$?
echo "    404: exit=$rc $out"
check "404 feed: exit 1 with a message" test $rc = 1 -a -n "$out"
out="$(MENEW_UPDATE_FEED="http://127.0.0.1:9/x.json" "$BIN" check-updates 2>&1)"; rc=$?
echo "    closed port: exit=$rc $out"
check "closed port: exit 1 with a message" test $rc = 1 -a -n "$out"
echo 'not json' > "$T/srv/api/garbage.json"
out="$(MENEW_UPDATE_FEED="$BASE/api/garbage.json" "$BIN" check-updates 2>&1)"; rc=$?
echo "    garbage listing: exit=$rc $out"
check "garbage listing: treated as nothing to update" test $rc = 0

echo "== 5. AppImage in a folder that cannot be written"
mkdir -p "$T/ro"; A="$(new_old_appimage "$T/ro")"; chmod 555 "$T/ro"
b="$(sha "$A")"
if touch "$T/ro/x" 2>/dev/null; then
    echo "SKIP  read-only folder check (running as a user that ignores permissions)"; rm -f "$T/ro/x"
else
    out="$(install_cli "$A" 2>&1)"; rc=$?
    echo "    exit=$rc $out"
    check "exit 2 and the 'download page' message" test $rc = 2 -a -n "$(echo "$out" | grep -i 'folder')"
    check "file untouched" test "$(sha "$A")" = "$b"
fi
chmod 755 "$T/ro"

echo "== 6. .deb: pkexec apt-get install -y <verified file> (pkexec simulated)"
cat > "$T/bin/pkexec" <<EOF
#!/bin/sh
# simulated polkit: records the arguments and what the file is, then succeeds or refuses
echo "pkexec \$*" >> "$T/pkexec.log"
f=""; for a in "\$@"; do f="\$a"; done
echo "file-exists=\$(test -f "\$f" && echo yes || echo no) sha256=\$(sha256sum "\$f" | cut -d' ' -f1) mode=\$(stat -c %a "\$f")" >> "$T/pkexec.log"
[ -n "\${FAKE_PKEXEC_EXIT:-}" ] && exit "\$FAKE_PKEXEC_EXIT"
exit 0
EOF
chmod +x "$T/bin/pkexec"
out="$(env PATH="$T/bin:$PATH" MENEW_UPDATE_INSTALL_KIND=deb "$BIN" check-updates --install 2>&1)"; rc=$?
echo "$out" | sed 's/^/    /'; sed 's/^/    pkexec.log: /' "$T/pkexec.log"
debsha="$(sha "$T/menew_${NEW}_amd64.deb")"
check "exit 0" test $rc = 0
check "pkexec was called as: apt-get install -y /tmp/menew-update-*/menew_$NEW""_amd64.deb" grep -Eq "^pkexec apt-get install -y /tmp/menew-update-[0-9]+/menew_${NEW}_amd64.deb\$" "$T/pkexec.log"
check "the file given to apt was the verified one (sha256, readable by apt)" grep -q "file-exists=yes sha256=$debsha mode=644" "$T/pkexec.log"
check "the temporary folder is gone" test -z "$(ls -d /tmp/menew-update-* 2>/dev/null)"
: > "$T/pkexec.log"
out="$(env PATH="$T/bin:$PATH" MENEW_UPDATE_INSTALL_KIND=deb FAKE_PKEXEC_EXIT=126 "$BIN" check-updates --install 2>&1)"; rc=$?
echo "    refused password: exit=$rc $(echo "$out" | tail -1)"
check "password refused: exit 2 and the 'download page' message" test $rc = 2
check "password refused: temporary folder removed" test -z "$(ls -d /tmp/menew-update-* 2>/dev/null)"
: > "$T/pkexec.log"
publish "$T/keys/evil.pem"
out="$(env PATH="$T/bin:$PATH" MENEW_UPDATE_INSTALL_KIND=deb "$BIN" check-updates --install 2>&1)"; rc=$?
echo "    bad signature: exit=$rc $(echo "$out" | tail -1)"
check "bad signature: pkexec is never called" test ! -s "$T/pkexec.log"
check "bad signature: exit 1" test $rc = 1
publish "$T/keys/good.pem"
mkdir -p "$T/bin2"; ln -s "$(command -v curl)" "$T/bin2/curl"
out="$(env PATH="$T/bin2" MENEW_UPDATE_INSTALL_KIND=deb "$BIN" check-updates --install 2>&1)"; rc=$?
echo "    no pkexec on PATH: exit=$rc $(echo "$out" | tail -1)"
check "no pkexec: exit 2 and the 'download page' message, no leftover" test $rc = 2 -a -z "$(ls -d /tmp/menew-update-* 2>/dev/null)"

echo "== 7. built from source (neither AppImage nor package): only tells"
out="$(env -u APPIMAGE MENEW_UPDATE_INSTALL_KIND=manual "$BIN" check-updates --install 2>&1)"; rc=$?
echo "    exit=$rc $(echo "$out" | tail -2 | tr '\n' ' ')"
check "exit 2 with the 'update it yourself' message" test $rc = 2 -a -n "$(echo "$out" | grep -i 'yourself')"

[ "$GUI" = 1 ] || { echo; echo "failures: $FAILS"; exit $((FAILS > 0)); }

# ---- the daemon: automatic check, the dot, install and restart ------------------------------------
echo "== 8. the daemon (headless dbus session, throwaway HOME)"
daemon_env() {
    export HOME="$T/home"; mkdir -p "$HOME"
    export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" XDG_CACHE_HOME="$HOME/.cache"
    export MENEW_DB_PATH="$T/clip.db" MENEW_TEST=1 MENEW_UPDATE_STARTUP_DELAY=2 GSETTINGS_BACKEND=keyfile
    export RUST_LOG=menew=info,menew_tray=info
}
inner() {
    daemon_env
    local A; A="$(new_old_appimage "$T/daemon-app")"
    rm -f "$MARKER"
    python3 "$REPO/scripts/desktop-test/watcher.py" >"$T/watcher.log" 2>&1 &
    sleep 1
    echo "    APPIMAGE before: $(sha "$A")"
    env APPIMAGE="$A" "$BIN" >"$T/daemon.log" 2>&1 &
    local pid=$!
    for _ in $(seq 100); do grep -q "Starting Menew Linux" "$T/daemon.log" && break; sleep 0.1; done
    sleep 6   # the automatic check runs 2 seconds after the start
    echo "    --- daemon log (updates, tray) ---"
    grep -E "Updates:|Tray status|Notification" "$T/daemon.log" | sed 's/^/    /' | head -20
    check "daemon: the automatic check found the version" grep -q "Updates: version $NEW found (manual=false)" "$T/daemon.log"
    check "daemon: no notification and no window for an automatic check" test -z "$(grep -E 'Notification sent' "$T/daemon.log")"
    sni=""
    for _ in $(seq 25); do
        sni="$(gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
            --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems 2>/dev/null \
            | grep -o 'org\.kde\.StatusNotifierItem-[0-9]*-[0-9]*' | head -1)"
        [ -n "$sni" ] && break
        sleep 0.5
    done
    if [ -n "$sni" ]; then
        prop() { gdbus call --session --dest "$sni" --object-path /StatusNotifierItem --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierItem "$1" 2>&1; }
        st="$(prop Status)"; ov="$(prop OverlayIconName)"
        echo "    tray Status: $st ; OverlayIconName: $ov"
        check "daemon: the tray icon has the reminder dot (NeedsAttention + overlay icon)" test "$st" = "(<'NeedsAttention'>,)" -a "$ov" = "(<'software-update-available-symbolic'>,)"
    else
        echo "SKIP  tray dot: no StatusNotifierItem registered (is gdbus installed?)"
    fi
    # "Check for Updates..." with the update already found opens the window (no GTK or libadwaita warnings)
    env MENEW_TEST=1 "$BIN" __updates-check >/dev/null 2>&1
    sleep 2
    check "daemon: the update window opened without GTK/Adwaita warnings" test -z "$(grep -aE 'CRITICAL|Gtk-WARNING|Adwaita-WARNING' "$T/daemon.log")"
    # bad signature through the daemon: refused, file intact, daemon stays up
    publish "$T/keys/evil.pem"
    b="$(sha "$A")"
    env MENEW_TEST=1 "$BIN" __updates-install >/dev/null 2>&1
    sleep 4
    echo "    APPIMAGE after the bad-signature install: $(sha "$A")"
    check "daemon: bad signature refused, file intact" test "$(sha "$A")" = "$b"
    check "daemon: it logged the verification failure and told the user" grep -q "Updates: verification failed" "$T/daemon.log"
    check "daemon: still running after a refusal" kill -0 "$pid"
    publish "$T/keys/good.pem"
    # the real install and restart
    want="$(sha "$T/Menew-$NEW-x86_64.AppImage")"
    env MENEW_TEST=1 "$BIN" __updates-install >/dev/null 2>&1
    for _ in $(seq 60); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
    sleep 2
    echo "    APPIMAGE after:  $(sha "$A")   (served: $want)"
    echo "    marker: $(cat "$MARKER" 2>/dev/null)"
    check "daemon: the AppImage was replaced by the served one" test "$(sha "$A")" = "$want"
    check "daemon: the old process quit" test ! -d "/proc/$pid"
    check "daemon: the new version was started, with no arguments" grep -q 'NEW-VERSION-STARTED args=\[\]' "$MARKER"
    pkill -f "menew-restart" 2>/dev/null
    kill "$pid" 2>/dev/null
}
export -f inner daemon_env new_old_appimage publish sha check pass fail
export T BIN REPO BASE NEW MARKER PUB FAILS
# the failures counted in the inner shell are carried through a file
dbus-run-session -- bash -c 'FAILS=0; inner; echo $FAILS > "$T/inner.fails"'
inner_fails="$(cat "$T/inner.fails" 2>/dev/null || echo 99)"
FAILS=$((FAILS + inner_fails))
echo
echo "failures: $FAILS"
exit $((FAILS > 0))
