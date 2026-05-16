use anyhow::Result;
use rusqlite::Connection;

pub fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS feeds (
            id          INTEGER PRIMARY KEY,
            title       TEXT NOT NULL,
            url         TEXT UNIQUE NOT NULL,
            feed_type   TEXT NOT NULL DEFAULT 'rss',
            category    TEXT,
            icon_url    TEXT,
            last_fetched_at TEXT,
            error_count INTEGER DEFAULT 0,
            created_at  TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS entries (
            id           INTEGER PRIMARY KEY,
            feed_id      INTEGER REFERENCES feeds(id),
            guid         TEXT UNIQUE NOT NULL,
            title        TEXT,
            link         TEXT,
            summary      TEXT,
            content      TEXT,
            author       TEXT,
            published_at TEXT,
            fetched_at   TEXT DEFAULT (datetime('now')),
            is_read      INTEGER DEFAULT 0,
            is_starred   INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entry_tags (
            entry_id INTEGER REFERENCES entries(id),
            tag_id   INTEGER REFERENCES tags(id),
            PRIMARY KEY (entry_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS highlights (
            id         INTEGER PRIMARY KEY,
            entry_id   INTEGER REFERENCES entries(id),
            text       TEXT NOT NULL,
            note       TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS embeddings (
            id        INTEGER PRIMARY KEY,
            entry_id  INTEGER REFERENCES entries(id),
            embedding BLOB NOT NULL,
            model     TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id           INTEGER PRIMARY KEY,
            name         TEXT NOT NULL,
            model        TEXT NOT NULL,
            description  TEXT DEFAULT '',
            session_type TEXT NOT NULL DEFAULT 'chat',
            created_at   TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id         INTEGER PRIMARY KEY,
            session_id INTEGER REFERENCES sessions(id),
            role       TEXT NOT NULL,
            content    TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS session_files (
            id         INTEGER PRIMARY KEY,
            session_id INTEGER REFERENCES sessions(id),
            filename   TEXT NOT NULL,
            file_type  TEXT NOT NULL,
            filepath   TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS session_embeddings (
            id         INTEGER PRIMARY KEY,
            session_id INTEGER REFERENCES sessions(id),
            embedding  BLOB NOT NULL,
            model      TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_session_files_session ON session_files(session_id);
        CREATE INDEX IF NOT EXISTS idx_session_embeddings_session ON session_embeddings(session_id);

        CREATE INDEX IF NOT EXISTS idx_entries_feed_id ON entries(feed_id);
        CREATE INDEX IF NOT EXISTS idx_entries_guid ON entries(guid);
        CREATE INDEX IF NOT EXISTS idx_entries_is_read ON entries(is_read);
        CREATE INDEX IF NOT EXISTS idx_entries_published_at ON entries(published_at);
        CREATE INDEX IF NOT EXISTS idx_embeddings_entry_id ON embeddings(entry_id);
        ",
    )?;
    Ok(())
}
