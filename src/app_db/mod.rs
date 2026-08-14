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
