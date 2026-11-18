//! Automatic updates (the Linux side of the Mac's Sparkle, see `docs/updates-spec.md`).
//!
//! What lives here is everything that does not need a window: semantic versions, choosing the release
//! out of the GitHub API listing, the `update.json` of a release, the signature check and the install
//! of an AppImage or a `.deb`. The network is `curl` (like `currency.rs`: no HTTP or TLS crate).
//! Nothing leaves the machine except the two GET requests of a check (the listing and `update.json`) and the
//! download of the artifact, all without any identifier: the only header is `User-Agent: Menew/<version>`.

use crate::sha256::sha256;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Releases of the repository, newest first. `releases/latest` is not used because it ignores pre-releases.
pub const RELEASES_API: &str = "https://api.github.com/repos/Quost/menew-linux/releases?per_page=10";
/// Release page of a tag (shown when the app cannot install by itself).
pub const RELEASE_PAGE: &str = "https://github.com/Quost/menew-linux/releases/tag/";
/// Name of the file each release carries with the notes and the signed artifacts.
pub const MANIFEST_NAME: &str = "update.json";
/// Automatic check every 24 hours (`SUScheduledCheckInterval` of the Mac: 86400).
pub const AUTO_CHECK_INTERVAL_SECS: u32 = 86_400;
/// First automatic check, a few minutes after the daemon starts (so not while the session is still loading).
pub const STARTUP_CHECK_DELAY_SECS: u32 = 300;
/// Nothing bigger than this is downloaded.
pub const MAX_ARTIFACT_BYTES: u64 = 600 * 1024 * 1024;

/// Ed25519 public key of the releases, base64 of the 32 bytes. The private key is the
/// `MENEW_UPDATE_SIGNING_KEY` secret of the repository and is not the Mac's Sparkle key.
/// The `Release` workflow refuses to publish when the secret does not match this constant.
/// The private half is kept by the maintainer, outside the repository; losing it means installed copies
/// can no longer update (a new key would need a manual reinstall).
pub const UPDATE_PUBLIC_KEY: &str = "0cRAUeb+ObjVOEp7be6u/mAckWruPhAuj+EZdo5slaM=";

// ---------------------------------------------------------------------------------------------
// Versions
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ident {
    Num(u64),
    Text(String),
}

/// A semantic version (`1.2.3`, `1.2.3-alpha.1`; a leading `v` and `+build` are accepted and ignored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pre: Vec<Ident>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let s = s.split('+').next()?;
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut nums = core.split('.');
        let major = nums.next()?.parse().ok()?;
        let minor = nums.next()?.parse().ok()?;
        let patch = nums.next()?.parse().ok()?;
        if nums.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => vec![],
            Some(p) => {
                let mut out = vec![];
                for part in p.split('.') {
                    if part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                        return None;
                    }
                    out.push(match part.parse::<u64>() {
                        Ok(n) => Ident::Num(n),
                        Err(_) => Ident::Text(part.to_string()),
                    });
                }
                out
            }
        };
        Some(Self { major, minor, patch, pre })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// The version of this build.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("the crate version is semver")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (i, id) in self.pre.iter().enumerate() {
            f.write_str(if i == 0 { "-" } else { "." })?;
            match id {
                Ident::Num(n) => write!(f, "{n}")?,
                Ident::Text(t) => f.write_str(t)?,
            }
        }
        Ok(())
    }
}

impl Ord for Version {
    /// SemVer 2.0 precedence: a pre-release is lower than its release; numeric identifiers are lower
    /// than text ones; a longer list of identifiers is higher when the shared ones are equal.
    fn cmp(&self, other: &Self) -> Ordering {
        let core = (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));
        if core != Ordering::Equal {
            return core;
        }
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            _ => {}
        }
        for (a, b) in self.pre.iter().zip(&other.pre) {
            let o = match (a, b) {
                (Ident::Num(x), Ident::Num(y)) => x.cmp(y),
                (Ident::Num(_), Ident::Text(_)) => Ordering::Less,
                (Ident::Text(_), Ident::Num(_)) => Ordering::Greater,
                (Ident::Text(x), Ident::Text(y)) => x.cmp(y),
            };
            if o != Ordering::Equal {
                return o;
            }
        }
        self.pre.len().cmp(&other.pre.len())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ---------------------------------------------------------------------------------------------
// Choosing the release
// ---------------------------------------------------------------------------------------------

