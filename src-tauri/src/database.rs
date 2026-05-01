use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub content_type: String, // "text" | "url" | "code" | "file" | "image"
    pub content: String,
    pub preview: String,
    pub hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_data_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&app_data_dir).ok();
        let db_path = app_data_dir.join("yeet.db");
        let conn = Connection::open(db_path)?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS clipboard_items (
                id TEXT PRIMARY KEY,
                content_type TEXT NOT NULL,
                content TEXT NOT NULL,
                preview TEXT NOT NULL,
                hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_items_created_at ON clipboard_items(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_items_hash ON clipboard_items(hash);
            "#,
        )?;
        Ok(())
    }

    pub fn insert_item(&self, item: &ClipboardItem) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // If the same content is already at the top, skip; otherwise allow
        // a new copy of identical text to bubble up to the top via insert.
        conn.execute(
            r#"INSERT INTO clipboard_items (id, content_type, content, preview, hash, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                item.id,
                item.content_type,
                item.content,
                item.preview,
                item.hash,
                item.created_at.to_rfc3339(),
            ],
        )?;
        // De-dup older copies of identical content so the list stays clean.
        conn.execute(
            "DELETE FROM clipboard_items WHERE hash = ?1 AND id != ?2",
            params![item.hash, item.id],
        )?;
        Ok(())
    }

    pub fn get_last_hash(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT hash FROM clipboard_items ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok())
    }

    pub fn get_items(
        &self,
        limit: u32,
        offset: u32,
        search: Option<&str>,
    ) -> Result<Vec<ClipboardItem>> {
        let conn = self.conn.lock().unwrap();
        let (sql, search_param) = match search {
            Some(s) if !s.is_empty() => (
                "SELECT id, content_type, content, preview, hash, created_at
                 FROM clipboard_items
                 WHERE content LIKE ?1 OR preview LIKE ?1
                 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
                    .to_string(),
                Some(format!("%{}%", s)),
            ),
            _ => (
                "SELECT id, content_type, content, preview, hash, created_at
                 FROM clipboard_items
                 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                    .to_string(),
                None,
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let row_to_item = |row: &rusqlite::Row| -> rusqlite::Result<ClipboardItem> {
            let created: String = row.get(5)?;
            Ok(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                content: row.get(2)?,
                preview: row.get(3)?,
                hash: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        };

        let items: Vec<ClipboardItem> = match search_param {
            Some(p) => stmt
                .query_map(params![p, limit, offset], row_to_item)?
                .collect::<Result<_>>()?,
            None => stmt
                .query_map(params![limit, offset], row_to_item)?
                .collect::<Result<_>>()?,
        };
        Ok(items)
    }

    pub fn get_item(&self, id: &str) -> Result<Option<ClipboardItem>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, content_type, content, preview, hash, created_at
             FROM clipboard_items WHERE id = ?1",
            params![id],
            |row| {
                let created: String = row.get(5)?;
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    preview: row.get(3)?,
                    hash: row.get(4)?,
                    created_at: DateTime::parse_from_rfc3339(&created)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                })
            },
        );
        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_item(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_items", [])?;
        Ok(())
    }

    pub fn enforce_limit(&self, limit: u32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_items
             WHERE id NOT IN (
                 SELECT id FROM clipboard_items
                 ORDER BY created_at DESC LIMIT ?1
             )",
            params![limit],
        )?;
        Ok(())
    }
}
