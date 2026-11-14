//! What the clipboard list knows about one entry (port of `ClipItem` and `ClipFilter` in
//! `ClipboardManager.swift`): kinds, filters, titles, links, colors and code detection. Pure.

use crate::clipboard_db::{ClipKind, ClipboardEntry};
use crate::i18n::Language;
use crate::strings::tr;

/// Default size of the history and the smallest one that can be set (`maxItems` on the Mac).
pub const DEFAULT_MAX_ITEMS: usize = 300;
pub const MIN_MAX_ITEMS: usize = 20;

/// The chips above the list. The Mac also has "Passwords", which comes with the notes (Phase 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipFilter {
    #[default]
    All,
    Text,
    Link,
    Image,
    File,
    Pinned,
    Snippets,
}

impl ClipFilter {
    pub const ALL: [ClipFilter; 7] =
        [ClipFilter::All, ClipFilter::Text, ClipFilter::Link, ClipFilter::Image, ClipFilter::File, ClipFilter::Pinned, ClipFilter::Snippets];

    pub fn id(self) -> &'static str {
        match self {
            ClipFilter::All => "all",
            ClipFilter::Text => "text",
            ClipFilter::Link => "link",
            ClipFilter::Image => "image",
            ClipFilter::File => "file",
            ClipFilter::Pinned => "pinned",
            ClipFilter::Snippets => "snippets",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    pub fn label(self, lang: Language) -> String {
        tr(&format!("clip_f_{}", self.id()), lang, &[])
    }

    /// The next filter, wrapping around (Tab on the Mac).
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// `ClipFilter.matches`. Snippets are not history entries, so none matches.
    pub fn matches(self, e: &ClipboardEntry) -> bool {
        match self {
            ClipFilter::All => true,
            ClipFilter::Text => e.kind == ClipKind::Text && url(e).is_none(),
            ClipFilter::Link => url(e).is_some(),
            ClipFilter::Image => e.kind == ClipKind::Image,
            ClipFilter::File => e.kind == ClipKind::File,
            ClipFilter::Pinned => e.is_pinned,
            ClipFilter::Snippets => false,
        }
    }
}

/// The text on one line, whitespace collapsed, at most 300 characters (`ClipItem.preview`).
pub fn collapsed_preview(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect()
}

/// The first line with something in it, trimmed, at most 200 characters (`ClipItem.title`).
pub fn first_line_title(text: &str) -> String {
    text.split(['\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}'])
        .find(|l| !l.trim_matches(|c| c == ' ' || c == '\t').is_empty())
        .map(|l| l.trim_matches(|c| c == ' ' || c == '\t').chars().take(200).collect())
        .unwrap_or_default()
}

/// Several lines that look like source code (`ClipItem.isCode`).
pub fn is_code(e: &ClipboardEntry) -> bool {
    e.kind == ClipKind::Text
        && e.text_content.contains('\n')
        && ["{", "=>", "func ", "def ", "</", "import ", "const ", "SELECT "].iter().any(|k| e.text_content.contains(k))
}

/// The address when the whole text is one `http` or `https` link (`ClipItem.url`).
pub fn url(e: &ClipboardEntry) -> Option<String> {
    if e.kind != ClipKind::Text {
        return None;
    }
    let t = e.text_content.trim();
    if t.contains(' ') || t.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"))?;
    if rest.is_empty() {
        return None;
    }
    Some(t.to_string())
}

/// `#1e90ff` or `1e90ff` as red, green and blue, for the swatch of the list (`ClipItem.hexColor`).
pub fn hex_color(e: &ClipboardEntry) -> Option<(u8, u8, u8)> {
    if e.kind != ClipKind::Text {
        return None;
    }
    let t = e.text_content.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    if t.chars().count() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The paths of a file entry, one per line.
pub fn file_paths(e: &ClipboardEntry) -> Vec<&str> {
    e.text_content.split('\n').filter(|p| !p.is_empty()).collect()
}

fn last_component(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

/// The title of a row in the list (`ClipRow`): the first line of a text, the file name (and how many
/// more) of files, "Image WxH" of an image.
pub fn row_title(e: &ClipboardEntry, lang: Language) -> String {
    match e.kind {
        ClipKind::File => {
            let paths = file_paths(e);
            let first = paths.first().map(|p| last_component(p)).unwrap_or("");
            if paths.len() > 1 {
                tr("clip_and_more", lang, &[first, &(paths.len() - 1).to_string()])
            } else {
                first.to_string()
            }
        }
        ClipKind::Image => first_line_title(&e.text_content).replace("Imagem", &tr("clip_image", lang, &[])),
        ClipKind::Text => first_line_title(&e.text_content),
    }
}

/// "3 files" and so on, for the details of a file entry.
pub fn files_count_text(n: usize, lang: Language) -> String {
    if n == 1 {
        tr("clip_files_one", lang, &[])
    } else {
        tr("clip_files_n", lang, &[&n.to_string()])
    }
}

/// Characters, words and lines of a text (the details line of the preview).
pub fn text_stats(text: &str, lang: Language) -> String {
    let words = text.split_whitespace().count();
    let lines = text.split('\n').count();
    tr(if lines == 1 { "clip_stats_one" } else { "clip_stats_n" }, lang, &[&text.chars().count().to_string(), &words.to_string(), &lines.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str) -> ClipboardEntry {
        ClipboardEntry::text_entry(1, text, false, 0)
    }

    #[test]
    fn links_and_filters() {
        assert!(url(&t("https://github.com/Quost/menew")).is_some());
        assert!(url(&t("  http://a.b/c  \n")).is_some());
        assert!(url(&t("see https://a.b")).is_none());
        assert!(url(&t("ftp://a.b")).is_none());
        assert!(url(&t("https://")).is_none());
        assert!(ClipFilter::Link.matches(&t("https://a.b")));
        assert!(!ClipFilter::Text.matches(&t("https://a.b")));
        assert!(ClipFilter::Text.matches(&t("hello")));
        assert!(!ClipFilter::Snippets.matches(&t("hello")));
        assert_eq!(ClipFilter::All.next(), ClipFilter::Text);
        assert_eq!(ClipFilter::Snippets.next(), ClipFilter::All);
        assert_eq!(ClipFilter::All.prev(), ClipFilter::Snippets);
    }

    #[test]
    fn colors() {
        assert_eq!(hex_color(&t("#6C5CE7")), Some((0x6C, 0x5C, 0xE7)));
        assert_eq!(hex_color(&t("1e90ff")), Some((0x1e, 0x90, 0xff)));
        assert_eq!(hex_color(&t("#12345")), None);
        assert_eq!(hex_color(&t("#12345g")), None);
    }

    #[test]
    fn titles() {
        assert_eq!(first_line_title("\n  \n  hello  \nworld"), "hello");
        assert_eq!(collapsed_preview("a   b\n\nc"), "a b c");
        assert_eq!(first_line_title(&"x".repeat(300)).chars().count(), 200);
        assert_eq!(collapsed_preview(&"x ".repeat(400)).chars().count(), 300);
    }

    #[test]
    fn code() {
        assert!(is_code(&t("func a() {\n}\n")));
        assert!(!is_code(&t("func a() {}")));
    }

    #[test]
    fn file_and_image_titles() {
        let mut f = t("/home/u/docs/report.pdf\n/home/u/a.txt\n/home/u/b.txt");
        f.kind = ClipKind::File;
        assert_eq!(row_title(&f, Language::En), "report.pdf and 2 more");
        f.text_content = "/home/u/docs/report.pdf".into();
        assert_eq!(row_title(&f, Language::En), "report.pdf");
        let mut i = t("Imagem 640\u{d7}480");
        i.kind = ClipKind::Image;
        assert_eq!(row_title(&i, Language::En), "Image 640\u{d7}480");
        assert_eq!(row_title(&i, Language::De), "Bild 640\u{d7}480");
    }
}
