use crate::clip_item::ClipFilter;
use crate::search::fold;
use rusqlite::{params, Connection, OptionalExtension, Result, Row};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What an entry holds, like `ClipItem.Kind` on the Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClipKind {
    #[default]
    Text,
    Image,
    File,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
            ClipKind::File => "file",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "image" => ClipKind::Image,
            "file" => ClipKind::File,
            _ => ClipKind::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: String,
    /// The text; for a file the paths, one per line; for an image "Imagem WxH" (the Mac keeps it in Portuguese).
    pub text_content: String,
    pub char_count: usize,
    pub is_pinned: bool,
    pub created_at: i64,
    pub kind: ClipKind,
    /// File name of the image inside the images folder.
    pub image_file: Option<String>,
    /// Text read from the image. `None` = not read yet, empty = read, no text.
    pub ocr_text: Option<String>,
    /// The app the text was copied from, when known (GNOME only).
    pub source_app: Option<String>,
}

impl ClipboardEntry {
    /// A plain text entry (tests and previews).
    pub fn text_entry(id: i64, text: &str, is_pinned: bool, created_at: i64) -> Self {
        Self {
            id,
            content_type: "text/plain".into(),
            text_content: text.to_string(),
            char_count: text.chars().count(),
            is_pinned,
            created_at,
            kind: ClipKind::Text,
            image_file: None,
            ocr_text: None,
            source_app: None,
        }
    }
}

const COLUMNS: &str = "id, content_type, text_content, char_count, is_pinned, created_at, kind, image_file, ocr_text, source_app";

fn read_entry(row: &Row) -> Result<ClipboardEntry> {
    Ok(ClipboardEntry {
        id: row.get(0)?,
        content_type: row.get(1)?,
        text_content: row.get(2)?,
        char_count: row.get::<_, i64>(3)? as usize,
        is_pinned: row.get::<_, i32>(4)? != 0,
        created_at: row.get(5)?,
        kind: ClipKind::from_db(&row.get::<_, String>(6)?),
        image_file: row.get(7)?,
        ocr_text: row.get(8)?,
        source_app: row.get(9)?,
    })
}

/// The data folder of Menew (`~/.local/share/menew`).
pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("menew")
}

#[derive(Clone)]
pub struct ClipboardDatabase {
    conn: Arc<Mutex<Connection>>,
    images_dir: Option<PathBuf>,
}

impl ClipboardDatabase {
    pub fn open_default() -> Result<Self> {
        let db_path = Self::default_db_path();
        if let Some(parent) = db_path.parent() {
            // the test hook can point at any folder (even /tmp): only the real data folder is made private
            if std::env::var_os("MENEW_DB_PATH").is_some() {
                let _ = std::fs::create_dir_all(parent);
            } else {
                let _ = crate::private_fs::create_private_dir(parent);
            }
        }
        // created 0600 first: SQLite gives its -wal and -shm files the mode of the database
        let _ = crate::private_fs::ensure_private_file(&db_path);
        // test hook: the images go next to the temporary database
        let images = if std::env::var_os("MENEW_DB_PATH").is_some() {
            db_path.with_extension("images")
        } else {
            data_dir().join("clipboard")
        };
        Ok(Self::open_file(&db_path)?.with_images_dir(images))
    }

    pub fn default_db_path() -> PathBuf {
        // Test hook: point the daemon at a temporary database
        if let Some(p) = std::env::var_os("MENEW_DB_PATH") {
            return PathBuf::from(p);
        }
        data_dir().join("clipboard.db")
    }

