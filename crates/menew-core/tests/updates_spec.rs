//! Runs every vector of `docs/updates-spec.md` through the real update code.
//! The expected values in that file were derived by hand from SemVer 2.0 and the Mac behaviour
//! (`../menew/Sources/Menew/Updater.swift`).
#![cfg(unix)]

use ed25519_compact::{KeyPair, Seed};
use menew_core::sha256::sha256;
use menew_core::updates::*;

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim_start().strip_prefix('|').unwrap().trim_end().strip_suffix('|').unwrap();
    inner
        .split('|')
        .map(|c| {
            let c = c.strip_prefix(' ').unwrap_or(c);
            c.strip_suffix(' ').unwrap_or(c).to_string()
        })
        .collect()
}

/// The rows whose first cell is `<prefix><digits>`.
fn rows(prefix: char) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/updates-spec.md")).unwrap();
    let rows: Vec<Vec<String>> = text
        .lines()
        .filter(|l| {
            l.strip_prefix("| ")
                .map(|r| {
                    let id: String = r.chars().take_while(|c| *c != ' ').collect();
                    id.len() == 3 && id.starts_with(prefix) && id[1..].chars().all(|c| c.is_ascii_digit())
                })
                .unwrap_or(false)
        })
        .map(cells)
        .collect();
    assert!(!rows.is_empty(), "no {prefix} vectors in docs/updates-spec.md");
    rows
}

fn finish(name: &str, executed: usize, total: usize, failures: Vec<String>) {
    println!("updates-spec {name}: executed {executed} vectors of {total} in docs/updates-spec.md");
    assert_eq!(executed, total);
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}

fn dash(s: &str) -> Option<&str> {
    (s != "-").then_some(s)
}

fn yes(s: &str) -> bool {
    assert!(s == "yes" || s == "no", "bad yes/no cell {s}");
    s == "yes"
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

fn sym(o: std::cmp::Ordering) -> &'static str {
    match o {
        std::cmp::Ordering::Less => "<",
        std::cmp::Ordering::Equal => "=",
        std::cmp::Ordering::Greater => ">",
    }
}

#[test]
fn a_version_order() {
    let all = rows('A');
    let mut fails = vec![];
    for r in &all {
        let (a, b) = (Version::parse(&r[1]).unwrap(), Version::parse(&r[2]).unwrap());
        let got = sym(a.cmp(&b));
        if got != r[3] {
            fails.push(format!("{}: {} vs {} -> {got}, expected {}", r[0], r[1], r[2], r[3]));
        }
        // the order is antisymmetric
        let want_back = match r[3].as_str() {
            "<" => ">",
            ">" => "<",
            _ => "=",
        };
        if sym(b.cmp(&a)) != want_back {
            fails.push(format!("{}: reverse order {}", r[0], sym(b.cmp(&a))));
        }
    }
    finish("A", all.len(), all.len(), fails);
}

#[test]
fn b_version_syntax() {
    let all = rows('B');
    let mut fails = vec![];
    for r in &all {
        let got = Version::parse(&r[1]).map(|v| format!("ok:{v}")).unwrap_or_else(|| "-".into());
        if got != r[2] {
            fails.push(format!("{}: {:?} -> {got}, expected {}", r[0], r[1], r[2]));
        }
    }
    finish("B", all.len(), all.len(), fails);
}

