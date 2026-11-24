//! Notes (port of `Notes.swift`): many notes with `#tags`, search, Markdown, and three kinds - normal,
//! protected and password. The file is `notes.json` with the same keys as the Mac (`id`, `text`, `secure`,
//! `created`, `updated`, `kind`; dates are seconds since 2001-01-01 like Swift's `Date`), in
//! `~/.local/share/menew/notes/`, so that a Mac file can be read here. Protected and password notes keep
//! **only their first line** in that file: the rest goes to the Secret Service (GNOME Keyring, KWallet)
//! through the [`Vault`] trait, which `menew-clipboard` implements over D-Bus. Pure: no GTK, no D-Bus.
//! `docs/notes-spec.md` has the vectors.

use crate::i18n::Language;
use crate::markdown;
use crate::search::fold;
use crate::strings::tr;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Seconds between 1970-01-01 and 2001-01-01, the reference date of Swift's `Date`.
pub const SWIFT_EPOCH_OFFSET: f64 = 978_307_200.0;

/// A copied password goes away from the clipboard after this many seconds (`SecretCopy`).
pub const SECRET_CLEAR_SECS: u64 = 30;

/// Seconds until a copied secret is cleared. `MENEW_TEST=1` plus `MENEW_SECRET_CLEAR_SECS=3` shortens it for
/// the desktop tests; without `MENEW_TEST=1` the variable is ignored.
pub fn secret_clear_secs(test_mode: bool, override_secs: Option<&str>) -> u64 {
    if test_mode {
        if let Some(n) = override_secs.and_then(|s| s.trim().parse::<u64>().ok()).filter(|n| *n > 0) {
            return n;
        }
    }
    SECRET_CLEAR_SECS
}

pub fn now_ref() -> f64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    unix - SWIFT_EPOCH_OFFSET
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Passwords,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// UUID in capitals, like Swift's `UUID.uuidString`.
    pub id: String,
    /// Protected notes keep only the first line (the title) here.
    pub text: String,
    #[serde(default)]
    pub secure: bool,
    pub created: f64,
    pub updated: f64,
    /// `None` is a text note (Markdown); optional so that notes saved before still open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
}

fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

impl Note {
    pub fn new(text: &str) -> Self {
        let now = now_ref();
        Self { id: crate::dev_tools::uuid_v4().to_uppercase(), text: text.to_string(), secure: false, created: now, updated: now, kind: None }
    }

    pub fn is_passwords(&self) -> bool {
        self.kind == Some(Kind::Passwords)
    }

    /// First line with content, without the leading `#` and spaces, at most 80 characters (`Note.title`).
    pub fn title(&self, lang: Language) -> String {
        let line = self.text.split(is_newline).find(|l| !markdown::trim_blanks(l).is_empty()).unwrap_or("");
        let clean = markdown::trim_blanks(line.trim_start_matches(['#', ' ']));
        if clean.is_empty() {
            tr("note_untitled", lang, &[])
        } else {
            clean.chars().take(80).collect()
        }
    }

    /// Second line with content, without the Markdown marks (`Note.preview`).
    pub fn preview(&self, lang: Language) -> String {
        if self.secure {
            return tr("note_protected_content", lang, &[]);
        }
        let lines: Vec<&str> = self.text.split(is_newline).map(markdown::trim_blanks).filter(|l| !l.is_empty()).collect();
        let rest = strip_block_marker(lines.get(1).copied().unwrap_or(""));
        let clean: String = rest.chars().filter(|c| !matches!(c, '*' | '_' | '`' | '~')).collect();
        clean.chars().take(120).collect()
    }

    pub fn tags(&self) -> Vec<String> {
        if self.secure {
            vec![]
        } else {
            markdown::tags_in(&self.text)
        }
    }
}

