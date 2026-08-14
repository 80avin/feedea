use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

pub struct SearchHit {
    pub article_id: String,
    pub title: Option<String>,
    pub feed_id: String,
    pub date: String,
    pub thumbnail_url: Option<String>,
}

pub struct CategoryTotals {
    pub category_id: String,
    pub total: i64,
    pub unread: i64,
}

pub fn open_readonly(db_path: &Path) -> anyhow::Result<Connection> {
    Ok(Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?)
}

pub fn descendant_category_ids(
    category_id: &str,
    children_by_parent: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![category_id.to_string()];
    let mut visited = HashSet::new();
    visited.insert(category_id.to_string());
    while let Some(current) = stack.pop() {
        out.push(current.clone());
        if let Some(children) = children_by_parent.get(&current) {
            for child in children {
                if visited.insert(child.clone()) {
                    stack.push(child.clone());
                }
            }
        }
    }
    out
}

pub fn search(db_path: &Path, query: &str, limit: i64) -> anyhow::Result<Vec<SearchHit>> {
    let conn = open_readonly(db_path)?;
    let term = news_flash::util::prepare_search_term(query);
    let mut stmt = conn.prepare(
        "SELECT a.article_id, a.title, a.feed_id, a.date, a.thumbnail_url
         FROM articles a
         WHERE a.rowid IN (SELECT rowid FROM fts_table WHERE fts_table MATCH ?1)
         ORDER BY a.date DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![term, limit], |row| {
        Ok(SearchHit {
            article_id: row.get(0)?,
            title: row.get(1)?,
            feed_id: row.get(2)?,
            date: row.get::<_, chrono::DateTime<chrono::Utc>>(3)?.to_rfc3339(),
            thumbnail_url: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn category_totals(db_path: &Path) -> anyhow::Result<Vec<CategoryTotals>> {
    let conn = open_readonly(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT fm.category_id,
                COUNT(a.article_id) AS total,
                SUM(CASE WHEN a.unread = 1 THEN 1 ELSE 0 END) AS unread
         FROM feed_mapping fm
         LEFT JOIN articles a ON a.feed_id = fm.feed_id
         GROUP BY fm.category_id",
    )?;
    let direct: HashMap<String, (i64, i64)> = stmt
        .query_map([], |row| {
            let unread: Option<i64> = row.get(2)?;
            Ok((row.get::<_, String>(0)?, (row.get(1)?, unread.unwrap_or(0))))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut cm_stmt = conn.prepare("SELECT parent_id, category_id FROM category_mapping")?;
    for cm in cm_stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (parent, child) = cm?;
        children.entry(parent).or_default().push(child);
    }

    let toplevel = news_flash::models::NEWSFLASH_TOPLEVEL.as_str().to_string();
    let mut cat_stmt = conn.prepare("SELECT category_id FROM categories")?;
    let mut cats: Vec<String> = cat_stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    cats.extend(direct.keys().cloned());
    cats.push(toplevel.clone());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cat in cats {
        if !seen.insert(cat.clone()) {
            continue;
        }
        let ids = descendant_category_ids(&cat, &children);
        let mut total = 0;
        let mut unread = 0;
        for id in ids {
            if let Some((t, u)) = direct.get(&id) {
                total += t;
                unread += u;
            }
        }
        out.push(CategoryTotals { category_id: cat, total, unread });
    }
    Ok(out)
}

pub fn total_article_count(db_path: &Path) -> anyhow::Result<i64> {
    let conn = open_readonly(db_path)?;
    Ok(conn.query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::engine::Engine;

    #[test]
    fn descendant_category_ids_walks_children_and_includes_self() {
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        children.entry("root".into()).or_default().extend(["a".to_string(), "b".to_string()]);
        children.entry("a".into()).or_default().push("a1".to_string());
        children.entry("b".into()).or_default().push("b1".to_string());
        let mut ids = descendant_category_ids("root", &children);
        ids.sort();
        assert_eq!(ids, vec!["a", "a1", "b", "b1", "root"]);
        let mut a = descendant_category_ids("a", &children);
        a.sort();
        assert_eq!(a, vec!["a", "a1"]);
        assert_eq!(descendant_category_ids("none", &children), vec!["none"]);
    }

    #[tokio::test]
    async fn search_and_counts_against_newsflash_db() {
        let server = crate::engine::tests::FeedServer::start(crate::engine::tests::RSS.to_string(), 10);
        let dir = std::env::temp_dir().join(format!("rssea-queries-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir.clone(), host: "127.0.0.1".into(), port: 0, allow_private_proxy: false };
        let engine = Engine::new(&config).await.unwrap();
        engine.add_feed(&server.url, Some("Test Feed".into()), None).await.unwrap();

        let db_path = dir.join("engine/data/database.sqlite");
        let hits = search(&db_path, "Alpha", 10).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].title.as_deref(), Some("Article Alpha"));
        let none = search(&db_path, "zzznottherezzz", 10).unwrap();
        assert!(none.is_empty());
        let totals = category_totals(&db_path).unwrap();
        assert!(totals.iter().any(|t| t.category_id == "NewsFlash.Toplevel" && t.total >= 2 && t.unread >= 2));
        assert!(total_article_count(&db_path).unwrap() >= 2);
        server.stop();
    }
}