/// A release of the GitHub listing that carries an `update.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub manifest_url: String,
    /// The `prerelease` flag of GitHub (the version itself can say it too, with `-alpha`).
    pub prerelease: bool,
}

/// Read the listing of `GET /repos/.../releases`: drafts, tags that are not versions and releases without an
/// `update.json` asset are dropped (a pre-release is kept: whether it counts depends on the running version).
pub fn parse_releases(body: &str) -> Vec<ReleaseInfo> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(body) else {
        return vec![];
    };
    list.iter()
        .filter_map(|r| {
            if r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) {
                return None;
            }
            let tag = r.get("tag_name")?.as_str()?;
            let version = Version::parse(tag)?;
            let url = r.get("assets")?.as_array()?.iter().find_map(|a| {
                (a.get("name")?.as_str()? == MANIFEST_NAME).then(|| a.get("browser_download_url")?.as_str().map(str::to_string))?
            })?;
            let prerelease = r.get("prerelease").and_then(|d| d.as_bool()).unwrap_or(false);
            Some(ReleaseInfo { tag: tag.to_string(), version, manifest_url: url, prerelease })
        })
        .collect()
}

/// The highest version greater than `current`. A stable `current` ignores pre-releases; a pre-release
/// `current` follows them too (the alpha testers follow the alphas).
pub fn pick_release<'a>(releases: &'a [ReleaseInfo], current: &Version) -> Option<&'a ReleaseInfo> {
    releases
        .iter()
        .filter(|r| r.version > *current)
        .filter(|r| current.is_prerelease() || !(r.version.is_prerelease() || r.prerelease))
        .max_by(|a, b| a.version.cmp(&b.version))
}

// ---------------------------------------------------------------------------------------------
// update.json
// ---------------------------------------------------------------------------------------------

/// One file of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub url: String,
    pub size: u64,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: String,
    /// Base64 Ed25519 signature of [`signed_message`].
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub notes: String,
    /// `appimage` and `deb`.
    pub artifacts: std::collections::BTreeMap<String, Artifact>,
}

impl Manifest {
    pub fn parse(body: &str) -> Result<Self, UpdateError> {
        let m: Manifest = serde_json::from_str(body).map_err(|_| UpdateError::BadFeed)?;
        if Version::parse(&m.version).is_none() {
            return Err(UpdateError::BadFeed);
        }
        Ok(m)
    }

    pub fn version(&self) -> Version {
        Version::parse(&self.version).expect("checked by parse")
    }
}

/// The bytes the release key signs for one artifact. Signing this instead of the file binds the version
/// and the kind to the signature (an old, validly signed file cannot be offered as a new version, an AppImage
/// cannot be swapped for the `.deb`); the file itself is bound through its SHA-256 and size.
pub fn signed_message(version: &str, kind: &str, size: u64, sha256_hex: &str) -> String {
    format!("menew-update\nversion={version}\nartifact={kind}\nsize={size}\nsha256={sha256_hex}\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// The network or the server did not answer.
    Network,
    /// The answer is not what the feed should say.
    BadFeed,
    /// The file is not the size the feed says.
    SizeMismatch,
    HashMismatch,
    /// The signature does not match the embedded key (or the key itself is unusable).
    BadSignature,
    /// The release has no file for this way of installing.
    NoArtifact,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            UpdateError::Network => "network error",
            UpdateError::BadFeed => "malformed update feed",
            UpdateError::SizeMismatch => "size does not match the feed",
            UpdateError::HashMismatch => "sha256 does not match the feed",
            UpdateError::BadSignature => "invalid signature",
            UpdateError::NoArtifact => "the release has no file for this install type",
        })
    }
}

impl std::error::Error for UpdateError {}

// ---------------------------------------------------------------------------------------------
// base64, hex and the signature
// ---------------------------------------------------------------------------------------------

/// Standard base64 (padding optional); no crate for twenty lines.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.trim().trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Check `signature_b64` over `message` with the base64 public key. Any malformed input is a refusal.
pub fn verify_signature(public_key_b64: &str, message: &[u8], signature_b64: &str) -> Result<(), UpdateError> {
    let key = base64_decode(public_key_b64)
        .and_then(|k| ed25519_compact::PublicKey::from_slice(&k).ok())
        .ok_or(UpdateError::BadSignature)?;
    let sig = base64_decode(signature_b64)
        .and_then(|s| ed25519_compact::Signature::from_slice(&s).ok())
        .ok_or(UpdateError::BadSignature)?;
    key.verify(message, &sig).map_err(|_| UpdateError::BadSignature)
}