/// `^(#+\s*|>\s*|[-*+]\s+(\[[ xX]\]\s*)?|\d+[.)]\s+)` removed from the start (once).
fn strip_block_marker(s: &str) -> &str {
    let ws = char::is_whitespace;
    if s.starts_with('#') {
        return s.trim_start_matches('#').trim_start_matches(ws);
    }
    if let Some(r) = s.strip_prefix('>') {
        return r.trim_start_matches(ws);
    }
    if s.starts_with(['-', '*', '+']) {
        let r = &s[1..];
        let after = r.trim_start_matches(ws);
        if after.len() != r.len() {
            let b = after.as_bytes();
            if b.len() >= 3 && b[0] == b'[' && matches!(b[1], b' ' | b'x' | b'X') && b[2] == b']' {
                return after[3..].trim_start_matches(ws);
            }
            return after;
        }
        return s;
    }
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let r = &s[digits..];
        if r.starts_with(['.', ')']) {
            let t = &r[1..];
            let after = t.trim_start_matches(ws);
            if after.len() != t.len() {
                return after;
            }
        }
    }
    s
}

// ---- password notes: "key: value" rows ----

/// One "key: value" row of a password note. The text of the note stays the title on the first line and one
/// field per line, so changing the type loses nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretField {
    pub key: String,
    pub value: String,
}

impl SecretField {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string() }
    }

    /// `SecretField.parse`: only the first `:` splits a line, so passwords with `:` stay whole.
    pub fn parse(text: &str) -> (String, Vec<SecretField>) {
        let mut lines = text.split('\n');
        let title = lines.next().unwrap_or("").to_string();
        let fields = lines
            .filter(|l| !markdown::trim_blanks(l).is_empty())
            .map(|line| match line.find(':') {
                None => SecretField { key: markdown::trim_blanks(line).to_string(), value: String::new() },
                Some(c) => {
                    let value = &line[c + 1..];
                    let value = value.strip_prefix(' ').unwrap_or(value);
                    SecretField { key: markdown::trim_blanks(&line[..c]).to_string(), value: value.to_string() }
                }
            })
            .collect();
        (title, fields)
    }

    /// `SecretField.format`: line breaks become spaces and `:` is dropped from names.
    pub fn format(title: &str, fields: &[SecretField]) -> String {
        let flat = |s: &str| s.replace('\n', " ");
        let mut rows = vec![flat(title)];
        for f in fields {
            let key = flat(&f.key).replace(':', "");
            rows.push(if f.value.is_empty() { format!("{key}:") } else { format!("{key}: {}", flat(&f.value)) });
        }
        rows.join("\n")
    }
}

const USERNAME_NAMES: [&str; 12] = ["usuário", "usuario", "user", "username", "login", "e-mail", "email", "benutzer", "用户", "账号", "conta", "account"];
const PASSWORD_NAMES: [&str; 12] = ["senha", "password", "passwort", "contraseña", "contrasena", "密码", "pass", "pin", "token", "chave", "key", "secret"];

/// `ClipboardManager.secretValue`: the password (or username) field by its name in any language; with no
/// known name, the password is the last filled field and the username the first (only when there are two).
pub fn secret_value(fields: &[SecretField], username: bool) -> Option<String> {
    let names: &[&str] = if username { &USERNAME_NAMES } else { &PASSWORD_NAMES };
    let folded: Vec<String> = names.iter().map(|n| fold(n)).collect();
    if let Some(f) = fields.iter().find(|f| {
        let k = fold(&f.key);
        folded.iter().any(|n| k.contains(n.as_str()))
    }) {
        return Some(f.value.clone());
    }
    let filled: Vec<&SecretField> = fields.iter().filter(|f| !f.value.is_empty()).collect();
    if username {
        if filled.len() > 1 {
            filled.first().map(|f| f.value.clone())
        } else {
            None
        }
    } else {
        filled.last().map(|f| f.value.clone())
    }
}

/// The notes of the clipboard "Passwords" filter (`ClipboardManager.passwordItems`): only password notes,
/// matched by **title** (the content stays in the keyring), newest first.
pub fn password_notes<'a>(notes: &'a [Note], query: &str, lang: Language) -> Vec<&'a Note> {
    let q = fold(markdown::trim_blanks(query));
    let mut out: Vec<&Note> = notes.iter().filter(|n| n.is_passwords() && (q.is_empty() || fold(&n.title(lang)).contains(&q))).collect();
    out.sort_by(|a, b| b.updated.partial_cmp(&a.updated).unwrap_or(std::cmp::Ordering::Equal));
    out
}

