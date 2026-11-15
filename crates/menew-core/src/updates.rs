//! The update check (port of the Mac's daily check of this repo's releases). Off until people turn
//! it on; one request a day to the GitHub releases API through the system `curl`, nothing else is
//! sent. Installing stays with the package manager or the AppImage.

pub const RELEASES_API: &str = "https://api.github.com/repos/Quost/menew-linux/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/Quost/menew-linux/releases/latest";

/// `v0.2.0` -> `[0, 2, 0]`; a pre-release suffix (`-alpha.1`) sorts before the release.
fn parts(v: &str) -> (Vec<u64>, Option<String>) {
    let v = v.trim().trim_start_matches('v');
    let (main, pre) = match v.split_once('-') {
        Some((m, p)) => (m, Some(p.to_string())),
        None => (v, None),
    };
    (main.split('.').map(|x| x.parse().unwrap_or(0)).collect(), pre)
}

/// Whether `latest` is newer than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, ap) = parts(latest);
    let (b, bp) = parts(current);
    let n = a.len().max(b.len());
    for i in 0..n {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    match (ap, bp) {
        (None, Some(_)) => true,
        (Some(x), Some(y)) => x > y,
        _ => false,
    }
}

/// `tag_name` of the releases API answer.
pub fn parse_tag(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("tag_name")?.as_str().map(str::to_string)
}

/// Ask GitHub for the latest release tag (blocking).
pub fn fetch_latest() -> Option<String> {
    let out = crate::sys::output("curl", &["-fsSL", "--max-time", "15", "-H", "Accept: application/vnd.github+json", "-A", "menew-linux", RELEASES_API])?;
    parse_tag(&out)
}

/// Due once a day.
pub fn due(last: f64, now: f64) -> bool {
    now - last >= 86_400.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.2.0", "0.1.0-alpha.1"));
        assert!(is_newer("v0.1.0", "0.1.0-alpha.1"));
        assert!(is_newer("0.1.0-alpha.2", "0.1.0-alpha.1"));
        assert!(!is_newer("v0.1.0-alpha.1", "0.1.0-alpha.1"));
        assert!(!is_newer("v0.1.0", "0.1.1"));
        assert!(is_newer("1.10.0", "1.9.9"));
    }

    #[test]
    fn tag_and_schedule() {
        assert_eq!(parse_tag(r#"{"tag_name":"v0.2.0","name":"x"}"#).as_deref(), Some("v0.2.0"));
        assert_eq!(parse_tag("nope"), None);
        assert!(due(0.0, 90_000.0));
        assert!(!due(10_000.0, 20_000.0));
    }
}