/// Verify a downloaded file against its manifest entry: size, SHA-256 and signature, in that order.
/// Nothing is installed unless this returns `Ok`.
pub fn verify_artifact(file: &Path, version: &str, kind: &str, art: &Artifact, public_key_b64: &str) -> Result<(), UpdateError> {
    let len = std::fs::metadata(file).map_err(|_| UpdateError::Network)?.len();
    if len != art.size {
        return Err(UpdateError::SizeMismatch);
    }
    let data = std::fs::read(file).map_err(|_| UpdateError::Network)?;
    if hex(&sha256(&data)) != art.sha256.to_ascii_lowercase() {
        return Err(UpdateError::HashMismatch);
    }
    verify_signature(public_key_b64, signed_message(version, kind, art.size, &art.sha256.to_ascii_lowercase()).as_bytes(), &art.signature)
}

// ---------------------------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------------------------

/// What a check found: a version newer than the running one, with everything needed to install it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub tag: String,
    pub version: Version,
    pub manifest: Manifest,
}

impl Found {
    pub fn page_url(&self) -> String {
        format!("{RELEASE_PAGE}{}", self.tag)
    }
}

/// Where the feed is and which key is trusted. Only tests change it (`MENEW_UPDATE_FEED`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    pub api_url: String,
    pub public_key: String,
    /// A feed from the environment may be plain `http` (a local test server); the real one never is.
    pub overridden: bool,
}

impl Feed {
    pub fn from_env() -> Self {
        Self::from_values(std::env::var("MENEW_UPDATE_FEED").ok(), std::env::var("MENEW_UPDATE_PUBKEY").ok())
    }

    /// The key can be replaced only together with the feed, so setting `MENEW_UPDATE_PUBKEY` alone does nothing.
    pub fn from_values(feed: Option<String>, pubkey: Option<String>) -> Self {
        match feed.filter(|f| !f.is_empty()) {
            Some(api_url) => Self {
                api_url,
                public_key: pubkey.filter(|k| !k.is_empty()).unwrap_or_else(|| UPDATE_PUBLIC_KEY.to_string()),
                overridden: true,
            },
            None => Self { api_url: RELEASES_API.to_string(), public_key: UPDATE_PUBLIC_KEY.to_string(), overridden: false },
        }
    }
}

/// Is an update check allowed in this process? Never with `--snapshot`; under `MENEW_TEST=1` only with an
/// explicit feed (so the desktop tests never reach the network).
pub fn checks_allowed(snapshot: bool, test_mode: bool, feed_overridden: bool) -> bool {
    !snapshot && (!test_mode || feed_overridden)
}

/// Run a check. `get` returns the body of a URL (the real one is [`curl_get`], tests pass a stub).
/// `Ok(None)` means up to date.
pub fn check_with(get: &dyn Fn(&str) -> Result<String, UpdateError>, feed: &Feed, current: &Version) -> Result<Option<Found>, UpdateError> {
    let releases = parse_releases(&get(&feed.api_url)?);
    let Some(rel) = pick_release(&releases, current) else {
        return Ok(None);
    };
    let manifest = Manifest::parse(&get(&rel.manifest_url)?)?;
    // the manifest must be the one of the release it was found in
    if manifest.version() != rel.version {
        return Err(UpdateError::BadFeed);
    }
    Ok(Some(Found { tag: rel.tag.clone(), version: rel.version.clone(), manifest }))
}

pub fn check(feed: &Feed, current: &Version) -> Result<Option<Found>, UpdateError> {
    check_with(&|u| curl_get(u, feed.overridden), feed, current)
}

fn curl_base(insecure_ok: bool) -> std::process::Command {
    let mut c = std::process::Command::new("curl");
    c.args(["--silent", "--show-error", "--fail", "--location", "--max-redirs", "5"]);
    let proto = if insecure_ok { "=http,https" } else { "=https" };
    c.args(["--proto", proto, "--proto-redir", proto]);
    c.args(["--user-agent", &format!("Menew/{}", env!("CARGO_PKG_VERSION"))]);
    c
}