// ---- where the secrets live, and who may open them ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// No Secret Service on the bus (no GNOME Keyring or KWallet running).
    Unavailable(String),
    /// The keyring is locked and was not unlocked.
    Locked,
    Failed(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Unavailable(m) => write!(f, "no Secret Service: {m}"),
            VaultError::Locked => write!(f, "the keyring is locked"),
            VaultError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// The content of the protected notes (`NoteVault`): the Secret Service, with the attributes
/// `application=app.menew.Menew` and `note=<uuid>`.
pub trait Vault {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError>;
    fn write(&self, id: &str, label: &str, text: &str) -> Result<(), VaultError>;
    fn delete(&self, id: &str) -> Result<(), VaultError>;
}

/// A vault in memory (tests, and the case where there is no keyring and nothing may be protected).
#[derive(Default)]
pub struct MemoryVault {
    pub items: std::cell::RefCell<HashMap<String, String>>,
    /// Makes every call fail, to test what the store does without a keyring.
    pub broken: std::cell::Cell<bool>,
}

impl Vault for MemoryVault {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError> {
        if self.broken.get() {
            return Err(VaultError::Unavailable("test".into()));
        }
        Ok(self.items.borrow().get(id).cloned())
    }
    fn write(&self, id: &str, _label: &str, text: &str) -> Result<(), VaultError> {
        if self.broken.get() {
            return Err(VaultError::Unavailable("test".into()));
        }
        self.items.borrow_mut().insert(id.to_string(), text.to_string());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), VaultError> {
        if self.broken.get() {
            return Err(VaultError::Unavailable("test".into()));
        }
        self.items.borrow_mut().remove(id);
        Ok(())
    }
}

/// How the user was asked before a protected note opened (the Mac asks for Touch ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    /// polkit asked for the password of the user (`app.menew.Menew.unlock-note`, `auth_self`).
    Polkit,
    /// polkit could not ask (no policy installed, no authentication agent, no polkit): the keyring that
    /// was unlocked at login is the only protection, and the UI says so.
    KeyringOnly(String),
    /// The user said no, or closed the dialog.
    Denied,
}

pub trait Authenticator: Send + Sync {
    /// Blocks until the user answered (call it off the UI thread).
    fn authenticate(&self) -> Auth;
}

// ---- the store ----

pub fn notes_dir() -> PathBuf {
    crate::clipboard_db::data_dir().join("notes")
}

pub struct NotesStore {
    path: Option<PathBuf>,
    vault: Rc<dyn Vault>,
    pub notes: Vec<Note>,
    /// Protected notes opened in this session: the full text.
    unlocked: HashMap<String, String>,
    dirty: bool,
    dirty_secrets: HashSet<String>,
}

impl NotesStore {
    /// An in-memory store (tests, demos).
    pub fn in_memory(vault: Rc<dyn Vault>) -> Self {
        Self { path: None, vault, notes: vec![], unlocked: HashMap::new(), dirty: false, dirty_secrets: HashSet::new() }
    }

    /// Read `notes.json` in `dir`. A file that cannot be read is kept as `notes.json.corrupt` and never overwritten.
    pub fn load(dir: &Path, vault: Rc<dyn Vault>) -> Self {
        let path = dir.join("notes.json");
        let mut store = Self { path: Some(path.clone()), vault, notes: vec![], unlocked: HashMap::new(), dirty: false, dirty_secrets: HashSet::new() };
        if let Ok(data) = std::fs::read(&path) {
            match serde_json::from_slice::<Vec<Note>>(&data) {
                Ok(list) => store.notes = list,
                Err(e) => {
                    tracing_free_warn(&format!("notes.json could not be read ({e}); kept as notes.json.corrupt"));
                    let _ = std::fs::rename(&path, dir.join("notes.json.corrupt"));
                }
            }
        }
        store
    }

