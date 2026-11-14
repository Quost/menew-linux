use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: String,
    pub text_content: String,
    pub char_count: usize,
    pub is_pinned: bool,
    pub created_at: i64,
}

/// Escape `\`, `%` and `_` so a user query is matched literally by `LIKE ? ESCAPE '\\'`.
fn escape_like(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for c in query.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[derive(Clone)]
pub struct ClipboardDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ClipboardDatabase {
    pub fn open_default() -> Result<Self> {
        let db_path = Self::default_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self::open_file(&db_path)
    }

    pub fn default_db_path() -> PathBuf {
        // Test hook: point the daemon at a temporary database
        if let Some(p) = std::env::var_os("MENEW_DB_PATH") {
            return PathBuf::from(p);
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("menew")
            .join("clipboard.db")
    }

    pub fn open_file(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
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
        Ok(())
    }

    /// Save text to clipboard history. Deduplicates with latest entry if identical.
    pub fn save_text(&self, text: &str) -> Result<Option<i64>> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();

        // Global dedupe by identical content: bump the existing row (keeping is_pinned)
        // instead of inserting a twin. Uses idx_clipboard_text, so no table scan.
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clipboard_entries WHERE text_content = ? ORDER BY id DESC LIMIT 1",
                params![text],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            conn.execute(
                "UPDATE clipboard_entries SET created_at = ? WHERE id = ?",
                params![now, id],
            )?;
            return Ok(Some(id));
        }

        let char_count = text.chars().count();
        conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, char_count, is_pinned, created_at)
             VALUES ('text/plain', ?, ?, 0, ?)",
            params![text, char_count as i64, now],
        )?;

        let id = conn.last_insert_rowid();
        Ok(Some(id))
    }

    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_type, text_content, char_count, is_pinned, created_at
             FROM clipboard_entries
             ORDER BY is_pinned DESC, created_at DESC, id DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                char_count: row.get::<_, i64>(3)? as usize,
                is_pinned: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let pattern = format!("%{}%", escape_like(query));
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_type, text_content, char_count, is_pinned, created_at
             FROM clipboard_entries
             WHERE text_content LIKE ? ESCAPE '\\'
             ORDER BY is_pinned DESC, created_at DESC, id DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                char_count: row.get::<_, i64>(3)? as usize,
                is_pinned: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    pub fn toggle_pinned(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let is_pinned: i32 = conn.query_row(
            "SELECT is_pinned FROM clipboard_entries WHERE id = ?",
            params![id],
            |r| r.get(0),
        )?;

        let new_state = if is_pinned == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE clipboard_entries SET is_pinned = ? WHERE id = ?",
            params![new_state, id],
        )?;

        Ok(new_state != 0)
    }

    pub fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries WHERE id = ?", params![id])?;
        Ok(())
    }

    pub fn clear_unpinned(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM clipboard_entries WHERE is_pinned = 0", [])?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM clipboard_entries WHERE text_content = 'x'",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(plan.contains("idx_clipboard_text"), "{plan}");
    }

    #[test]
    fn search_treats_percent_and_underscore_literally() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        for t in ["500", "50% off", "axb", "a_b", "back\\slash"] {
            db.save_text(t).unwrap();
        }
        let texts = |q: &str| -> Vec<String> {
            db.search(q, 10).unwrap().into_iter().map(|e| e.text_content).collect()
        };
        assert_eq!(texts("50%"), vec!["50% off".to_string()]);
        assert!(!texts("50%").contains(&"500".to_string()));
        assert_eq!(texts("a_b"), vec!["a_b".to_string()]);
        assert!(!texts("a_b").contains(&"axb".to_string()));
        assert_eq!(texts("\\"), vec!["back\\slash".to_string()]);
        // plain substring search still works
        assert_eq!(texts("off"), vec!["50% off".to_string()]);
    }
}