/// GET with `curl`: https only (`http` just for the test feed), 20 seconds, at most 5 redirects.
pub fn curl_get(url: &str, insecure_ok: bool) -> Result<String, UpdateError> {
    let out = curl_base(insecure_ok)
        .args(["--max-time", "20", "--header", "Accept: application/vnd.github+json", "--", url])
        .output()
        .map_err(|_| UpdateError::Network)?;
    if !out.status.success() {
        return Err(UpdateError::Network);
    }
    String::from_utf8(out.stdout).map_err(|_| UpdateError::BadFeed)
}

/// Download `url` to `dest` (a partial file is removed on failure).
pub fn download(url: &str, dest: &Path, insecure_ok: bool) -> Result<(), UpdateError> {
    let ok = curl_base(insecure_ok)
        .args(["--max-time", "900", "--max-filesize", &MAX_ARTIFACT_BYTES.to_string(), "--output"])
        .arg(dest)
        .args(["--", url])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(dest);
        return Err(UpdateError::Network);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// State that survives restarts: the skipped version
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    /// "Skip this version": the automatic check stays quiet about it (a manual check still shows it).
    pub skipped: Option<String>,
}

impl UpdateState {
    pub fn path_in(config_dir: &Path) -> PathBuf {
        config_dir.join("menew").join("update-state.json")
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path_in(&crate::config::config_dir()))
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).map_err(std::io::Error::other)? + "\n")
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&Self::path_in(&crate::config::config_dir()))
    }

    pub fn is_skipped(&self, version: &Version) -> bool {
        self.skipped.as_deref().and_then(Version::parse).map(|s| s == *version).unwrap_or(false)
    }
}

/// Should the automatic check tell about this version? (Not when the user skipped exactly it.)
pub fn announce_automatically(state: &UpdateState, found: &Version) -> bool {
    !state.is_skipped(found)
}

// ---------------------------------------------------------------------------------------------
// Release notes as plain text (the Mac turns the HTML of the appcast into text)
// ---------------------------------------------------------------------------------------------

/// The CHANGELOG Markdown of a version as plain text: bullets become `•`, headings lose their `#`,
/// bold, code and link markup is removed (`[text](url)` keeps `text`).
pub fn plain_notes(md: &str) -> String {
    let mut out: Vec<String> = vec![];
    for line in md.lines() {
        let t = line.trim_end();
        let (indent, body) = {
            let b = t.trim_start();
            (&t[..t.len() - b.len()], b)
        };
        let body = if let Some(rest) = body.strip_prefix("- ").or_else(|| body.strip_prefix("* ")) {
            format!("\u{2022} {rest}")
        } else {
            body.trim_start_matches('#').trim_start().to_string()
        };
        let mut s = String::new();
        let mut rest = body.as_str();
        while let Some(c) = rest.chars().next() {
            if let Some(r) = rest.strip_prefix("**") {
                rest = r;
            } else if c == '`' {
                rest = &rest[1..];
            } else if c == '[' {
                match rest.find("](").and_then(|m| rest[m..].find(')').map(|e| (m, m + e))) {
                    Some((m, e)) => {
                        s.push_str(&rest[1..m]);
                        rest = &rest[e + 1..];
                    }
                    None => {
                        s.push(c);
                        rest = &rest[1..];
                    }
                }
            } else {
                s.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        out.push(format!("{indent}{s}"));
    }
    // no more than one blank line in a row, none at the ends
    let mut cleaned: Vec<String> = vec![];
    for l in out {
        if l.trim().is_empty() && cleaned.last().map(|p: &String| p.trim().is_empty()).unwrap_or(true) {
            continue;
        }
        cleaned.push(l);
    }
    while cleaned.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        cleaned.pop();
    }
    cleaned.join("\n")
}

// ---------------------------------------------------------------------------------------------
// Installing
// ---------------------------------------------------------------------------------------------

/// How this copy of Menew was installed, which decides how it updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    /// `$APPIMAGE`: the file is replaced.
    AppImage(PathBuf),
    /// `/usr/bin/menew` owned by dpkg: the `.deb` is installed with `pkexec apt-get`.
    Deb,
    /// Built from source or anywhere else: only told about, with the link of the release.
    Manual,
}