    pub fn load_default(vault: Rc<dyn Vault>) -> Self {
        let dir = notes_dir();
        let _ = crate::private_fs::create_private_dir(&dir);
        Self::load(&dir, vault)
    }

    pub fn vault(&self) -> &Rc<dyn Vault> {
        &self.vault
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

    /// `NotesStore.filtered`: by tag, then by text (case and accent insensitive), newest first.
    pub fn filtered(&self, query: &str, tag: Option<&str>) -> Vec<&Note> {
        let q = fold(markdown::trim_blanks(query));
        let mut out: Vec<&Note> = self
            .notes
            .iter()
            .filter(|n| tag.map(|t| n.tags().iter().any(|x| x == t)).unwrap_or(true))
            .filter(|n| q.is_empty() || fold(&n.text).contains(&q))
            .collect();
        out.sort_by(|a, b| b.updated.partial_cmp(&a.updated).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn note(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    /// The full text; of the protected notes, only after unlocking.
    pub fn text_of(&self, id: &str) -> String {
        match self.note(id) {
            None => String::new(),
            Some(n) if n.secure => self.unlocked.get(id).cloned().unwrap_or_default(),
            Some(n) => n.text.clone(),
        }
    }

    pub fn is_locked(&self, id: &str) -> bool {
        self.note(id).map(|n| n.secure).unwrap_or(false) && !self.unlocked.contains_key(id)
    }

    pub fn create(&mut self, text: &str) -> String {
        let n = Note::new(text);
        let id = n.id.clone();
        self.notes.insert(0, n);
        self.dirty = true;
        id
    }

    /// A new password note: already protected, with a username and a password row to fill in. If there is no
    /// keyring the note stays a normal one and the error says why (nothing secret is ever written to the file).
    pub fn create_passwords(&mut self, lang: Language) -> Result<String, VaultError> {
        let text = SecretField::format("", &[SecretField::new(&tr("pw_username", lang, &[]), ""), SecretField::new(&tr("pw_password", lang, &[]), "")]);
        let id = self.create(&text);
        if let Err(e) = self.set_kind(&id, Kind::Passwords) {
            self.notes.retain(|n| n.id != id);
            return Err(e);
        }
        Ok(id)
    }

    fn first_line(text: &str) -> String {
        text.split('\n').next().unwrap_or("").to_string()
    }

    /// Change the text in memory; `flush` writes it (the UI calls it a moment after typing stops).
    pub fn set_text(&mut self, id: &str, text: &str) {
        let Some(i) = self.notes.iter().position(|n| n.id == id) else { return };
        if self.notes[i].secure {
            if !self.unlocked.contains_key(id) {
                return;
            }
            self.unlocked.insert(id.to_string(), text.to_string());
            self.notes[i].text = Self::first_line(text);
            self.dirty_secrets.insert(id.to_string());
        } else {
            self.notes[i].text = text.to_string();
        }
        self.notes[i].updated = now_ref();
        self.dirty = true;
    }

    /// Passwords are always protected; going back to text keeps the protection.
    pub fn set_kind(&mut self, id: &str, kind: Kind) -> Result<(), VaultError> {
        let Some(i) = self.notes.iter().position(|n| n.id == id) else { return Ok(()) };
        if kind == Kind::Passwords && !self.notes[i].secure {
            self.set_secure(id, true)?;
        }
        if let Some(i) = self.notes.iter().position(|n| n.id == id) {
            self.notes[i].kind = if kind == Kind::Text { None } else { Some(kind) };
        }
        self.dirty = true;
        self.flush()
    }

    pub fn delete(&mut self, id: &str) -> Result<(), VaultError> {
        let was_secure = self.note(id).map(|n| n.secure).unwrap_or(false);
        self.notes.retain(|n| n.id != id);
        self.unlocked.remove(id);
        self.dirty_secrets.remove(id);
        self.dirty = true;
        let gone = if was_secure { self.vault.delete(id) } else { Ok(()) };
        self.flush()?;
        gone
    }

    /// Protect (the text goes to the keyring and only the title stays in the file) or remove the protection (the
    /// text comes back to the file, which needs the note to be open). Protecting writes to the keyring **first**:
    /// if that fails the note stays normal and nothing changes.
    pub fn set_secure(&mut self, id: &str, secure: bool) -> Result<(), VaultError> {
        let Some(i) = self.notes.iter().position(|n| n.id == id) else { return Ok(()) };
        if self.notes[i].secure == secure {
            return Ok(());
        }
        if secure {
            let full = self.notes[i].text.clone();
            self.vault.write(id, "Menew note", &full)?;
            self.notes[i].secure = true;
            self.notes[i].text = Self::first_line(&full);
            self.unlocked.insert(id.to_string(), full);
        } else {
            let Some(full) = self.unlocked.get(id).cloned() else { return Ok(()) };
            self.notes[i].secure = false;
            self.notes[i].text = full;
            self.unlocked.remove(id);
            self.dirty_secrets.remove(id);
            // the file gets the text first, then the keyring item goes
            self.dirty = true;
            self.flush()?;
            return self.vault.delete(id);
        }
        self.dirty = true;
        self.flush()
    }

    /// Read the text of a protected note from the keyring (the caller asked the user first, see [`Authenticator`]).
    pub fn unlock(&mut self, id: &str) -> Result<bool, VaultError> {
        if self.unlocked.contains_key(id) {
            return Ok(true);
        }
        let Some(n) = self.note(id) else { return Ok(false) };
        let fallback = n.text.clone();
        let full = self.vault.read(id)?.unwrap_or(fallback);
        self.unlocked.insert(id.to_string(), full);
        Ok(true)
    }

    pub fn lock(&mut self, id: &str) {
        self.unlocked.remove(id);
    }

    pub fn lock_all(&mut self) {
        self.unlocked.clear();
    }

    /// Write what changed: `notes.json` (atomically) and the keyring items of the edited protected notes. A
    /// keyring that fails keeps the text in memory and in the dirty set, and the error comes back.
    pub fn flush(&mut self) -> Result<(), VaultError> {
        let mut result = Ok(());
        let pending: Vec<String> = self.dirty_secrets.iter().cloned().collect();
        for id in pending {
            match self.unlocked.get(&id) {
                Some(text) => match self.vault.write(&id, "Menew note", text) {
                    Ok(()) => {
                        self.dirty_secrets.remove(&id);
                    }
                    Err(e) => result = Err(e),
                },
                None => {
                    self.dirty_secrets.remove(&id);
                }
            }
        }
        if self.dirty {
            if let Some(path) = &self.path {
                if let Ok(data) = serde_json::to_vec(&self.notes) {
                    if crate::private_fs::write_private(path, &data).is_ok() {
                        self.dirty = false;
                    }
                }
            } else {
                self.dirty = false;
            }
        }
        result
    }

    pub fn has_unsaved(&self) -> bool {
        self.dirty || !self.dirty_secrets.is_empty()
    }
}

/// `menew-core` has no logging crate; the daemon logs what the store tells it. This keeps a line on stderr.
fn tracing_free_warn(msg: &str) {
    eprintln!("menew-core: {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (NotesStore, Rc<MemoryVault>) {
        let v = Rc::new(MemoryVault::default());
        (NotesStore::in_memory(v.clone()), v)
    }

    #[test]
    fn protecting_moves_the_text_to_the_vault_and_keeps_only_the_title() {
        let (mut s, v) = store();
        let id = s.create("Bank\nuser: ana\nsecret-xyz");
        s.set_secure(&id, true).unwrap();
        assert_eq!(s.note(&id).unwrap().text, "Bank");
        assert_eq!(v.items.borrow().get(&id).unwrap(), "Bank\nuser: ana\nsecret-xyz");
        s.lock(&id);
        assert!(s.is_locked(&id));
        assert_eq!(s.text_of(&id), "");
        assert!(s.unlock(&id).unwrap());
        assert_eq!(s.text_of(&id), "Bank\nuser: ana\nsecret-xyz");
    }

    #[test]
    fn protecting_without_a_keyring_changes_nothing() {
        let (mut s, v) = store();
        v.broken.set(true);
        let id = s.create("Bank\nsecret-xyz");
        assert!(s.set_secure(&id, true).is_err());
        assert!(!s.note(&id).unwrap().secure);
        assert_eq!(s.note(&id).unwrap().text, "Bank\nsecret-xyz");
        assert!(s.create_passwords(Language::En).is_err());
        assert_eq!(s.notes.len(), 1);
    }

    #[test]
    fn removing_the_protection_brings_the_text_back_and_deletes_the_item() {
        let (mut s, v) = store();
        let id = s.create("T\nbody");
        s.set_secure(&id, true).unwrap();
        s.set_secure(&id, false).unwrap();
        assert_eq!(s.note(&id).unwrap().text, "T\nbody");
        assert!(v.items.borrow().is_empty());
    }

    #[test]
    fn editing_a_protected_note_goes_to_the_vault_on_flush() {
        let (mut s, v) = store();
        let id = s.create("T");
        s.set_secure(&id, true).unwrap();
        s.set_text(&id, "T2\nnew-secret");
        assert_eq!(s.note(&id).unwrap().text, "T2");
        assert_eq!(v.items.borrow().get(&id).unwrap(), "T");
        s.flush().unwrap();
        assert_eq!(v.items.borrow().get(&id).unwrap(), "T2\nnew-secret");
        // locked notes cannot be edited
        s.lock(&id);
        s.set_text(&id, "hacked");
        assert_eq!(s.note(&id).unwrap().text, "T2");
    }

    #[test]
    fn a_failing_keyring_keeps_the_edit_in_memory_and_reports() {
        let (mut s, v) = store();
        let id = s.create("T");
        s.set_secure(&id, true).unwrap();
        v.broken.set(true);
        s.set_text(&id, "T\nsecret-2");
        assert!(s.flush().is_err());
        assert!(s.has_unsaved());
        v.broken.set(false);
        s.flush().unwrap();
        assert_eq!(v.items.borrow().get(&id).unwrap(), "T\nsecret-2");
    }

    #[test]
    fn deleting_removes_the_vault_item() {
        let (mut s, v) = store();
        let id = s.create_passwords(Language::En).unwrap();
        assert!(v.items.borrow().contains_key(&id));
        s.delete(&id).unwrap();
        assert!(v.items.borrow().is_empty() && s.notes.is_empty());
    }

    #[test]
    fn the_file_never_holds_the_secret_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let v = Rc::new(MemoryVault::default());
        let mut s = NotesStore::load(dir.path(), v.clone());
        let id = s.create("Wi-Fi");
        s.set_text(&id, "Wi-Fi\nPassword: hunter2-unique-value\n#home");
        s.set_secure(&id, true).unwrap();
        s.flush().unwrap();
        let file = std::fs::read_to_string(dir.path().join("notes.json")).unwrap();
        assert!(!file.contains("hunter2-unique-value"), "{file}");
        let s2 = NotesStore::load(dir.path(), v);
        assert_eq!(s2.notes.len(), 1);
        assert!(s2.notes[0].secure);
        assert_eq!(s2.notes[0].text, "Wi-Fi");
    }

    #[test]
    fn a_corrupt_file_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.json"), "{not json").unwrap();
        let mut s = NotesStore::load(dir.path(), Rc::new(MemoryVault::default()));
        assert!(s.notes.is_empty());
        s.create("x");
        s.flush().unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("notes.json.corrupt")).unwrap(), "{not json");
    }

    #[test]
    fn clear_delay_is_30_seconds_unless_the_test_hook_is_on() {
        assert_eq!(secret_clear_secs(false, Some("3")), 30);
        assert_eq!(secret_clear_secs(true, Some("3")), 3);
        assert_eq!(secret_clear_secs(true, None), 30);
        assert_eq!(secret_clear_secs(true, Some("0")), 30);
    }
}