    pub fn open_file(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Arc::new(Mutex::new(conn)), images_dir: None };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn: Arc::new(Mutex::new(conn)), images_dir: None };
        db.init_schema()?;
        Ok(db)
    }

    /// The folder where the images are kept (created when the first image is saved).
    pub fn with_images_dir(mut self, dir: PathBuf) -> Self {
        self.images_dir = Some(dir);
        self
    }

    pub fn images_dir(&self) -> Option<&Path> {
        self.images_dir.as_deref()
    }

    /// The file of an image entry.
    pub fn image_path(&self, entry: &ClipboardEntry) -> Option<PathBuf> {
        match (&self.images_dir, &entry.image_file) {
            (Some(dir), Some(f)) => Some(dir.join(f)),
            _ => None,
        }
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text/plain',
                text_content TEXT NOT NULL,
                char_count INTEGER NOT NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_clipboard_created
            ON clipboard_entries(created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_clipboard_text
            ON clipboard_entries(text_content);
            "#,
        )?;
        // databases of the first alphas have no image, file or OCR columns
        let mut have: Vec<String> = vec![];
        {
            let mut stmt = conn.prepare("PRAGMA table_info(clipboard_entries)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            for r in rows {
                have.push(r?);
            }
        }
        for (name, ddl) in [
            ("kind", "ALTER TABLE clipboard_entries ADD COLUMN kind TEXT NOT NULL DEFAULT 'text'"),
            ("image_file", "ALTER TABLE clipboard_entries ADD COLUMN image_file TEXT"),
            ("ocr_text", "ALTER TABLE clipboard_entries ADD COLUMN ocr_text TEXT"),
            ("source_app", "ALTER TABLE clipboard_entries ADD COLUMN source_app TEXT"),
        ] {
            if !have.iter().any(|c| c == name) {
                conn.execute(ddl, [])?;
            }
        }
        Ok(())
    }

    /// Save text to clipboard history. Deduplicates with latest entry if identical.
    pub fn save_text(&self, text: &str) -> Result<Option<i64>> {
        self.save_text_from(text, None)
    }

    /// Like [`save_text`](Self::save_text), remembering the app the text came from.
    pub fn save_text_from(&self, text: &str, source_app: Option<&str>) -> Result<Option<i64>> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let char_count = text.chars().count();
        self.save_item(ClipKind::Text, text, char_count, None, source_app)
    }

    /// Paths copied from a file manager (one per line).
    pub fn save_files(&self, paths: &str, source_app: Option<&str>) -> Result<Option<i64>> {
        if paths.trim().is_empty() {
            return Ok(None);
        }
        self.save_item(ClipKind::File, paths, paths.chars().count(), None, source_app)
    }

    /// An image already written to the images folder under `file`. A second copy of the same image
    /// (same name, which comes from its hash) moves the existing entry to the top.
    pub fn save_image(&self, file: &str, width: u32, height: u32, source_app: Option<&str>) -> Result<Option<i64>> {
        let label = if width == 0 || height == 0 { "Imagem".to_string() } else { format!("Imagem {width}\u{d7}{height}") };
        self.save_item(ClipKind::Image, &label, label.chars().count(), Some(file), source_app)
    }

    /// Insert, or when the same content is already there (same text for text and files, same file for
    /// images) bump the existing row to now, keeping its id and pin.
    fn save_item(&self, kind: ClipKind, text: &str, char_count: usize, image_file: Option<&str>, source_app: Option<&str>) -> Result<Option<i64>> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        let existing: Option<i64> = match image_file {
            Some(f) => conn
                .query_row("SELECT id FROM clipboard_entries WHERE kind = 'image' AND image_file = ? ORDER BY id DESC LIMIT 1", params![f], |r| r.get(0))
                .optional()?,
            // uses idx_clipboard_text, so no table scan
            None => conn
                .query_row(
                    "SELECT id FROM clipboard_entries WHERE text_content = ? AND kind = ? ORDER BY id DESC LIMIT 1",
                    params![text, kind.as_str()],
                    |r| r.get(0),
                )
                .optional()?,
        };
        if let Some(id) = existing {
            conn.execute("UPDATE clipboard_entries SET created_at = ?, source_app = COALESCE(?, source_app) WHERE id = ?", params![now, source_app, id])?;
            return Ok(Some(id));
        }
        let content_type = match kind {
            ClipKind::Text => "text/plain",
            ClipKind::Image => "image",
            ClipKind::File => "text/uri-list",
        };
        conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, char_count, is_pinned, created_at, kind, image_file, source_app)
             VALUES (?, ?, ?, 0, ?, ?, ?, ?)",
            params![content_type, text, char_count as i64, now, kind.as_str(), image_file, source_app],
        )?;
        Ok(Some(conn.last_insert_rowid()))
    }

    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM clipboard_entries
             ORDER BY is_pinned DESC, created_at DESC, id DESC
             LIMIT ?"
        ))?;
        let rows = stmt.query_map(params![limit as i64], read_entry)?;
        rows.collect()
    }

    pub fn get(&self, id: i64) -> Result<Option<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(&format!("SELECT {COLUMNS} FROM clipboard_entries WHERE id = ?"), params![id], read_entry).optional()
    }

    /// The most recently copied entry, whether or not it is pinned (the palette applies text
    /// transforms to it when the clipboard cannot be read directly).
    pub fn get_newest(&self) -> Result<Option<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM clipboard_entries
             ORDER BY created_at DESC, id DESC
             LIMIT 1"
        ))?;
        let mut rows = stmt.query_map([], read_entry)?;
        rows.next().transpose()
    }

    /// The newest entry that holds text (what the text transforms and smart actions apply to).
    pub fn get_newest_text(&self) -> Result<Option<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            &format!("SELECT {COLUMNS} FROM clipboard_entries WHERE kind = 'text' ORDER BY created_at DESC, id DESC LIMIT 1"),
            [],
            read_entry,
        )
        .optional()
    }

    /// Search like the Mac (`ClipboardManager.filtered`): case and accent insensitive, in the text, the
    /// source app and the text read from images, pinned first, then newest first.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ClipboardEntry>> {
        self.filtered(ClipFilter::All, query, limit)
    }

    /// The entries of a filter that match the query, in display order.
    pub fn filtered(&self, filter: ClipFilter, query: &str, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let q = fold(query.trim());
        let all = self.get_recent(i64::MAX as usize)?;
        Ok(all
            .into_iter()
            .filter(|e| filter.matches(e))
            .filter(|e| {
                q.is_empty()
                    || [Some(e.text_content.as_str()), e.source_app.as_deref(), e.ocr_text.as_deref()]
                        .into_iter()
                        .flatten()
                        .any(|t| fold(t).contains(&q))
            })
            .take(limit)
            .collect())
    }

    pub fn toggle_pinned(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let is_pinned: i32 = conn.query_row("SELECT is_pinned FROM clipboard_entries WHERE id = ?", params![id], |r| r.get(0))?;

        let new_state = if is_pinned == 0 { 1 } else { 0 };
        conn.execute("UPDATE clipboard_entries SET is_pinned = ? WHERE id = ?", params![new_state, id])?;

        Ok(new_state != 0)
    }

    /// Set the time of an entry (milliseconds); the demo data of `--snapshot` uses it for varied ages.
    pub fn set_created_at(&self, id: i64, created_at: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE clipboard_entries SET created_at = ? WHERE id = ?", params![created_at, id])?;
        Ok(())
    }

    /// Move an entry to the top, as if it had just been copied.
    pub fn touch(&self, id: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE clipboard_entries SET created_at = ? WHERE id = ?", params![now, id])?;
        Ok(())
    }

    pub fn set_ocr_text(&self, id: i64, text: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE clipboard_entries SET ocr_text = ? WHERE id = ?", params![text, id])?;
        Ok(())
    }

    /// Images whose text was not read yet, newest first.
    pub fn images_without_text(&self) -> Result<Vec<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM clipboard_entries WHERE kind = 'image' AND ocr_text IS NULL AND image_file IS NOT NULL
             ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map([], read_entry)?;
        rows.collect()
    }

    pub fn delete_entry(&self, id: i64) -> Result<()> {
        let file: Option<String> = {
            let conn = self.conn.lock().unwrap();
            let f = conn.query_row("SELECT image_file FROM clipboard_entries WHERE id = ?", params![id], |r| r.get(0)).optional()?.flatten();
            conn.execute("DELETE FROM clipboard_entries WHERE id = ?", params![id])?;
            f
        };
        self.remove_orphan_images(file.into_iter().collect());
        Ok(())
    }

    /// Delete everything that is not pinned, and the image files nobody uses any more.
    pub fn clear_unpinned(&self) -> Result<usize> {
        let (deleted, files) = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT image_file FROM clipboard_entries WHERE is_pinned = 0 AND image_file IS NOT NULL")?;
            let files: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_>>()?;
            drop(stmt);
            (conn.execute("DELETE FROM clipboard_entries WHERE is_pinned = 0", [])?, files)
        };
        self.remove_orphan_images(files);
        Ok(deleted)
    }

    /// The limits of the Mac (`ClipboardManager.prune`): at most `max_items` (at least 20) unpinned
    /// entries, and none older than `days` days when `days` > 0. Pinned entries never go. Returns how many were removed.
    pub fn prune(&self, max_items: usize, days: u32) -> Result<usize> {
        let max_items = max_items.max(crate::clip_item::MIN_MAX_ITEMS);
        let cutoff = (days > 0).then(|| chrono::Utc::now().timestamp_millis() - days as i64 * 86_400_000);
        let (removed, files) = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id, image_file, created_at FROM clipboard_entries WHERE is_pinned = 0 ORDER BY created_at DESC, id DESC")?;
            let rows: Vec<(i64, Option<String>, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_>>()?;
            drop(stmt);
            let mut ids = vec![];
            let mut files = vec![];
            for (i, (id, file, created)) in rows.into_iter().enumerate() {
                if i + 1 > max_items || cutoff.map(|c| created < c).unwrap_or(false) {
                    ids.push(id);
                    files.extend(file);
                }
            }
            for id in &ids {
                conn.execute("DELETE FROM clipboard_entries WHERE id = ?", params![id])?;
            }
            (ids.len(), files)
        };
        self.remove_orphan_images(files);
        Ok(removed)
    }

    /// Forget entries whose image file is gone (the Mac does this when it starts).
    pub fn drop_missing_images(&self) -> Result<usize> {
        let Some(dir) = self.images_dir.clone() else { return Ok(0) };
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, image_file FROM clipboard_entries WHERE image_file IS NOT NULL")?;
        let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_>>()?;
        drop(stmt);
        let mut n = 0;
        for (id, f) in rows {
            if !dir.join(&f).exists() {
                conn.execute("DELETE FROM clipboard_entries WHERE id = ?", params![id])?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Remove the files of images that no entry points to any more.
    fn remove_orphan_images(&self, files: Vec<String>) {
        let Some(dir) = &self.images_dir else { return };
        let conn = self.conn.lock().unwrap();
        for f in files {
            let used: i64 = conn.query_row("SELECT COUNT(*) FROM clipboard_entries WHERE image_file = ?", params![f], |r| r.get(0)).unwrap_or(1);
            if used == 0 && !f.contains('/') && !f.contains("..") {
                let _ = std::fs::remove_file(dir.join(&f));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_ignores_pins() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        assert_eq!(db.get_newest().unwrap(), None);
        let old = db.save_text("old").unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.save_text("new").unwrap();
        db.toggle_pinned(old).unwrap();
        // get_recent lists the pinned one first, get_newest the last one copied
        assert_eq!(db.get_recent(1).unwrap()[0].text_content, "old");
        assert_eq!(db.get_newest().unwrap().unwrap().text_content, "new");
    }

    #[test]
    fn test_clipboard_db_crud() {
        let db = ClipboardDatabase::open_in_memory().expect("in-memory db open failed");

        let id1 = db.save_text("Hello World").expect("save text failed");
        assert!(id1.is_some());

        // Test duplicate detection
        let id2 = db.save_text("Hello World").expect("save text failed");
        assert_eq!(id1, id2);

        let id3 = db.save_text("Second Entry").expect("save text failed");
        assert_ne!(id1, id3);

        let recent = db.get_recent(10).expect("get_recent failed");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text_content, "Second Entry");

        // Test search
        let results = db.search("Hello", 10).expect("search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text_content, "Hello World");

        // Test pinning
        let is_pinned = db.toggle_pinned(id1.unwrap()).expect("toggle pin failed");
        assert!(is_pinned);

        // Test clear unpinned
        let deleted = db.clear_unpinned().expect("clear unpinned failed");
        assert_eq!(deleted, 1);

        let remaining = db.get_recent(10).expect("get_recent failed");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].text_content, "Hello World");
    }

    #[test]
    fn saving_a_b_a_leaves_two_rows_with_a_most_recent() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let a = db.save_text("A").unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = db.save_text("B").unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let a2 = db.save_text("A").unwrap().unwrap();
        assert_eq!(a, a2, "same row is reused");
        assert_ne!(a, b);
        let rows = db.get_recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text_content, "A");
        assert_eq!(rows[1].text_content, "B");
    }

    #[test]
    fn recopying_a_pinned_entry_keeps_one_pinned_row() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let a = db.save_text("keep me").unwrap().unwrap();
        db.toggle_pinned(a).unwrap();
        db.save_text("other").unwrap();
        let again = db.save_text("keep me").unwrap().unwrap();
        assert_eq!(a, again);
        let rows = db.get_recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text_content, "keep me");
        assert!(rows[0].is_pinned);
        assert!(!rows[1].is_pinned);
    }

    #[test]
    fn text_lookup_uses_the_index() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let conn = db.conn.lock().unwrap();
        let plan: String = conn
            .query_row("EXPLAIN QUERY PLAN SELECT id FROM clipboard_entries WHERE text_content = 'x' AND kind = 'text'", [], |r| r.get(3))
            .unwrap();
        assert!(plan.contains("idx_clipboard_text"), "{plan}");
    }

    #[test]
    fn search_treats_percent_and_underscore_literally() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        for t in ["500", "50% off", "axb", "a_b", "back\\slash"] {
            db.save_text(t).unwrap();
        }
        let texts = |q: &str| -> Vec<String> { db.search(q, 10).unwrap().into_iter().map(|e| e.text_content).collect() };
        assert_eq!(texts("50%"), vec!["50% off".to_string()]);
        assert!(!texts("50%").contains(&"500".to_string()));
        assert_eq!(texts("a_b"), vec!["a_b".to_string()]);
        assert!(!texts("a_b").contains(&"axb".to_string()));
        assert_eq!(texts("\\"), vec!["back\\slash".to_string()]);
        // plain substring search still works
        assert_eq!(texts("off"), vec!["50% off".to_string()]);
    }

    #[test]
    fn search_ignores_case_and_accents_like_the_mac() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        db.save_text("Reuni\u{e3}o de alinhamento").unwrap();
        assert_eq!(db.search("REUNIAO", 10).unwrap().len(), 1);
        assert_eq!(db.search("alinh", 10).unwrap().len(), 1);
    }

    #[test]
    fn images_files_and_text_dedupe_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        let db = ClipboardDatabase::open_in_memory().unwrap().with_images_dir(dir.path().to_path_buf());
        let a = db.save_image("aaaa.png", 640, 480, None).unwrap().unwrap();
        let again = db.save_image("aaaa.png", 640, 480, Some("Firefox")).unwrap().unwrap();
        assert_eq!(a, again, "same image file is the same entry");
        let b = db.save_image("bbbb.png", 640, 480, None).unwrap().unwrap();
        assert_ne!(a, b, "same size, different image");
        // a text equal to a file's paths is another entry
        let f = db.save_files("/tmp/a.txt", None).unwrap().unwrap();
        let t = db.save_text("/tmp/a.txt").unwrap().unwrap();
        assert_ne!(f, t);
        assert_eq!(db.get(a).unwrap().unwrap().source_app.as_deref(), Some("Firefox"));
        assert_eq!(db.get(a).unwrap().unwrap().text_content, "Imagem 640\u{d7}480");
    }

    #[test]
    fn deleting_and_clearing_remove_the_image_files() {
        let dir = tempfile::tempdir().unwrap();
        let db = ClipboardDatabase::open_in_memory().unwrap().with_images_dir(dir.path().to_path_buf());
        for f in ["one.png", "two.png", "three.png"] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        let one = db.save_image("one.png", 1, 1, None).unwrap().unwrap();
        let two = db.save_image("two.png", 1, 1, None).unwrap().unwrap();
        db.save_image("three.png", 1, 1, None).unwrap();
        db.toggle_pinned(two).unwrap();
        db.delete_entry(one).unwrap();
        assert!(!dir.path().join("one.png").exists());
        assert_eq!(db.clear_unpinned().unwrap(), 1);
        assert!(!dir.path().join("three.png").exists());
        assert!(dir.path().join("two.png").exists(), "the pinned image stays");
    }

    #[test]
    fn prune_keeps_pinned_and_applies_the_limits() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let pinned = db.save_text("pinned").unwrap().unwrap();
        db.toggle_pinned(pinned).unwrap();
        for i in 0..30 {
            db.save_text(&format!("item {i}")).unwrap();
        }
        // the minimum is 20 unpinned, whatever is asked
        assert_eq!(db.prune(5, 0).unwrap(), 10);
        let rows = db.get_recent(100).unwrap();
        assert_eq!(rows.len(), 21);
        assert!(rows.iter().any(|r| r.id == pinned));
        // nothing is old enough
        assert_eq!(db.prune(300, 7).unwrap(), 0);
    }

    #[test]
    fn text_read_from_images_is_searched() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let id = db.save_image("x.png", 10, 10, None).unwrap().unwrap();
        assert_eq!(db.images_without_text().unwrap().len(), 1);
        assert!(db.search("invoice", 10).unwrap().is_empty());
        db.set_ocr_text(id, "Invoice 2026").unwrap();
        assert_eq!(db.search("invoice", 10).unwrap().len(), 1);
        assert!(db.images_without_text().unwrap().is_empty());
        // read, no text: stays read
        let id2 = db.save_image("y.png", 10, 10, None).unwrap().unwrap();
        db.set_ocr_text(id2, "").unwrap();
        assert!(db.images_without_text().unwrap().is_empty());
    }

    #[test]
    fn an_old_database_gets_the_new_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE clipboard_entries (id INTEGER PRIMARY KEY AUTOINCREMENT, content_type TEXT NOT NULL DEFAULT 'text/plain',
                 text_content TEXT NOT NULL, char_count INTEGER NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);
                 INSERT INTO clipboard_entries (text_content, char_count, created_at) VALUES ('legacy', 6, 1);",
            )
            .unwrap();
        }
        let db = ClipboardDatabase::open_file(&path).unwrap();
        let rows = db.get_recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, ClipKind::Text);
        assert_eq!(rows[0].image_file, None);
    }
}