/// The JSON of the GitHub listing for the spec of a row.
fn listing(spec: &str) -> String {
    if spec == "-" {
        return "[]".into();
    }
    let items: Vec<String> = spec
        .split('¶')
        .map(|r| {
            let (flag, tag) = match r.split_once(':') {
                Some((f, t)) if f == "pre" || f == "draft" || f == "noasset" => (f, t),
                _ => ("", r),
            };
            let assets = if flag == "noasset" {
                r#"[{"name":"Menew.AppImage","browser_download_url":"https://x/a"}]"#.to_string()
            } else {
                format!(r#"[{{"name":"update.json","browser_download_url":"https://x/{tag}/update.json"}}]"#)
            };
            format!(r#"{{"tag_name":"{tag}","draft":{},"prerelease":{},"assets":{assets}}}"#, flag == "draft", flag == "pre")
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[test]
fn c_choosing_the_release() {
    let all = rows('C');
    let mut fails = vec![];
    for r in &all {
        let cur = Version::parse(&r[1]).unwrap();
        let releases = parse_releases(&listing(&r[2]));
        let picked = pick_release(&releases, &cur);
        let got = picked.map(|x| x.tag.clone());
        if got.as_deref() != dash(&r[3]) {
            fails.push(format!("{}: running {} -> {got:?}, expected {}", r[0], r[1], r[3]));
        }
        if let Some(x) = picked {
            if x.manifest_url != format!("https://x/{}/update.json", x.tag) {
                fails.push(format!("{}: wrong manifest url {}", r[0], x.manifest_url));
            }
        }
    }
    finish("C", all.len(), all.len(), fails);
}

#[test]
fn d_update_json() {
    let all = rows('D');
    let art = r#"{"url":"https://x/f","size":1,"sha256":"ab","signature":"cd"}"#;
    let mut fails = vec![];
    for r in &all {
        let json = r[1].trim_matches('`').replace("ART", art);
        let got = match Manifest::parse(&json) {
            Ok(m) => format!("ok:{}:{}", m.version, m.artifacts.keys().cloned().collect::<Vec<_>>().join(",")),
            Err(_) => "err".to_string(),
        };
        if got != r[2] {
            fails.push(format!("{}: -> {got}, expected {}", r[0], r[2]));
        }
    }
    finish("D", all.len(), all.len(), fails);
}

fn keypair(seed: u8) -> KeyPair {
    KeyPair::from_seed(Seed::new([seed; 32]))
}

fn sign(kp: &KeyPair, version: &str, kind: &str, size: u64, sha: &str) -> String {
    b64(kp.sk.sign(signed_message(version, kind, size, sha).as_bytes(), None).as_ref())
}

#[test]
fn e_verifying_a_download() {
    let all = rows('E');
    let (version, kind) = ("0.1.0-alpha.2", "appimage");
    let content: Vec<u8> = (0..4000u32).map(|i| (i * 7 % 251) as u8).collect();
    let size = content.len() as u64;
    let sha = hex(&sha256(&content));
    let (good, other) = (keypair(1), keypair(2));
    let good_pub = b64(good.pk.as_ref());
    let dir = tempfile::tempdir().unwrap();
    let mut fails = vec![];
    for r in &all {
        let mut file = content.clone();
        let mut art = Artifact { url: "https://x/f".into(), size, sha256: sha.clone(), signature: sign(&good, version, kind, size, &sha) };
        let mut key = good_pub.clone();
        match r[0].as_str() {
            "E01" => {}
            "E02" => file.push(0),
            "E03" => file[100] ^= 1,
            "E04" => art.sha256 = hex(&sha256(b"another file")),
            "E05" => {
                art.size = size + 5;
                art.signature = sign(&good, version, kind, size + 5, &sha);
            }
            "E06" => art.signature = sign(&good, "0.1.0-alpha.1", kind, size, &sha),
            "E07" => art.signature = sign(&good, version, "deb", size, &sha),
            "E08" => art.signature = sign(&other, version, kind, size, &sha),
            "E09" => art.signature = String::new(),
            "E10" => art.signature = "!!!not base64!!!".into(),
            "E11" => art.signature = b64(&good.sk.sign(b"x", None).as_ref()[..63]),
            "E12" => key = b64(&[1u8; 31]),
            "E13" => art.signature = sign(&good, version, kind, size + 1, &sha),
            "E14" => art.sha256 = sha.to_uppercase(),
            id => panic!("no scenario for {id}"),
        }
        let path = dir.path().join(&r[0]);
        std::fs::write(&path, &file).unwrap();
        let got = match verify_artifact(&path, version, kind, &art, &key) {
            Ok(()) => "ok",
            Err(UpdateError::SizeMismatch) => "size",
            Err(UpdateError::HashMismatch) => "sha256",
            Err(UpdateError::BadSignature) => "signature",
            Err(e) => panic!("unexpected error {e}"),
        };
        if got != r[2] {
            fails.push(format!("{}: {} -> {got}, expected {}", r[0], r[1], r[2]));
        }
    }
    finish("E", all.len(), all.len(), fails);
}

#[test]
fn f_skipped_version() {
    let all = rows('F');
    let mut fails = vec![];
    for r in &all {
        let state = UpdateState { skipped: dash(&r[1]).map(str::to_string) };
        let got = announce_automatically(&state, &Version::parse(&r[2]).unwrap());
        if got != yes(&r[3]) {
            fails.push(format!("{}: skipped {} found {} -> {got}", r[0], r[1], r[2]));
        }
    }
    // the skipped version survives a restart
    let dir = tempfile::tempdir().unwrap();
    let path = UpdateState::path_in(dir.path());
    UpdateState { skipped: Some("0.1.0-alpha.2".into()) }.save_to(&path).unwrap();
    if UpdateState::load_from(&path).skipped.as_deref() != Some("0.1.0-alpha.2") {
        fails.push("F: the skipped version was not saved".into());
    }
    finish("F", all.len(), all.len(), fails);
}

#[test]
fn g_interval_and_gating() {
    let all = rows('G');
    let mut fails = vec![];
    for r in &all {
        if r.len() == 5 {
            let got = checks_allowed(yes(&r[1]), yes(&r[2]), yes(&r[3]));
            if got != yes(&r[4]) {
                fails.push(format!("{}: -> {got}", r[0]));
            }
        } else {
            let got = match r[1].as_str() {
                "AUTO_CHECK_INTERVAL_SECS" => AUTO_CHECK_INTERVAL_SECS,
                "STARTUP_CHECK_DELAY_SECS" => STARTUP_CHECK_DELAY_SECS,
                c => panic!("unknown constant {c}"),
            };
            if got.to_string() != r[2] {
                fails.push(format!("{}: {} = {got}", r[0], r[1]));
            }
        }
    }
    finish("G", all.len(), all.len(), fails);
}

#[test]
fn h_how_a_copy_is_installed() {
    let all = rows('H');
    let mut fails = vec![];
    for r in &all {
        let m = decide_install(dash(&r[1]), yes(&r[2]), yes(&r[3]));
        let got = match &m {
            InstallMethod::AppImage(p) => format!("appimage:{}", p.display()),
            InstallMethod::Deb => "deb".into(),
            InstallMethod::Manual => "manual".into(),
        };
        if got != r[4] {
            fails.push(format!("{}: -> {got}, expected {}", r[0], r[4]));
        }
        if artifact_kind(&m).unwrap_or("-") != r[5] {
            fails.push(format!("{}: artifact {:?}, expected {}", r[0], artifact_kind(&m), r[5]));
        }
    }
    finish("H", all.len(), all.len(), fails);
}

#[test]
fn the_embedded_key_is_a_32_byte_key_and_never_trusts_a_zero_signature() {
    assert_eq!(base64_decode(UPDATE_PUBLIC_KEY).unwrap().len(), 32);
    assert!(verify_signature(UPDATE_PUBLIC_KEY, b"x", &b64(&[0u8; 64])).is_err());
}
