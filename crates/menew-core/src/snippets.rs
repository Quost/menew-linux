//! Snippets (port of `Snippets.swift`): saved texts with variables that are filled in on paste.
//! `{date}`, `{time}`, `{weekday}`, `{tomorrow}` and `{clipboard}` are replaced when pasting, `{cursor}`
//! says where the cursor ends up and `{input:Name}` is asked first. Stored in `snippets.json` in
//! the data folder, with the same keys as the Mac file. Abbreviations typed in any app (the text
//! expander) are not part of this port: they need an input method, see `docs/parity.md`.

use crate::format::{date_medium, format_time};
use crate::i18n::Language;
use crate::strings::tr;
use chrono::{Datelike, Duration, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub text: String,
    /// Kept so that a file from the Mac keeps its abbreviations; not expanded on Linux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abbreviation: Option<String>,
}

impl Snippet {
    pub fn new(title: &str, text: &str) -> Self {
        Self { id: crate::dev_tools::uuid_v4().to_uppercase(), title: title.to_string(), text: text.to_string(), abbreviation: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SnippetStore {
    path: Option<PathBuf>,
    pub snippets: Vec<Snippet>,
}

pub fn default_path() -> PathBuf {
    crate::clipboard_db::data_dir().join("snippets.json")
}

impl SnippetStore {
    /// An in-memory store (tests, demos).
    pub fn in_memory(snippets: Vec<Snippet>) -> Self {
        Self { path: None, snippets }
    }

    /// Read the file, or start with the two examples of the Mac ("Today's date" and the e-mail
    /// signature) on the first run, and write them.
    pub fn load(path: &Path, lang: Language) -> Self {
        let read = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Vec<Snippet>>(&s).ok());
        match read {
            Some(snippets) => Self { path: Some(path.to_path_buf()), snippets },
            None => {
                let mut date = Snippet::new(&tr("snip_ex_date", lang, &[]), "{date}");
                date.abbreviation = Some(";date".into());
                let sig = Snippet::new(&tr("snip_ex_sig", lang, &[]), &tr("snip_ex_sig_text", lang, &[]));
                let store = Self { path: Some(path.to_path_buf()), snippets: vec![date, sig] };
                store.save();
                store
            }
        }
    }

    pub fn load_default(lang: Language) -> Self {
        Self::load(&default_path(), lang)
    }

    /// Insert at the top, or replace the snippet with the same id.
    pub fn upsert(&mut self, s: Snippet) {
        match self.snippets.iter().position(|x| x.id == s.id) {
            Some(i) => self.snippets[i] = s,
            None => self.snippets.insert(0, s),
        }
        self.save();
    }

    pub fn delete(&mut self, id: &str) {
        self.snippets.retain(|s| s.id != id);
        self.save();
    }

    pub fn find(&self, id: &str) -> Option<&Snippet> {
        self.snippets.iter().find(|s| s.id == id)
    }

    /// The snippets of the "Snippets" filter: those whose title or text contains the query
    /// (case and accent insensitive).
    pub fn matching(&self, query: &str) -> Vec<&Snippet> {
        let q = crate::search::fold(query.trim());
        self.snippets.iter().filter(|s| q.is_empty() || crate::search::fold(&format!("{} {}", s.title, s.text)).contains(&q)).collect()
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.snippets) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }
}

const WEEKDAYS_WIDE: [[&str; 7]; 5] = [
    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    ["segunda-feira", "ter\u{e7}a-feira", "quarta-feira", "quinta-feira", "sexta-feira", "s\u{e1}bado", "domingo"],
    ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"],
    ["lunes", "martes", "mi\u{e9}rcoles", "jueves", "viernes", "s\u{e1}bado", "domingo"],
    ["\u{661f}\u{671f}\u{4e00}", "\u{661f}\u{671f}\u{4e8c}", "\u{661f}\u{671f}\u{4e09}", "\u{661f}\u{671f}\u{56db}", "\u{661f}\u{671f}\u{4e94}", "\u{661f}\u{671f}\u{516d}", "\u{661f}\u{671f}\u{65e5}"],
];

fn lang_index(lang: Language) -> usize {
    match lang {
        Language::En => 0,
        Language::Pt => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Zh => 4,
    }
}

/// The wide weekday name (`.dateTime.weekday(.wide)`).
pub fn weekday_wide(dt: &NaiveDateTime, lang: Language) -> &'static str {
    WEEKDAYS_WIDE[lang_index(lang)][dt.weekday().num_days_from_monday() as usize]
}

