//! Notes (port of `Notes.swift`): many notes, #tags, search, Markdown with checklists, protected
//! notes and password notes.
//!
//! Notes live in `~/.local/share/menew/notes.json`. A protected note keeps only its first line
//! (the title) in that file; the full text goes to the Secret Service (GNOME Keyring or KWallet)
//! through `secret-tool`, like the Mac keeps it in the Keychain. Opening a protected note asks for
//! the session password through polkit (`app.menew.Menew.unlock-notes`, the Touch ID of the Mac).

use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind as PKind, PaletteItem};
use crate::search::fold;
use crate::strings::tr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Text,
    Passwords,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    /// Protected notes keep only their first line here; the rest is in the Secret Service.
    pub text: String,
    #[serde(default)]
    pub secure: bool,
    /// Seconds since the Unix epoch.
    pub created: f64,
    pub updated: f64,
    /// `None` is a text (Markdown) note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<NoteKind>,
}

impl Note {
    pub fn new(text: &str, now: f64) -> Self {
        Self { id: crate::dev_tools::uuid_v4(), text: text.to_string(), secure: false, created: now, updated: now, kind: None }
    }

    pub fn is_passwords(&self) -> bool {
        self.kind == Some(NoteKind::Passwords)
    }

    /// First line with content, without heading marks, at most 80 characters.
    pub fn title(&self, lang: Language) -> String {
        let line = self.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let clean = line.trim_start_matches(['#', ' ']).trim();
        if clean.is_empty() {
            tr("note_untitled", lang, &[])
        } else {
            clean.chars().take(80).collect()
        }
    }

    /// Second line with content, without Markdown marks.
    pub fn preview(&self, lang: Language) -> String {
        if self.secure {
            return tr("note_protected_content", lang, &[]);
        }
        let mut lines = self.text.lines().map(str::trim).filter(|l| !l.is_empty());
        lines.next();
        let rest = lines.next().unwrap_or("");
        strip_marks(rest).chars().take(120).collect()
    }

    pub fn tags(&self) -> Vec<String> {
        if self.secure {
            vec![]
        } else {
            tags_in(&self.text)
        }
    }
}

/// Remove block marks (heading, quote, list, task, numbered) and emphasis.
pub fn strip_marks(line: &str) -> String {
    let mut s = line.trim_start();
    if s.starts_with('#') {
        s = s.trim_start_matches('#').trim_start();
    } else if let Some(r) = s.strip_prefix('>') {
        s = r.trim_start();
    } else if let Some(r) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")).or_else(|| s.strip_prefix("+ ")) {
        s = r.trim_start();
        for t in ["[ ]", "[x]", "[X]"] {
            if let Some(r2) = s.strip_prefix(t) {
                s = r2.trim_start();
            }
        }
    } else {
        let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && matches!(s[digits..].chars().next(), Some('.') | Some(')')) && s[digits + 1..].starts_with(' ') {
            s = s[digits + 2..].trim_start();
        }
    }
    s.chars().filter(|c| !matches!(c, '*' | '_' | '`' | '~')).collect()
}

/// `#tag` anywhere in the text (`# Title`, with a space, is not a tag). Lower case, in order.
pub fn tags_in(text: &str) -> Vec<String> {
    let mut seen: Vec<String> = vec![];
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
                j += 1;
            }
            if j > i + 1 {
                let t: String = chars[i + 1..j].iter().collect::<String>().to_lowercase();
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    seen
}

/// A `key: value` line of a password note. The text stays a title on the first line and one field
/// per line, so switching the kind loses nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretField {
    pub key: String,
    pub value: String,
}

pub fn parse_fields(text: &str) -> (String, Vec<SecretField>) {
    let mut lines = text.split('\n');
    let title = lines.next().unwrap_or("").to_string();
    let fields = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| match l.split_once(':') {
            Some((k, v)) => SecretField { key: k.trim().to_string(), value: v.strip_prefix(' ').unwrap_or(v).to_string() },
            None => SecretField { key: l.trim().to_string(), value: String::new() },
        })
        .collect();
    (title, fields)
}

pub fn format_fields(title: &str, fields: &[SecretField]) -> String {
    let flat = |s: &str| s.replace('\n', " ");
    let mut out = vec![flat(title)];
    for f in fields {
        let key = flat(&f.key).replace(':', "");
        out.push(if f.value.is_empty() { format!("{key}:") } else { format!("{key}: {}", flat(&f.value)) });
    }
    out.join("\n")
}

