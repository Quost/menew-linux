#!/usr/bin/env bash
# Writes dist/update.json (the feed of the in-app updater) for one release and signs every artifact with the
# release key. Used by the Release workflow; also runnable locally with a test key.
#
#   scripts/make-update-json.sh VERSION DIST_DIR PRIVATE_KEY_PEM BASE_URL NOTES_FILE [DATE]
#
# VERSION       0.1.0-alpha.2 (no leading v)
# DIST_DIR      folder with menew_<VERSION>_amd64.deb and Menew-<VERSION>-x86_64.AppImage; update.json goes there
# PRIVATE_KEY   Ed25519 private key in PEM (PKCS#8), the MENEW_UPDATE_SIGNING_KEY secret
# BASE_URL      where the artifacts will be downloaded from, e.g.
#               https://github.com/Quost/menew-linux/releases/download/v0.1.0-alpha.2
# NOTES_FILE    the CHANGELOG section of the version (Markdown)
#
# What is signed is NOT the file but `signed_message` of crates/menew-core/src/updates.rs (see
# docs/updates-spec.md): the version, the kind, the size and the SHA-256 of the file. The app checks the size and
# the SHA-256 of what it downloads and then this signature against the public key compiled into it.
# Needs openssl 3 (Ed25519 with `pkeyutl -rawin`), jq, sha256sum and base64. Nothing is sent anywhere.
set -euo pipefail

[ $# -ge 5 ] || { sed -n '2,19p' "$0" >&2; exit 2; }
VERSION=$1 DIST=$2 KEY=$3 BASE_URL=${4%/} NOTES=$5 DATE=${6:-$(date -u +%Y-%m-%d)}

[ -s "$KEY" ] || { echo "make-update-json: the signing key is missing or empty ($KEY)" >&2; exit 1; }
[ -s "$NOTES" ] || { echo "make-update-json: the release notes are missing ($NOTES)" >&2; exit 1; }
openssl pkey -in "$KEY" -noout 2>/dev/null || { echo "make-update-json: $KEY is not a valid private key" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
openssl pkey -in "$KEY" -pubout -out "$WORK/pub.pem"

artifact() { # kind file -> prints the JSON object of the artifact
    local kind=$1 file=$2
    [ -f "$DIST/$file" ] || { echo "make-update-json: $DIST/$file is missing" >&2; exit 1; }
    local size sha sig
    size=$(stat -c %s "$DIST/$file")
    sha=$(sha256sum "$DIST/$file" | cut -d' ' -f1)
    printf 'menew-update\nversion=%s\nartifact=%s\nsize=%s\nsha256=%s\n' "$VERSION" "$kind" "$size" "$sha" > "$WORK/$kind.msg"
    openssl pkeyutl -sign -inkey "$KEY" -rawin -in "$WORK/$kind.msg" -out "$WORK/$kind.sig"
    # the signature must verify with the public half before it is published
    openssl pkeyutl -verify -pubin -inkey "$WORK/pub.pem" -rawin -in "$WORK/$kind.msg" -sigfile "$WORK/$kind.sig" >&2
    sig=$(base64 -w0 < "$WORK/$kind.sig")
    jq -n --arg url "$BASE_URL/$file" --argjson size "$size" --arg sha "$sha" --arg sig "$sig" \
        '{url: $url, size: $size, sha256: $sha, signature: $sig}'
}

APPIMAGE=$(artifact appimage "Menew-${VERSION}-x86_64.AppImage")
DEB=$(artifact deb "menew_${VERSION}_amd64.deb")

jq -n --arg version "$VERSION" --arg date "$DATE" --rawfile notes "$NOTES" \
    --argjson appimage "$APPIMAGE" --argjson deb "$DEB" \
    '{version: $version, date: $date, notes: $notes, artifacts: {appimage: $appimage, deb: $deb}}' > "$DIST/update.json"
echo "wrote $DIST/update.json"