/// Text ready to paste and how many characters the cursor goes back because of `{cursor}`.
/// Like the Mac: the variables first, `{clipboard}` after them (so the copied text is not expanded
/// itself), then the first `{cursor}` is cut out and the others are dropped.
pub fn expand_with_cursor(text: &str, now: &NaiveDateTime, clipboard: &str, lang: Language) -> (String, usize) {
    let mut out = text.to_string();
    out = out.replace("{date}", &date_medium(now, lang));
    out = out.replace("{time}", &format_time(now.hour(), now.minute(), lang));
    out = out.replace("{weekday}", weekday_wide(now, lang));
    out = out.replace("{tomorrow}", &date_medium(&(*now + Duration::days(1)), lang));
    if out.contains("{clipboard}") {
        out = out.replace("{clipboard}", clipboard);
    }
    match out.find("{cursor}") {
        None => (out, 0),
        Some(i) => {
            let back = out[i + "{cursor}".len()..].chars().count();
            out.replace_range(i..i + "{cursor}".len(), "");
            (out.replace("{cursor}", ""), back)
        }
    }
}

/// The local date and time now (what `{date}` and `{time}` are filled from).
pub fn local_now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

/// `expand_with_cursor` without the cursor (copy only, no paste).
pub fn expand(text: &str, now: &NaiveDateTime, clipboard: &str, lang: Language) -> String {
    expand_with_cursor(text, now, clipboard, lang).0
}

/// The `{input:Name}` fields, in order and without repeats.
pub fn fields_in(text: &str) -> Vec<String> {
    let mut seen: Vec<String> = vec![];
    let mut rest = text;
    while let Some(i) = rest.find("{input:") {
        let after = &rest[i + 7..];
        match after.find('}') {
            Some(end) if end > 0 => {
                let name = after[..end].trim().to_string();
                if !seen.contains(&name) {
                    seen.push(name);
                }
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    seen
}

/// Replace every `{input:Name}` with the value typed for that name.
pub fn apply(text: &str, values: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (name, value) in values {
        let mut result = String::new();
        let mut rest = out.as_str();
        while let Some(i) = rest.find("{input:") {
            result.push_str(&rest[..i]);
            let after = &rest[i + 7..];
            match after.find('}') {
                Some(end) if end > 0 && after[..end].trim() == name => {
                    result.push_str(value);
                    rest = &after[end + 1..];
                }
                _ => {
                    result.push_str("{input:");
                    rest = after;
                }
            }
        }
        result.push_str(rest);
        out = result;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(15, 5, 0).unwrap()
    }

    #[test]
    fn variables() {
        assert_eq!(expand("{date}", &at(), "", Language::En), "Nov 14, 2026");
        assert_eq!(expand("{time}", &at(), "", Language::En), "3:05 PM");
        assert_eq!(expand("{time}", &at(), "", Language::De), "15:05");
        assert_eq!(expand("{weekday}", &at(), "", Language::Pt), "s\u{e1}bado");
        assert_eq!(expand("{tomorrow}", &at(), "", Language::Pt), "15 de nov. de 2026");
        assert_eq!(expand("{date}", &at(), "", Language::Zh), "2026\u{5e74}11\u{6708}14\u{65e5}");
    }

    #[test]
    fn clipboard_is_not_expanded_itself() {
        assert_eq!(expand("a {clipboard} b", &at(), "{date}", Language::En), "a {date} b");
    }

    #[test]
    fn cursor_goes_back_by_the_text_after_it() {
        assert_eq!(expand_with_cursor("Hi {cursor}, bye", &at(), "", Language::En), ("Hi , bye".to_string(), 5));
        assert_eq!(expand_with_cursor("no cursor", &at(), "", Language::En), ("no cursor".to_string(), 0));
        // the second marker is dropped, but the Mac counts its 8 characters in the way back
        assert_eq!(expand_with_cursor("a{cursor}b{cursor}c", &at(), "", Language::En), ("abc".to_string(), 10));
    }

    #[test]
    fn input_fields() {
        let t = "Dear {input:Name}, {input: Name }. Re: {input:Topic} {input:}";
        assert_eq!(fields_in(t), vec!["Name".to_string(), "Topic".to_string()]);
        let v = vec![("Name".to_string(), "Ana".to_string()), ("Topic".to_string(), "x".to_string())];
        assert_eq!(apply(t, &v), "Dear Ana, Ana. Re: x {input:}");
    }

    #[test]
    fn store_round_trip_and_examples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        let mut s = SnippetStore::load(&path, Language::En);
        assert_eq!(s.snippets.len(), 2);
        assert_eq!(s.snippets[0].text, "{date}");
        assert!(path.exists());
        let n = Snippet::new("Hello", "Hi {input:Name}");
        s.upsert(n.clone());
        assert_eq!(s.snippets[0].id, n.id);
        let again = SnippetStore::load(&path, Language::En);
        assert_eq!(again.snippets.len(), 3);
        assert_eq!(again.matching("hello").len(), 1);
        let mut s = again;
        s.delete(&n.id);
        assert_eq!(SnippetStore::load(&path, Language::En).snippets.len(), 2);
    }
}