/// Where protected text is kept.
pub trait Vault {
    fn read(&self, id: &str) -> Option<String>;
    fn write(&self, id: &str, text: &str) -> bool;
    fn delete(&self, id: &str);
}

/// The Secret Service through `secret-tool` (package `libsecret-tools`).
pub struct SecretServiceVault;

const SERVICE: &str = "app.menew.Menew.notes";

impl Vault for SecretServiceVault {
    fn read(&self, id: &str) -> Option<String> {
        crate::sys::output("secret-tool", &["lookup", "service", SERVICE, "account", id])
    }

    fn write(&self, id: &str, text: &str) -> bool {
        let child = Command::new("secret-tool")
            .args(["store", "--label=Menew note", "service", SERVICE, "account", id])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { return false };
        if let Some(mut stdin) = child.stdin.take() {
            // secret-tool reads until EOF; no trailing newline is added
            if stdin.write_all(text.as_bytes()).is_err() {
                return false;
            }
        }
        child.wait().map(|s| s.success()).unwrap_or(false)
    }

    fn delete(&self, id: &str) {
        let _ = crate::sys::run("secret-tool", &["clear", "service", SERVICE, "account", id]);
    }
}

pub fn vault_available() -> bool {
    crate::sys::has("secret-tool")
}

/// In-memory vault for tests and the demo.
#[derive(Default)]
pub struct MemoryVault(pub std::cell::RefCell<HashMap<String, String>>);

impl Vault for MemoryVault {
    fn read(&self, id: &str) -> Option<String> {
        self.0.borrow().get(id).cloned()
    }
    fn write(&self, id: &str, text: &str) -> bool {
        self.0.borrow_mut().insert(id.to_string(), text.to_string());
        true
    }
    fn delete(&self, id: &str) {
        self.0.borrow_mut().remove(id);
    }
}

/// The polkit action that guards protected notes (installed by the packages).
pub const UNLOCK_ACTION: &str = "app.menew.Menew.unlock-notes";

/// Ask for the session password (polkit's own dialog). When the policy is not installed (running
/// from source), the Secret Service lock is the only guard and this returns true.
pub fn authenticate() -> bool {
    if !crate::sys::has("pkcheck") || !crate::sys::run("pkaction", &["--action-id", UNLOCK_ACTION]) {
        return true;
    }
    let pid = std::process::id().to_string();
    crate::sys::run("pkcheck", &["--action-id", UNLOCK_ACTION, "--process", &pid, "--allow-user-interaction"])
}

fn first_line(text: &str) -> String {
    text.split('\n').next().unwrap_or("").to_string()
}

pub struct NotesStore {
    pub notes: Vec<Note>,
    /// Protected notes opened in this session.
    unlocked: HashMap<String, String>,
    path: Option<PathBuf>,
    vault: Box<dyn Vault>,
}

impl NotesStore {
    pub fn default_path() -> PathBuf {
        crate::clipboard_db::data_dir().join("notes.json")
    }

    pub fn open(path: Option<PathBuf>, vault: Box<dyn Vault>) -> Self {
        let notes = path.as_deref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { notes, unlocked: HashMap::new(), path, vault }
    }

    pub fn open_default() -> Self {
        Self::open(Some(Self::default_path()), Box::new(SecretServiceVault))
    }

