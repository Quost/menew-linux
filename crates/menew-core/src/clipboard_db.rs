use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection, Result};
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

        // Check latest entry to prevent duplicate spam
        let mut stmt = conn.prepare(
            "SELECT id, text_content FROM clipboard_entries ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let last_id: i64 = row.get(0)?;
            let last_text: String = row.get(1)?;
            if last_text == text {
                // Just update the timestamp
                conn.execute(
                    "UPDATE clipboard_entries SET created_at = ? WHERE id = ?",
                    params![now, last_id],
                )?;
                return Ok(Some(last_id));
            }
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
        let pattern = format!("%{}%", query);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_type, text_content, char_count, is_pinned, created_at
             FROM clipboard_entries
             WHERE text_content LIKE ?
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
}