/// Pure decision. `appimage_exists`: the `$APPIMAGE` path is a file; `dpkg_owns_exe`: the running
/// executable is `/usr/bin/menew` and `dpkg -S` knows it.
pub fn decide_install(appimage_env: Option<&str>, appimage_exists: bool, dpkg_owns_exe: bool) -> InstallMethod {
    match appimage_env.filter(|a| !a.is_empty()) {
        Some(p) if appimage_exists => InstallMethod::AppImage(PathBuf::from(p)),
        _ if dpkg_owns_exe => InstallMethod::Deb,
        _ => InstallMethod::Manual,
    }
}

/// The artifact kind of the manifest for a way of installing.
pub fn artifact_kind(method: &InstallMethod) -> Option<&'static str> {
    match method {
        InstallMethod::AppImage(_) => Some("appimage"),
        InstallMethod::Deb => Some("deb"),
        InstallMethod::Manual => None,
    }
}

pub fn detect_install(feed: &Feed) -> InstallMethod {
    // test hook, honoured only together with the test feed: `MENEW_UPDATE_INSTALL_KIND=deb|manual|appimage`
    if feed.overridden {
        match std::env::var("MENEW_UPDATE_INSTALL_KIND").as_deref() {
            Ok("deb") => return InstallMethod::Deb,
            Ok("manual") => return InstallMethod::Manual,
            _ => {}
        }
    }
    let appimage = std::env::var("APPIMAGE").ok();
    let exists = appimage.as_deref().map(|p| Path::new(p).is_file()).unwrap_or(false);
    let owns = std::env::current_exe().map(|e| e == Path::new("/usr/bin/menew")).unwrap_or(false)
        && std::process::Command::new("dpkg")
            .args(["-S", "/usr/bin/menew"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    decide_install(appimage.as_deref(), exists, owns)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    Verify(UpdateError),
    /// The folder of the AppImage cannot be written: the page of the release is offered instead.
    NotWritable,
    /// `pkexec` is missing or the password was refused.
    Declined,
    /// Anything else (the text goes to the log).
    Failed(String),
    /// Built from source: nothing to install.
    Manual,
}

/// Replace `target` with the verified `new_file` (same folder, so the rename is atomic): the permissions of
/// the old file are kept and the executable bits are set.
pub fn replace_appimage(new_file: &Path, target: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(target).map(|m| m.permissions().mode() & 0o7777).unwrap_or(0o755) | 0o755;
    std::fs::set_permissions(new_file, std::fs::Permissions::from_mode(mode)).map_err(|e| InstallError::Failed(e.to_string()))?;
    std::fs::rename(new_file, target).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            InstallError::NotWritable
        } else {
            InstallError::Failed(e.to_string())
        }
    })
}

/// Where the download of an AppImage goes: next to the target, hidden (the rename needs the same file system).
pub fn appimage_download_path(target: &Path) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Menew.AppImage".into());
    target.with_file_name(format!(".{name}.update"))
}

/// Install the `.deb` with polkit: `pkexec apt-get install -y <file>`. `pkexec` is looked up in `PATH`.
pub fn install_deb(file: &Path) -> Result<(), InstallError> {
    let status = std::process::Command::new("pkexec")
        .args(["apt-get", "install", "-y"])
        .arg(file)
        .status()
        .map_err(|_| InstallError::Declined)?;
    // 126 / 127: polkit refused or the password prompt was dismissed
    match status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err(InstallError::Declined),
        _ => Err(InstallError::Failed(format!("apt-get exited with {status}"))),
    }
}

/// Download, verify and install `found` the way `method` says. `on_stage` is told "download" before the
/// download (the window shows it). The `.deb` file is kept in a private temporary folder.
pub fn install(found: &Found, method: &InstallMethod, feed: &Feed, on_stage: &dyn Fn(&str)) -> Result<(), InstallError> {
    let kind = artifact_kind(method).ok_or(InstallError::Manual)?;
    let art = found.manifest.artifacts.get(kind).ok_or(InstallError::Verify(UpdateError::NoArtifact))?;
    let version = found.version.to_string();
    on_stage("download");
    match method {
        InstallMethod::AppImage(target) => {
            let tmp = appimage_download_path(target);
            // creating the file first tells "folder not writable" apart from a network failure
            std::fs::File::create(&tmp).map_err(|_| InstallError::NotWritable)?;
            download(&art.url, &tmp, feed.overridden).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                InstallError::Verify(e)
            })?;
            if let Err(e) = verify_artifact(&tmp, &version, kind, art, &feed.public_key) {
                let _ = std::fs::remove_file(&tmp);
                return Err(InstallError::Verify(e));
            }
            on_stage("install");
            let r = replace_appimage(&tmp, target);
            if r.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            r
        }
        InstallMethod::Deb => {
            use std::os::unix::fs::PermissionsExt;
            // apt runs as root and as `_apt`: a folder everyone can read, private to the user for writing
            let dir = std::env::temp_dir().join(format!("menew-update-{}", std::process::id()));
            std::fs::create_dir_all(&dir).map_err(|e| InstallError::Failed(e.to_string()))?;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
            let file = dir.join(format!("menew_{version}_amd64.deb"));
            let result = (|| {
                download(&art.url, &file, feed.overridden).map_err(InstallError::Verify)?;
                verify_artifact(&file, &version, kind, art, &feed.public_key).map_err(InstallError::Verify)?;
                let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
                on_stage("install");
                install_deb(&file)
            })();
            let _ = std::fs::remove_dir_all(&dir);
            result
        }
        InstallMethod::Manual => Err(InstallError::Manual),
    }
}

