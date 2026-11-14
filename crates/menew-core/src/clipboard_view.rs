//! Pure presentation logic for the clipboard mode of the palette: palette mode,
//! list ordering, one-line preview and relative age.

use crate::clipboard_db::ClipboardEntry;
use crate::i18n::{current_language, t_in, Language};

pub const PREVIEW_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteMode {
    #[default]
    Apps,
    Clipboard,
}

impl PaletteMode {
    /// Tab switches between the two modes.
    pub fn toggled(self) -> Self {
        match self {
            PaletteMode::Apps => PaletteMode::Clipboard,
            PaletteMode::Clipboard => PaletteMode::Apps,
        }
    }
}

/// Pinned entries first, then most recent first (ties broken by newest id).
pub fn sort_for_display(entries: &mut [ClipboardEntry]) {
    entries.sort_by(|a, b| {
        b.is_pinned
            .cmp(&a.is_pinned)
            .then(b.created_at.cmp(&a.created_at))
            .then(b.id.cmp(&a.id))
    });
}

/// First non-blank line, trimmed, cut to `max_chars` characters with an ellipsis.
pub fn preview(text: &str, max_chars: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        let cut: String = line.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}\u{2026}", cut.trim_end())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    Now,
    Minutes(u32),
    Hours(u32),
    Days(u32),
}

/// Relative age from two millisecond timestamps. Future timestamps count as "now".
pub fn age_between(now_ms: i64, created_ms: i64) -> Age {
    let secs = (now_ms - created_ms).max(0) / 1000;
    if secs < 60 {
        Age::Now
    } else if secs < 3600 {
        Age::Minutes((secs / 60) as u32)
    } else if secs < 86_400 {
        Age::Hours((secs / 3600) as u32)
    } else {
        Age::Days((secs / 86_400).min(u32::MAX as i64) as u32)
    }
}

/// Localized short age in the current language, e.g. "2 min", "3 h", "1 d".
pub fn format_age(age: Age) -> String {
    format_age_in(age, current_language())
}

pub fn format_age_in(age: Age, lang: Language) -> String {
    match age {
        Age::Now => t_in("age_now", lang).to_string(),
        Age::Minutes(n) => format!("{} {}", n, t_in("age_min", lang)),
        Age::Hours(n) => format!("{} {}", n, t_in("age_hour", lang)),
        Age::Days(n) => format!("{} {}", n, t_in("age_day", lang)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: i64, pinned: bool, created_at: i64) -> ClipboardEntry {
        ClipboardEntry::text_entry(id, &format!("t{id}"), pinned, created_at)
    }

    #[test]
    fn tab_toggles_between_modes() {
        assert_eq!(PaletteMode::Apps.toggled(), PaletteMode::Clipboard);
        assert_eq!(PaletteMode::Clipboard.toggled(), PaletteMode::Apps);
        assert_eq!(PaletteMode::default(), PaletteMode::Apps);
    }

    #[test]
    fn pinned_first_then_most_recent() {
        let mut v = vec![e(1, false, 100), e(2, true, 50), e(3, false, 300), e(4, true, 60)];
        sort_for_display(&mut v);
        let ids: Vec<i64> = v.iter().map(|x| x.id).collect();
        assert_eq!(ids, vec![4, 2, 3, 1]);
    }

    #[test]
    fn same_timestamp_prefers_newest_id() {
        let mut v = vec![e(1, false, 100), e(2, false, 100)];
        sort_for_display(&mut v);
        assert_eq!(v[0].id, 2);
    }

    #[test]
    fn preview_takes_first_nonblank_line() {
        assert_eq!(preview("hello\nworld", 80), "hello");
        assert_eq!(preview("\n\n  indented  \nnext", 80), "indented");
        assert_eq!(preview("   \n  ", 80), "");
    }

    #[test]
    fn preview_truncates_with_ellipsis() {
        let long = "a".repeat(200);
        let p = preview(&long, PREVIEW_MAX_CHARS);
        assert_eq!(p.chars().count(), PREVIEW_MAX_CHARS);
        assert!(p.ends_with('\u{2026}'));
        // exactly at the limit is untouched
        let exact = "b".repeat(PREVIEW_MAX_CHARS);
        assert_eq!(preview(&exact, PREVIEW_MAX_CHARS), exact);
        // multibyte characters are counted as characters, not bytes
        let cjk = "\u{4e2d}".repeat(100);
        assert_eq!(preview(&cjk, 10).chars().count(), 10);
    }

    #[test]
    fn age_buckets() {
        let now = 10_000_000_000;
        assert_eq!(age_between(now, now), Age::Now);
        assert_eq!(age_between(now, now + 5_000), Age::Now);
        assert_eq!(age_between(now, now - 59_000), Age::Now);
        assert_eq!(age_between(now, now - 60_000), Age::Minutes(1));
        assert_eq!(age_between(now, now - 120_000), Age::Minutes(2));
        assert_eq!(age_between(now, now - 3_600_000), Age::Hours(1));
        assert_eq!(age_between(now, now - 5 * 3_600_000), Age::Hours(5));
        assert_eq!(age_between(now, now - 86_400_000), Age::Days(1));
        assert_eq!(age_between(now, now - 3 * 86_400_000), Age::Days(3));
    }

    #[test]
    fn age_formatting_is_localized() {
        let en = Language::En;
        assert_eq!(format_age_in(Age::Minutes(2), en), "2 min");
        assert_eq!(format_age_in(Age::Hours(3), en), "3 h");
        assert_eq!(format_age_in(Age::Days(1), en), "1 d");
        assert_eq!(format_age_in(Age::Now, en), "now");
        assert_eq!(format_age_in(Age::Now, Language::Pt), "agora");
        assert_eq!(format_age_in(Age::Minutes(5), Language::De), "5 Min.");
    }
}