    pub fn save(&self) {
        let Some(p) = &self.path else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.notes) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, p);
            }
        }
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut seen: Vec<String> = vec![];
        for n in &self.notes {
            for t in n.tags() {
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
        }
        seen.sort();
        seen
    }

    /// Newest first, matching the text (accents and case ignored) and the tag.
    pub fn filtered(&self, query: &str, tag: Option<&str>) -> Vec<&Note> {
        let q = fold(query.trim());
        let mut v: Vec<&Note> = self
            .notes
            .iter()
            .filter(|n| tag.is_none_or(|t| n.tags().iter().any(|x| x == t)))
            .filter(|n| q.is_empty() || fold(&n.text).contains(&q))
            .collect();
        v.sort_by(|a, b| b.updated.partial_cmp(&a.updated).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    pub fn note(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.notes.iter().position(|n| n.id == id)
    }

    /// The full text: of a protected note only after unlocking.
    pub fn text(&self, id: &str) -> String {
        match self.note(id) {
            Some(n) if n.secure => self.unlocked.get(id).cloned().unwrap_or_default(),
            Some(n) => n.text.clone(),
            None => String::new(),
        }
    }

    pub fn is_locked(&self, id: &str) -> bool {
        self.note(id).is_some_and(|n| n.secure) && !self.unlocked.contains_key(id)
    }

    pub fn create(&mut self, text: &str, now: f64) -> String {
        let n = Note::new(text, now);
        let id = n.id.clone();
        self.notes.insert(0, n);
        self.save();
        id
    }

    pub fn set_text(&mut self, id: &str, text: &str, now: f64) {
        let Some(i) = self.index(id) else { return };
        if self.notes[i].secure {
            if !self.unlocked.contains_key(id) {
                return;
            }
            self.unlocked.insert(id.to_string(), text.to_string());
            self.vault.write(id, text);
            self.notes[i].text = first_line(text);
        } else {
            self.notes[i].text = text.to_string();
        }
        self.notes[i].updated = now;
        self.save();
    }

    /// A new password note: already protected, with user and password to fill in.
    pub fn create_passwords(&mut self, lang: Language, now: f64) -> String {
        let text = format_fields("", &[SecretField { key: tr("note_field_user", lang, &[]), value: String::new() }, SecretField { key: tr("note_field_password", lang, &[]), value: String::new() }]);
        let id = self.create(&text, now);
        self.set_kind(&id, NoteKind::Passwords);
        id
    }

    /// Password notes are always protected; going back to text keeps the protection.
    pub fn set_kind(&mut self, id: &str, kind: NoteKind) {
        let Some(i) = self.index(id) else { return };
        self.notes[i].kind = (kind == NoteKind::Passwords).then_some(kind);
        if kind == NoteKind::Passwords && !self.notes[i].secure {
            self.set_secure(id, true);
        } else {
            self.save();
        }
    }

    pub fn delete(&mut self, id: &str) {
        self.notes.retain(|n| n.id != id);
        self.unlocked.remove(id);
        self.vault.delete(id);
        self.save();
    }

    /// Protect (the text goes to the vault) or unprotect (back to the notes file). Returns false
    /// when the vault did not take the text, and then nothing changes.
    pub fn set_secure(&mut self, id: &str, secure: bool) -> bool {
        let Some(i) = self.index(id) else { return false };
        if self.notes[i].secure == secure {
            return true;
        }
        if secure {
            let full = self.notes[i].text.clone();
            if !self.vault.write(id, &full) {
                return false;
            }
            self.notes[i].secure = true;
            self.notes[i].text = first_line(&full);
            self.unlocked.insert(id.to_string(), full);
        } else {
            let Some(full) = self.unlocked.get(id).cloned() else { return false };
            self.notes[i].secure = false;
            self.notes[i].text = full;
            self.unlocked.remove(id);
            self.vault.delete(id);
        }
        self.save();
        true
    }

    /// After [`authenticate`]: read the text from the vault.
    pub fn unlock(&mut self, id: &str) -> bool {
        match self.vault.read(id) {
            Some(t) => {
                self.unlocked.insert(id.to_string(), t);
                true
            }
            None => false,
        }
    }

    /// Unlock with a text read elsewhere (the vault is read off the GTK thread).
    pub fn set_unlocked(&mut self, id: &str, text: String) {
        self.unlocked.insert(id.to_string(), text);
    }

    pub fn lock_all(&mut self) {
        self.unlocked.clear();
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Palette items: new note, new password note, and the notes that match the text (protected ones
/// by title only, never by content).
pub fn palette_items(store: &NotesStore, raw: &str, lang: Language) -> Vec<PaletteItem> {
    let mut out = vec![
        PaletteItem::new("action.newnote", PKind::Action, &tr("note_new", lang, &[]))
            .kw("nota note notiz nueva new nova neue anotar escrever write 笔记 新建")
            .icon(Icon::Symbol("document-new-symbolic"))
            .shortcut("Ctrl+Alt+J")
            .act(ItemAction::Command("note.new".into())),
        PaletteItem::new("action.notes", PKind::Action, &tr("note_open_all", lang, &[]))
            .kw("notas notes notizen notas anotações 笔记")
            .icon(Icon::Symbol("accessories-text-editor-symbolic"))
            .act(ItemAction::Command("open.notes".into())),
        PaletteItem::new("action.newpasswords", PKind::Action, &tr("note_new_passwords", lang, &[]))
            .kw("senha password passwort contraseña chave key cofre vault 密码")
            .icon(Icon::Symbol("dialog-password-symbolic"))
            .act(ItemAction::Command("note.newpasswords".into())),
    ];
    let q = fold(raw.trim());
    if q.chars().count() >= 2 {
        for n in &store.notes {
            let title = n.title(lang);
            let hit = fold(&title).contains(&q) || (!n.secure && fold(&n.text).contains(&q));
            if hit {
                let icon = if n.is_passwords() { "dialog-password-symbolic" } else if n.secure { "channel-secure-symbolic" } else { "text-x-generic-symbolic" };
                out.push(
                    PaletteItem::new(&format!("note.{}", n.id), PKind::Note, &title)
                        .subtitle(&n.preview(lang))
                        .kw(&format!("{} {}", raw, n.tags().join(" ")))
                        .icon(Icon::Symbol(icon))
                        .forget()
                        .act(ItemAction::Command(format!("note.open:{}", n.id))),
                );
            }
        }
    }
    out
}

/// Markdown lines for the editor's styling (`Markdown.swift`, block level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Heading(u8),
    Quote,
    Bullet,
    Task(bool),
    Numbered,
    Code,
    Plain,
}

pub fn block_of(line: &str) -> Block {
    let t = line.trim_start();
    if t.starts_with("```") {
        return Block::Code;
    }
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        return Block::Heading(hashes as u8);
    }
    if t.starts_with("> ") || t == ">" {
        return Block::Quote;
    }
    for m in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(m) {
            if r.starts_with("[ ]") {
                return Block::Task(false);
            }
            if r.starts_with("[x]") || r.starts_with("[X]") {
                return Block::Task(true);
            }
            return Block::Bullet;
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")) {
        return Block::Numbered;
    }
    Block::Plain
}

/// Flip `[ ]` and `[x]` on a task line.
pub fn toggle_task(line: &str) -> String {
    if let Some(i) = line.find("[ ]") {
        format!("{}[x]{}", &line[..i], &line[i + 3..])
    } else if let Some(i) = line.find("[x]").or_else(|| line.find("[X]")) {
        format!("{}[ ]{}", &line[..i], &line[i + 3..])
    } else {
        line.to_string()
    }
}

/// Inline spans for **bold**, *italic*, `code` and ~~strike~~ as (start, end, style) in char offsets.
pub fn inline_spans(line: &str) -> Vec<(usize, usize, &'static str)> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    let find = |from: usize, pat: &[char]| -> Option<usize> { (from..=chars.len().saturating_sub(pat.len())).find(|&j| chars[j..j + pat.len()] == *pat) };
    while i < chars.len() {
        let rest = &chars[i..];
        let (pat, style): (&[char], &str) = if rest.starts_with(&['*', '*']) {
            (&['*', '*'], "bold")
        } else if rest.starts_with(&['~', '~']) {
            (&['~', '~'], "strike")
        } else if rest[0] == '`' {
            (&['`'], "code")
        } else if rest[0] == '*' || rest[0] == '_' {
            (if rest[0] == '*' { &['*'] } else { &['_'] }, "italic")
        } else {
            i += 1;
            continue;
        };
        let start = i + pat.len();
        match find(start, pat) {
            Some(end) if end > start => {
                out.push((i, end + pat.len(), style));
                i = end + pat.len();
            }
            _ => i += pat.len(),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> NotesStore {
        NotesStore::open(None, Box::new(MemoryVault::default()))
    }

    #[test]
    fn title_preview_and_tags_like_the_mac() {
        let n = Note::new("# Menew ideas\n- [x] Snippets\n- [ ] Per-app\n\n#ideas #Menew #ideas", 0.0);
        assert_eq!(n.title(Language::En), "Menew ideas");
        assert_eq!(n.preview(Language::En), "Snippets");
        assert_eq!(n.tags(), vec!["ideas", "menew"]);
        assert_eq!(Note::new("   \n", 0.0).title(Language::Pt), "Nota sem título");
        assert_eq!(tags_in("# Title\nnot#tag but #ok_1 and #a-b"), vec!["ok_1", "a-b"]);
        assert_eq!(strip_marks("1. **first** step"), "first step");
        assert_eq!(strip_marks("> `quote`"), "quote");
    }

    #[test]
    fn fields_round_trip() {
        let (t, f) = parse_fields("Home Wi-Fi\nNetwork: Casa-5G\nPassword: a:b c\nPIN\n");
        assert_eq!(t, "Home Wi-Fi");
        assert_eq!(f[1], SecretField { key: "Password".into(), value: "a:b c".into() });
        assert_eq!(f[2], SecretField { key: "PIN".into(), value: "".into() });
        assert_eq!(format_fields(&t, &f), "Home Wi-Fi\nNetwork: Casa-5G\nPassword: a:b c\nPIN:");
        assert_eq!(format_fields("a\nb", &[SecretField { key: "k:\n".into(), value: "v\nw".into() }]), "a b\nk : v w");
    }

    #[test]
    fn protected_notes_keep_only_the_title_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        let vault = MemoryVault::default();
        let mut s = NotesStore::open(Some(path.clone()), Box::new(vault));
        let id = s.create("Bank\nPIN: 1234", 1.0);
        assert!(s.set_secure(&id, true));
        let file = std::fs::read_to_string(&path).unwrap();
        assert!(file.contains("Bank") && !file.contains("1234"), "{file}");
        assert_eq!(s.text(&id), "Bank\nPIN: 1234");
        s.lock_all();
        assert!(s.is_locked(&id));
        assert_eq!(s.text(&id), "");
        // editing a locked note does nothing
        s.set_text(&id, "hacked", 2.0);
        assert!(s.unlock(&id));
        assert_eq!(s.text(&id), "Bank\nPIN: 1234");
        s.set_text(&id, "Bank 2\nPIN: 9", 3.0);
        assert_eq!(s.note(&id).unwrap().text, "Bank 2");
        assert!(s.set_secure(&id, false));
        assert_eq!(s.note(&id).unwrap().text, "Bank 2\nPIN: 9");
        // reopening reads the same file
        let again = NotesStore::open(Some(path), Box::new(MemoryVault::default()));
        assert_eq!(again.notes.len(), 1);
    }

    #[test]
    fn password_notes_are_always_protected() {
        let mut s = store();
        let id = s.create_passwords(Language::En, 1.0);
        let n = s.note(&id).unwrap();
        assert!(n.secure && n.is_passwords());
        assert_eq!(s.text(&id), "\nUser:\nPassword:");
        s.set_kind(&id, NoteKind::Text);
        assert!(s.note(&id).unwrap().secure);
    }

    #[test]
    fn search_and_tags_skip_protected_content() {
        let mut s = store();
        s.create("Groceries\n- coffee\n#home", 1.0);
        let b = s.create("Bank\nPIN coffee", 2.0);
        s.set_secure(&b, true);
        s.create("Trip\n#travel #home", 3.0);
        assert_eq!(s.filtered("", None).len(), 3);
        assert_eq!(s.filtered("", None)[0].title(Language::En), "Trip");
        assert_eq!(s.filtered("COFFÉE", None).len(), 1);
        assert_eq!(s.filtered("", Some("home")).len(), 2);
        assert_eq!(s.all_tags(), vec!["home", "travel"]);
        let items = palette_items(&s, "coffee", Language::En);
        assert_eq!(items.iter().filter(|i| i.kind == PKind::Note).count(), 1);
        let items = palette_items(&s, "bank", Language::En);
        assert_eq!(items.iter().filter(|i| i.kind == PKind::Note).count(), 1);
    }

    #[test]
    fn markdown_blocks_and_spans() {
        assert_eq!(block_of("## Title"), Block::Heading(2));
        assert_eq!(block_of("#tag"), Block::Plain);
        assert_eq!(block_of("- [ ] todo"), Block::Task(false));
        assert_eq!(block_of("  * [x] done"), Block::Task(true));
        assert_eq!(block_of("- item"), Block::Bullet);
        assert_eq!(block_of("2) two"), Block::Numbered);
        assert_eq!(block_of("> said"), Block::Quote);
        assert_eq!(toggle_task("- [ ] a"), "- [x] a");
        assert_eq!(toggle_task("- [X] a"), "- [ ] a");
        assert_eq!(inline_spans("a **b** `c` *d* ~~e~~"), vec![(2, 7, "bold"), (8, 11, "code"), (12, 15, "italic"), (16, 21, "strike")]);
        assert_eq!(inline_spans("2 * 3 = 6"), vec![]);
    }
}