/// The command that starts the new Menew once this process is gone (the single-instance application would
/// otherwise hand the start over to the old one): waits for `pid` to exit, then runs `exe` with no arguments,
/// which is a first launch, so the palette stays hidden.
pub fn restart_command(exe: &Path, pid: u32) -> std::process::Command {
    use std::os::unix::process::CommandExt;
    let mut c = std::process::Command::new("sh");
    c.arg("-c")
        .arg("while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; shift; exec \"$@\"")
        .arg("menew-restart")
        .arg(pid.to_string())
        .arg(exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0);
    c
}

/// The file to start after an update: the AppImage, or the installed binary.
pub fn restart_target(method: &InstallMethod) -> Option<PathBuf> {
    match method {
        InstallMethod::AppImage(p) => Some(p.clone()),
        InstallMethod::Deb => Some(PathBuf::from("/usr/bin/menew")),
        InstallMethod::Manual => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn semver_order() {
        assert!(v("0.1.0-alpha.1") < v("0.1.0-alpha.2"));
        assert!(v("0.1.0-alpha.2") < v("0.1.0-beta.1"));
        assert!(v("0.1.0-beta.1") < v("0.1.0"));
        assert!(v("0.1.0-alpha.2") < v("0.1.0-alpha.10"));
        assert_eq!(v("v1.2.3+build5"), v("1.2.3"));
        assert_eq!(v("1.2.3-rc.1").to_string(), "1.2.3-rc.1");
        assert!(Version::parse("1.2").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(Version::parse("x.1.0").is_none());
        assert!(Version::parse("1.0.0-").is_none());
    }

    #[test]
    fn base64_round_trip_of_known_values() {
        assert_eq!(base64_decode("aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(base64_decode("aGVsbG8"), Some(b"hello".to_vec()));
        assert_eq!(base64_decode("AAAA"), Some(vec![0, 0, 0]));
        assert_eq!(base64_decode("a$b"), None);
        assert_eq!(base64_decode(UPDATE_PUBLIC_KEY).map(|k| k.len()), Some(32));
    }

    #[test]
    fn the_release_key_refuses_a_bad_signature() {
        assert_eq!(verify_signature(UPDATE_PUBLIC_KEY, b"x", "AAAA"), Err(UpdateError::BadSignature));
    }

    #[test]
    fn plain_notes_strips_markdown() {
        let md = "### What works\n\n- **Palette**: the `apps` list, see [docs](https://x.y/z)\n  - nested\n\n\n\nEnd\n";
        assert_eq!(plain_notes(md), "What works\n\n\u{2022} Palette: the apps list, see docs\n  \u{2022} nested\n\nEnd");
    }

    #[test]
    fn feed_key_override_needs_the_feed() {
        assert!(!Feed::from_values(None, Some("AAAA".into())).overridden);
        assert_eq!(Feed::from_values(None, Some("AAAA".into())).public_key, UPDATE_PUBLIC_KEY);
        let f = Feed::from_values(Some("http://127.0.0.1:1/r".into()), Some("AAAA".into()));
        assert!(f.overridden && f.public_key == "AAAA");
    }

    #[test]
    fn restart_command_waits_for_the_pid() {
        let c = restart_command(Path::new("/x/Menew.AppImage"), 42);
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args[2], "menew-restart");
        assert_eq!(&args[3..], ["42", "/x/Menew.AppImage"]);
    }
}
