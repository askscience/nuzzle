use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::types::{AIMessage, AISession, Embedding, Entry, Feed, Highlight, Tag};

pub struct Repository {
    conn: Connection,
}

impl Repository {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {:?}", path))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        super::schema::initialize(&conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // --- Feeds ---

    pub fn list_feeds(&self) -> Result<Vec<Feed>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, url, feed_type, category, icon_url, last_fetched_at, error_count FROM feeds ORDER BY title",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Feed {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                feed_type: row.get(3)?,
                category: row.get(4)?,
                icon_url: row.get(5)?,
                last_fetched_at: row
                    .get::<_, Option<String>>(6)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                error_count: row.get(7)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_feed(&self, title: &str, url: &str, feed_type: &str) -> Result<Feed> {
        self.conn.execute(
            "INSERT OR IGNORE INTO feeds (title, url, feed_type) VALUES (?1, ?2, ?3)",
            params![title, url, feed_type],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Feed {
            id,
            title: title.to_string(),
            url: url.to_string(),
            feed_type: feed_type.to_string(),
            category: None,
            icon_url: None,
            last_fetched_at: None,
            error_count: 0,
        })
    }

    pub fn remove_feed(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM entries WHERE feed_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_feed_fetch_time(&self, id: i64, time: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE feeds SET last_fetched_at = ?1, error_count = 0 WHERE id = ?2",
            params![time, id],
        )?;
        Ok(())
    }

    pub fn increment_feed_error(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE feeds SET error_count = error_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // --- Entries ---

    pub fn list_entries(&self, feed_id: i64) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, guid, title, link, summary, content, author, published_at, fetched_at, is_read, is_starred
             FROM entries WHERE feed_id = ?1 ORDER BY published_at DESC",
        )?;
        let rows = stmt.query_map(params![feed_id], Self::map_entry)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_all_entries(&self) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, guid, title, link, summary, content, author, published_at, fetched_at, is_read, is_starred
             FROM entries ORDER BY published_at DESC",
        )?;
        let rows = stmt.query_map([], Self::map_entry)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_entry(&self, id: i64) -> Result<Option<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, guid, title, link, summary, content, author, published_at, fetched_at, is_read, is_starred
             FROM entries WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::map_entry)?;
        match rows.next() {
            Some(Ok(entry)) => Ok(Some(entry)),
            _ => Ok(None),
        }
    }

    pub fn entry_exists(&self, guid: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM entries WHERE guid = ?1")?;
        let count: i64 = stmt.query_row(params![guid], |row| row.get(0))?;
        Ok(count > 0)
    }

    pub fn insert_entry(&self, entry: &Entry) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO entries (feed_id, guid, title, link, summary, content, author, published_at, is_read, is_starred)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.feed_id,
                entry.guid,
                entry.title,
                entry.link,
                entry.summary,
                entry.content,
                entry.author,
                entry.published_at.map(|d| d.to_rfc3339()),
                entry.is_read,
                entry.is_starred,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn mark_read(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE entries SET is_read = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_star(&self, id: i64) -> Result<bool> {
        self.conn.execute(
            "UPDATE entries SET is_starred = CASE WHEN is_starred THEN 0 ELSE 1 END WHERE id = ?1",
            params![id],
        )?;
        let new: bool = self.conn.query_row(
            "SELECT is_starred FROM entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(new)
    }

    // --- Tags ---

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Tag { id: row.get(0)?, name: row.get(1)? })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn entry_tags(&self, entry_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name FROM tags t
             JOIN entry_tags et ON et.tag_id = t.id
             WHERE et.entry_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![entry_id], |row| {
            Ok(Tag { id: row.get(0)?, name: row.get(1)? })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_tag_to_entry(&self, entry_id: i64, tag_name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag_name],
        )?;
        let tag_id: i64 = self.conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![tag_name],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_tags (entry_id, tag_id) VALUES (?1, ?2)",
            params![entry_id, tag_id],
        )?;
        Ok(())
    }

    // --- Highlights ---

    pub fn list_highlights(&self) -> Result<Vec<Highlight>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_id, text, note, created_at FROM highlights ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Highlight {
                id: row.get(0)?,
                entry_id: row.get(1)?,
                text: row.get(2)?,
                note: row.get(3)?,
                created_at: row
                    .get::<_, Option<String>>(4)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_highlight(&self, entry_id: i64, text: &str, note: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO highlights (entry_id, text, note) VALUES (?1, ?2, ?3)",
            params![entry_id, text, note],
        )?;
        Ok(())
    }

    pub fn remove_highlight(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM highlights WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- Embeddings ---

    pub fn save_embedding(&self, entry_id: i64, embedding: &[f32], model: &str) -> Result<()> {
        let bytes: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (entry_id, embedding, model) VALUES (?1, ?2, ?3)",
            params![entry_id, bytes, model],
        )?;
        Ok(())
    }

    pub fn load_all_embeddings(&self) -> Result<Vec<Embedding>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.entry_id, e.embedding, e.model FROM embeddings e
             JOIN entries en ON en.id = e.entry_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let bytes: Vec<u8> = row.get(2)?;
            let embedding: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(Embedding {
                id: row.get(0)?,
                entry_id: row.get(1)?,
                embedding,
                model: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // --- Sessions ---

    pub fn create_session(&self, name: &str, model: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (name, model) VALUES (?1, ?2)",
            params![name, model],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_sessions(&self) -> Result<Vec<AISession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, model, created_at FROM sessions ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AISession {
                id: row.get(0)?,
                name: row.get(1)?,
                model: row.get(2)?,
                created_at: row.get::<_, Option<String>>(3)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_session(&self, id: i64) -> Result<Option<AISession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, model, created_at FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(AISession {
                id: row.get(0)?,
                name: row.get(1)?,
                model: row.get(2)?,
                created_at: row.get::<_, Option<String>>(3)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?;
        match rows.next() { Some(Ok(s)) => Ok(Some(s)), _ => Ok(None) }
    }

    pub fn add_message(&self, session_id: i64, role: &str, content: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO messages (session_id, role, content) VALUES (?1, ?2, ?3)",
            params![session_id, role, content],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn session_messages(&self, session_id: i64, limit: usize) -> Result<Vec<AIMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, created_at FROM messages
             WHERE session_id = ?1 ORDER BY id ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], |row| {
            Ok(AIMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get::<_, Option<String>>(4)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_entry(row: &rusqlite::Row) -> rusqlite::Result<Entry> {
        Ok(Entry {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            guid: row.get(2)?,
            title: row.get(3)?,
            link: row.get(4)?,
            summary: row.get(5)?,
            content: row.get(6)?,
            author: row.get(7)?,
            published_at: row
                .get::<_, Option<String>>(8)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            fetched_at: row
                .get::<_, Option<String>>(9)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            is_read: row.get(10)?,
            is_starred: row.get(11)?,
        })
    }
}
