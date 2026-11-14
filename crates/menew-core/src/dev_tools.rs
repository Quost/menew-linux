//! Tools for developers in the palette (from `Palette.swift`): generate a UUID, copy the Unix
//! timestamp, and turn a pasted timestamp ("1793400000", "1793400000000") into a date.
//! The JWT decoder lives in `text_transforms` (it works on the copied text, like the Mac's).

use crate::format::date_long_secs;
use crate::i18n::Language;
use std::io::Read;

/// A random (version 4) UUID, lowercase.
pub fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_ok();
    if !ok {
        // no /dev/urandom (not Linux): the std hasher is randomly seeded per process
        use std::hash::{BuildHasher, Hasher};
        for (i, chunk) in b.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i as u64 ^ now_secs() as u64);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    format_uuid(b)
}

pub fn format_uuid(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        hex[0..4].concat(),
        hex[4..6].concat(),
        hex[6..8].concat(),
        hex[8..10].concat(),
        hex[10..16].concat()
    )
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// What a pasted timestamp means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampInfo {
    /// The date in the computer's zone, localized ("Nov 14, 2026 at 3:00:00 PM").
    pub text: String,
    /// ISO 8601 in UTC, what Enter copies ("2026-11-14T18:00:00Z").
    pub iso: String,
}

/// 10 digits are seconds, 13 are milliseconds; anything else is not a timestamp.
/// `local_offset` is the computer's UTC offset in seconds at that instant.
pub fn timestamp_query(q: &str, lang: Language, local_offset: &dyn Fn(i64) -> i32) -> Option<TimestampInfo> {
    if !q.chars().all(|c| c.is_ascii_digit()) || !(q.len() == 10 || q.len() == 13) {
        return None;
    }
    let n: i64 = q.parse().ok()?;
    let secs = if q.len() == 13 { n / 1000 } else { n };
    let utc = chrono::DateTime::from_timestamp(secs, 0)?;
    let local = chrono::DateTime::from_timestamp(secs + local_offset(secs) as i64, 0)?.naive_utc();
    Some(TimestampInfo {
        text: date_long_secs(&local, lang),
        iso: utc.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_has_the_v4_shape() {
        let u = uuid_v4();
        assert_eq!(u.len(), 36);
        assert_eq!(u.matches('-').count(), 4);
        assert_eq!(&u[14..15], "4");
        assert!("89ab".contains(&u[19..20]));
        assert_eq!(u, u.to_lowercase());
        assert_ne!(uuid_v4(), uuid_v4());
    }

    #[test]
    fn fixed_bytes_make_a_known_uuid() {
        assert_eq!(format_uuid([0; 16]), "00000000-0000-4000-8000-000000000000");
        assert_eq!(format_uuid([0xff; 16]), "ffffffff-ffff-4fff-bfff-ffffffffffff");
    }
}
