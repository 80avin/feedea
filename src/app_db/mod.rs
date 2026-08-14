use std::path::Path;

use rusqlite::Connection;

pub struct AppDb {
    pub conn: Connection,
}

pub fn open(data_dir: &Path) -> anyhow::Result<AppDb> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join("rssea.sqlite");
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(include_str!("schema.sql"))?;
    Ok(AppDb { conn })
}

impl AppDb {
    pub fn set_setting(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(rusqlite::params![key])?;
        Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
    }

    pub fn create_session(&mut self, token_hash: &str, expires_at: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions (token_hash, created_at, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![token_hash, chrono::Utc::now().to_rfc3339(), expires_at],
        )?;
        Ok(())
    }

    pub fn delete_session(&mut self, token_hash: &str) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM sessions WHERE token_hash = ?1", rusqlite::params![token_hash])?;
        Ok(())
    }

    pub fn session_exists(&self, token_hash: &str) -> anyhow::Result<bool> {
        let mut stmt = self.conn.prepare("SELECT 1 FROM sessions WHERE token_hash = ?1 AND expires_at > ?2")?;
        let mut rows = stmt.query(rusqlite::params![token_hash, chrono::Utc::now().to_rfc3339()])?;
        Ok(rows.next()?.is_some())
    }

    pub fn theme(&self) -> anyhow::Result<Option<String>> { self.get_setting("theme") }
    pub fn set_theme(&mut self, theme: &str) -> anyhow::Result<()> { self.set_setting("theme", theme) }
    pub fn sync_interval_minutes(&self) -> anyhow::Result<i64> {
        Ok(self.get_setting("sync_interval_minutes")?.and_then(|s| s.parse().ok()).unwrap_or(30))
    }
    pub fn set_sync_interval_minutes(&mut self, minutes: i64) -> anyhow::Result<()> {
        self.set_setting("sync_interval_minutes", &minutes.to_string())
    }

    pub fn password_hash(&self) -> anyhow::Result<Option<String>> {
        self.get_setting("password_hash")
    }

    pub fn set_password_hash(&mut self, hash: &str) -> anyhow::Result<()> {
        self.set_setting("password_hash", hash)
    }

    pub fn article_ids_for_tag(&self, tag: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT article_id FROM saved_tags WHERE tag = ?1")?;
        let mut rows = stmt.query(rusqlite::params![tag])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row.get(0)?);
        }
        Ok(out)
    }

    pub fn save_article(&mut self, article_id: &str, note: Option<&str>, tags: &[String]) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO saved (article_id, saved_at, note, updated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![article_id, now, note, now],
        )?;
        self.conn.execute("DELETE FROM saved_tags WHERE article_id = ?1", rusqlite::params![article_id])?;
        for tag in tags {
            self.conn.execute("INSERT OR IGNORE INTO tags (tag) VALUES (?1)", rusqlite::params![tag])?;
            self.conn.execute("INSERT OR REPLACE INTO saved_tags (article_id, tag) VALUES (?1, ?2)", rusqlite::params![article_id, tag])?;
        }
        Ok(())
    }

    pub fn unsave_article(&mut self, article_id: &str) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM saved WHERE article_id = ?1", rusqlite::params![article_id])?;
        self.conn.execute("DELETE FROM saved_tags WHERE article_id = ?1", rusqlite::params![article_id])?;
        Ok(())
    }

    pub fn saved_articles(&self, offset: i64, limit: i64) -> anyhow::Result<(Vec<(String, String)>, i64)> {
        let total: i64 = self.conn.query_row("SELECT COUNT(*) FROM saved", [], |r| r.get(0))?;
        let mut stmt = self.conn.prepare("SELECT article_id, saved_at FROM saved ORDER BY saved_at DESC LIMIT ?1 OFFSET ?2")?;
        let mut rows = stmt.query(rusqlite::params![limit, offset])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push((row.get(0)?, row.get(1)?));
        }
        Ok((out, total))
    }

    pub fn note_and_tags(&self, article_id: &str) -> anyhow::Result<(Option<String>, Vec<String>)> {
        let note: Option<String> = self.conn.query_row("SELECT note FROM saved WHERE article_id = ?1", rusqlite::params![article_id], |r| r.get(0)).unwrap_or(None);
        let mut stmt = self.conn.prepare("SELECT tag FROM saved_tags WHERE article_id = ?1 ORDER BY tag")?;
        let mut rows = stmt.query(rusqlite::params![article_id])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            tags.push(row.get(0)?);
        }
        Ok((note, tags))
    }

    pub fn all_tags(&self) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT tag FROM tags ORDER BY tag")?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row.get(0)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rssea-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn open_creates_schema_and_roundtrips_setting() {
        let dir = tmp_dir();
        let mut db = open(&dir).unwrap();
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("dark".to_string()));
        assert_eq!(db.get_setting("missing").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_create_exists_delete_roundtrip() {
        let dir = tmp_dir();
        let mut db = open(&dir).unwrap();
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        assert!(!db.session_exists("tok-hash").unwrap());
        db.create_session("tok-hash", &future.to_rfc3339()).unwrap();
        assert!(db.session_exists("tok-hash").unwrap());
        db.delete_session("tok-hash").unwrap();
        assert!(!db.session_exists("tok-hash").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expired_session_does_not_exist() {
        let dir = tmp_dir();
        let mut db = open(&dir).unwrap();
        let past = chrono::Utc::now() - chrono::Duration::seconds(1);
        db.create_session("stale", &past.to_rfc3339()).unwrap();
        assert!(!db.session_exists("stale").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn password_hash_roundtrip() {
        let dir = tmp_dir();
        let mut db = open(&dir).unwrap();
        assert_eq!(db.password_hash().unwrap(), None);
        db.set_password_hash("argon2hash").unwrap();
        assert_eq!(db.password_hash().unwrap(), Some("argon2hash".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_tables_exist() {
        let dir = tmp_dir();
        let db = open(&dir).unwrap();
        let tables: Vec<String> = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|x| x.ok())
            .collect();
        for t in ["saved", "saved_tags", "tags", "sessions", "settings"] {
            assert!(tables.contains(&t.to_string()), "missing table {t}: {tables:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
