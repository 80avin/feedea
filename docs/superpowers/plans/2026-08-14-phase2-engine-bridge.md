# Phase 2: Engine Bridge + First Sync — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Boot a headless news-flash engine (`local_rss`) as a background actor, add a source, sync articles, and expose feeds + article headlines + favicon/thumbnail through our axum API.

**Architecture:** A new `Engine` struct in `src/engine/` owns the `NewsFlash` instance (held in `Arc`) plus a reqwest 0.13 client and a mutation lock. All sync `get_*`/count calls and the synchronous `create()` run via `tokio::task::spawn_blocking`. A tokio scheduler task (`src/engine/sync.rs`) periodically calls `sync_all()`. DTOs live in a new `src/dto.rs` shared by engine and API. Axum handlers take `State<AppState>` where `AppState { engine: Engine }`.

**Tech Stack:** news-flash 3.2.0 (local_rss plugin), reqwest 0.13.4, axum 0.8.9, tokio 1.53.1, serde/serde_json, anyhow 1, chrono 0.4 (NEW dependency), tracing.

## Global Constraints

- No comments in code unless a task explicitly shows them.
- `reqwest` is 0.13 (news-flash's own Client type — do not change).
- All synchronous news-flash calls (`get_*`, counts, `create()`) MUST run inside `spawn_blocking`. Async news-flash methods (`sync`, `add_feed`, `fetch_feed`, `get_article_thumbnail`) are network-bound; run them under the engine's mutation lock but may be awaited directly (documented design decision, not a defect).
- news-flash engine data lives under `<data_dir>/engine/config` and `<data_dir>/engine/data`. Our sidecar (`feedea.sqlite`) stays at `<data_dir>/feedea.sqlite` (untouched this phase).
- `ArticleFilter.order` MUST be `Some(ArticleOrder::NewestFirst)` whenever you want deterministic date-desc ordering — news-flash applies NO ORDER BY when `order` is `None`.
- DTOs are defined in `src/dto.rs`, shared by engine and API. Do not serialize news-flash model types directly.
- Add `chrono = "0.4"` to Cargo.toml.
- Keep `/api/health` working unchanged.
- The API in this phase is unauthenticated (auth arrives in Phase 3).
- Commit after each task with the exact message given.
- Edition 2024, nightly rust 1.99.

---

### Task 1: Engine struct + headless init + spawn_blocking bridge

**Files:**
- Create: `src/engine/mod.rs` (replace empty stub)
- Modify: `src/engine/sync.rs` (keep stub; scheduler added in Task 5)
- Modify: `Cargo.toml` (add chrono)
- Test: inline unit tests in `src/engine/mod.rs`

**Interfaces:**
- Consumes: `crate::config::Config` (`{ data_dir: PathBuf, host: String, port: u16 }`, `ensure_data_dir()`).
- Produces:
  - `#[derive(Clone)] pub struct Engine { nf: Arc<NewsFlash>, client: reqwest::Client, mutation_lock: Arc<tokio::sync::Mutex<()>> }`
  - `impl Engine { pub async fn new(config: &Config) -> anyhow::Result<Engine>; pub async fn with_nf<T,F>(&self, f: F) -> anyhow::Result<T>; pub fn client(&self) -> &reqwest::Client; pub async fn mutation_guard(&self) -> tokio::sync::MutexGuard<'_, ()>; pub async fn last_sync(&self) -> chrono::DateTime<chrono::Utc>; }`
  - `with_nf` signature: `where T: Send + 'static, F: FnOnce(&NewsFlash) -> news_flash::error::NewsFlashResult<T> + Send + 'static` — runs `f` via `spawn_blocking` and maps errors to `anyhow::Error`.

- [ ] **Step 1: Add chrono to Cargo.toml**

Add to `[dependencies]`: `chrono = "0.4"`.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("feedea-engine-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn engine_inits_headless_with_dirs() {
        let dir = tmp_dir();
        let config = Config {
            data_dir: dir.clone(),
            host: "127.0.0.1".into(),
            port: 0,
        };
        let engine = Engine::new(&config).await.unwrap();
        assert!(dir.join("engine/config").exists());
        assert!(dir.join("engine/data").exists());
        let empty = engine
            .with_nf(|nf| nf.is_database_empty())
            .await
            .unwrap();
        assert!(empty);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test engine::tests`
Expected: FAIL — `Engine`, `with_nf` don't exist.

- [ ] **Step 4: Implement src/engine/mod.rs**

```rust
use std::sync::Arc;

use news_flash::error::NewsFlashError;
use news_flash::models::PluginID;
use news_flash::NewsFlash;

use crate::config::Config;

pub mod sync;

#[derive(Clone)]
pub struct Engine {
    nf: Arc<NewsFlash>,
    client: reqwest::Client,
    mutation_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Engine {
    pub async fn new(config: &Config) -> anyhow::Result<Engine> {
        config.ensure_data_dir()?;
        let engine_dir = config.data_dir.join("engine");
        let config_dir = engine_dir.join("config");
        let data_dir = engine_dir.join("data");
        let nf = tokio::task::spawn_blocking(move || {
            NewsFlash::builder()
                .plugin(PluginID::new("local_rss"))
                .config_dir(&config_dir)
                .data_dir(&data_dir)
                .create()
                .map_err(anyhow::Error::from)
        })
        .await??;
        Ok(Engine {
            nf: Arc::new(nf),
            client: reqwest::Client::new(),
            mutation_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub async fn with_nf<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&NewsFlash) -> Result<T, NewsFlashError> + Send + 'static,
    {
        let nf = self.nf.clone();
        tokio::task::spawn_blocking(move || f(&nf))
            .await?
            .map_err(anyhow::Error::from)
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn mutation_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mutation_lock.lock().await
    }

    pub async fn last_sync(&self) -> chrono::DateTime<chrono::Utc> {
        self.nf.last_sync().await
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test engine::tests`
Expected: PASS. Then run `cargo test` (full) — all prior tests still pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/engine/mod.rs && git commit -m "Phase 2: add Engine struct with headless init and spawn_blocking bridge"
```

---

### Task 2: Engine mutations — add_feed, sync_all, fetch_feed + test feed server

**Files:**
- Modify: `src/engine/mod.rs`
- Create: `tests/feed_server.rs` (shared test helper)
- Modify: `src/engine/mod.rs` tests (use the helper)

**Interfaces:**
- Consumes: `Engine::new`, `mutation_guard`, `with_nf`, `client`.
- Produces (in `src/dto.rs` — create it):
  - `#[derive(Serialize, Debug, Clone)] pub struct FeedSummary { pub id: String, pub title: String, pub website: Option<String>, pub feed_url: Option<String>, pub icon_url: Option<String>, pub category_id: String, pub unread_count: i64, pub error_count: i32, pub error_message: Option<String> }`
- Engine methods (added):
  - `pub async fn add_feed(&self, url: &str, title: Option<String>, category_id: Option<String>) -> anyhow::Result<FeedSummary>` — parses Url, takes mutation guard, calls `self.nf.add_feed(&url, title, category_id, &self.client).await`, maps `(Feed, FeedMapping, ..)` → FeedSummary (category_id from mapping, unread 0).
  - `pub async fn sync_all(&self) -> anyhow::Result<std::collections::HashMap<String, i64>>` — mutation guard, `FeedHeaderMap` empty, `self.nf.sync(&self.client, header).await`, keyed by `FeedID::as_str()`.
  - `pub async fn fetch_feed(&self, feed_id: &str) -> anyhow::Result<i64>` — mutation guard, `self.nf.fetch_feed(&FeedID::new(feed_id), &self.client, reqwest::header::HeaderMap::new()).await`.

- [ ] **Step 1: Write the failing test helper `tests/feed_server.rs`**

```rust
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};

pub struct FeedServer {
    pub url: String,
    handle: JoinHandle<()>,
}

impl FeedServer {
    pub fn start(rss_body: String, connections: usize) -> FeedServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/feed.xml");
        let handle = thread::spawn(move || {
            for _ in 0..connections {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            rss_body.len(),
                            rss_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(_) => break,
                }
            }
        });
        FeedServer { url, handle }
    }

    pub fn stop(self) {
        self.handle.join().unwrap();
    }
}
```

- [ ] **Step 2: Write the failing test (in `src/engine/mod.rs` tests module, plus add a `RSS` sample)**

Add to the tests module:

```rust
const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <link>http://127.0.0.1/</link>
    <description>test</description>
    <item>
      <title>Article Alpha</title>
      <link>http://example.com/alpha</link>
      <guid isPermaLink="false">alpha-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
    <item>
      <title>Article Beta</title>
      <link>http://example.com/beta</link>
      <guid isPermaLink="false">beta-1</guid>
      <pubDate>Tue, 12 Aug 2026 11:30:00 GMT</pubDate>
      <description>Beta body.</description>
    </item>
  </channel>
</rss>
"#;

#[tokio::test]
async fn add_feed_and_sync_pulls_articles() {
    let server = crate::engine::tests::FeedServer::start(RSS.to_string(), 6);
    let dir = tmp_dir();
    let config = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 0,
    };
    let engine = Engine::new(&config).await.unwrap();
    let feed = engine.add_feed(&server.url, Some("Test Feed".into()), None).await.unwrap();
    assert_eq!(feed.title, "Test Feed");
    let counts = engine.sync_all().await.unwrap();
    assert_eq!(counts.get(&server.url).copied(), Some(2));
    let article_count = engine.with_nf(|nf| nf.get_article_ids(news_flash::models::ArticleFilter::default()).map(|v| v.len())).await.unwrap();
    assert_eq!(article_count, 2);
    let counts2 = engine.sync_all().await.unwrap();
    assert_eq!(counts2.get(&server.url).copied(), Some(0));
    server.stop();
}
```

Note: `FeedServer` and `RSS` must be defined in a `#[cfg(test)] pub mod tests { ... }` block at the bottom of `src/engine/mod.rs` (crate unit tests cannot import integration-test modules). The test above references them as `crate::engine::tests::FeedServer` and the bare `RSS` (import with `use super::*;` or `crate::engine::tests::RSS`). Do NOT create `tests/feed_server.rs` in Task 2 — that file is created in Task 4 for the integration tests (which CAN import it via `mod feed_server;`).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test engine::tests::add_feed_and_sync_pulls_articles`
Expected: FAIL — `add_feed`/`sync_all` don't exist (or FeedServer not found if you kept the file).

- [ ] **Step 4: Implement add_feed, sync_all, fetch_feed**

In `src/engine/mod.rs` add `use` lines and methods:

```rust
use news_flash::feed_api::FeedHeaderMap;
use news_flash::models::{ArticleFilter, CategoryID, FeedID, Url};
use std::collections::HashMap;

use crate::dto::FeedSummary;

    pub async fn add_feed(
        &self,
        url: &str,
        title: Option<String>,
        category_id: Option<String>,
    ) -> anyhow::Result<FeedSummary> {
        let url = Url::parse(url)?;
        let category_id = category_id.map(|c| CategoryID::new(&c));
        let _guard = self.mutation_guard().await;
        let (feed, feed_mapping, _, _) = self.nf.add_feed(&url, title, category_id, &self.client).await?;
        Ok(FeedSummary {
            id: feed.feed_id.as_str().to_string(),
            title: feed.label,
            website: feed.website.map(|u| u.to_string()),
            feed_url: feed.feed_url.map(|u| u.to_string()),
            icon_url: feed.icon_url.map(|u| u.to_string()),
            category_id: feed_mapping.category_id.as_str().to_string(),
            unread_count: 0,
            error_count: feed.error_count,
            error_message: feed.error_message,
        })
    }

    pub async fn sync_all(&self) -> anyhow::Result<HashMap<String, i64>> {
        let _guard = self.mutation_guard().await;
        let header: FeedHeaderMap = HashMap::new();
        let counts = self.nf.sync(&self.client, header).await?;
        Ok(counts.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
    }

    pub async fn fetch_feed(&self, feed_id: &str) -> anyhow::Result<i64> {
        let _guard = self.mutation_guard().await;
        let feed_id = FeedID::new(feed_id);
        Ok(self.nf.fetch_feed(&feed_id, &self.client, reqwest::header::HeaderMap::new()).await?)
    }
```

Create `src/dto.rs`:

```rust
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct FeedSummary {
    pub id: String,
    pub title: String,
    pub website: Option<String>,
    pub feed_url: Option<String>,
    pub icon_url: Option<String>,
    pub category_id: String,
    pub unread_count: i64,
    pub error_count: i32,
    pub error_message: Option<String>,
}
```

Add `pub mod dto;` to `src/lib.rs` (alphabetical: after config).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test engine::tests`
Expected: PASS. Then `cargo test` full.

- [ ] **Step 6: Commit**

```bash
git add src/engine/mod.rs src/dto.rs src/lib.rs && git commit -m "Phase 2: add add_feed, sync_all, fetch_feed engine mutations"
```

---

### Task 3: Engine reads — feeds, headlines, article detail

**Files:**
- Modify: `src/engine/mod.rs`
- Modify: `src/dto.rs`

**Interfaces:**
- Consumes: `with_nf`, `get_feeds` (built here).
- Produces DTOs in `src/dto.rs`:
  - `FeedSummary` (exists)
  - `#[derive(Serialize, Debug, Clone)] pub struct Headline { pub id: String, pub title: Option<String>, pub feed_id: String, pub feed_title: Option<String>, pub url: Option<String>, pub date: chrono::DateTime<chrono::Utc>, pub summary: Option<String>, pub thumbnail_url: Option<String>, pub unread: bool, pub marked: bool }`
  - `#[derive(Serialize, Debug)] pub struct ArticleDetail { pub id: String, pub title: Option<String>, pub author: Option<String>, pub feed_id: String, pub feed_title: Option<String>, pub url: Option<String>, pub date: chrono::DateTime<chrono::Utc>, pub html: Option<String>, pub summary: Option<String>, pub unread: bool, pub marked: bool, pub thumbnail_url: Option<String>, pub plain_text: Option<String> }`
- Engine methods:
  - `pub async fn get_feeds(&self) -> anyhow::Result<Vec<FeedSummary>>` — unread map + get_feeds + feed_mappings; category_id from mapping (fallback `NewsFlash.Toplevel`).
  - `pub async fn get_headlines(&self, filter: ArticleFilter) -> anyhow::Result<Vec<Headline>>` — get_fat_articles then map; feed_title from a feed-id→label lookup (call get_feeds once).
  - `pub async fn get_article_detail(&self, article_id: &str) -> anyhow::Result<ArticleDetail>` — get_fat_article; html = `scraped_content.or(html)`.
- Conversions: `Read`/`Marked` → bool (`a.unread == news_flash::models::Read::Unread`, `a.marked == news_flash::models::Marked::Marked`).

- [ ] **Step 1: Write the failing tests (extend tests module)**

```rust
#[tokio::test]
async fn reads_return_feeds_and_headlines() {
    let server = crate::engine::tests::FeedServer::start(RSS.to_string(), 6);
    let config = Config { data_dir: tmp_dir(), host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let feed = engine.add_feed(&server.url, Some("Test Feed".into()), None).await.unwrap();
    engine.sync_all().await.unwrap();

    let feeds = engine.get_feeds().await.unwrap();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].title, "Test Feed");

    let mut filter = news_flash::models::ArticleFilter::default();
    filter.order = Some(news_flash::models::ArticleOrder::NewestFirst);
    filter.order_by = Some(news_flash::models::OrderBy::Published);
    let headlines = engine.get_headlines(filter).await.unwrap();
    assert_eq!(headlines.len(), 2);
    assert_eq!(headlines[0].title.as_deref(), Some("Article Beta"));
    assert!(headlines[0].unread);
    assert!(!headlines[0].marked);

    let detail = engine.get_article_detail(&headlines[0].id).await.unwrap();
    assert_eq!(detail.title.as_deref(), Some("Article Beta"));
    server.stop();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test engine::tests::reads_return_feeds_and_headlines`
Expected: FAIL — methods don't exist.

- [ ] **Step 3: Implement**

In `src/engine/mod.rs`:

```rust
use news_flash::models::{ArticleOrder, Marked, OrderBy, Read};

    pub async fn get_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let (feeds, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let category_by_feed: HashMap<String, String> = mappings
            .into_iter()
            .map(|m| (m.feed_id.as_str().to_string(), m.category_id.as_str().to_string()))
            .collect();
        let mut out = Vec::with_capacity(feeds.len());
        for feed in feeds {
            let id = feed.feed_id.as_str().to_string();
            out.push(FeedSummary {
                category_id: category_by_feed.get(&id).cloned().unwrap_or_else(|| "NewsFlash.Toplevel".to_string()),
                unread_count: unread.get(&feed.feed_id).copied().unwrap_or(0),
                id,
                title: feed.label,
                website: feed.website.map(|u| u.to_string()),
                feed_url: feed.feed_url.map(|u| u.to_string()),
                icon_url: feed.icon_url.map(|u| u.to_string()),
                error_count: feed.error_count,
                error_message: feed.error_message,
            });
        }
        Ok(out)
    }

    pub async fn get_headlines(&self, filter: ArticleFilter) -> anyhow::Result<Vec<Headline>> {
        let articles = self.with_nf(|nf| nf.get_fat_articles(filter)).await?;
        let feed_titles: HashMap<String, String> = self
            .get_feeds()
            .await?
            .into_iter()
            .map(|f| (f.id, f.title))
            .collect();
        let mut out = Vec::with_capacity(articles.len());
        for a in articles {
            let feed_id = a.feed_id.as_str().to_string();
            out.push(Headline {
                id: a.article_id.as_str().to_string(),
                title: a.title,
                feed_id: feed_id.clone(),
                feed_title: feed_titles.get(&feed_id).cloned(),
                url: a.url.map(|u| u.to_string()),
                date: a.date,
                summary: a.summary,
                thumbnail_url: a.thumbnail_url,
                unread: a.unread == Read::Unread,
                marked: a.marked == Marked::Marked,
            });
        }
        Ok(out)
    }

    pub async fn get_article_detail(&self, article_id: &str) -> anyhow::Result<ArticleDetail> {
        let id = news_flash::models::ArticleID::new(article_id);
        let a = self.with_nf(move |nf| nf.get_fat_article(&id)).await?;
        let feed_id = a.feed_id.as_str().to_string();
        let feed_title = self
            .get_feeds()
            .await?
            .into_iter()
            .find(|f| f.id == feed_id)
            .map(|f| f.title);
        Ok(ArticleDetail {
            id: a.article_id.as_str().to_string(),
            title: a.title,
            author: a.author,
            feed_id,
            feed_title,
            url: a.url.map(|u| u.to_string()),
            date: a.date,
            html: a.scraped_content.or(a.html),
            summary: a.summary,
            unread: a.unread == Read::Unread,
            marked: a.marked == Marked::Marked,
            thumbnail_url: a.thumbnail_url,
            plain_text: a.plain_text,
        })
    }
```

Add the new DTOs to `src/dto.rs`. Add `ArticleFilter`, `ArticleID`, `Headline`, `ArticleDetail` imports to `src/engine/mod.rs` as needed.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test engine::tests`
Expected: PASS. Then full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/engine/mod.rs src/dto.rs && git commit -m "Phase 2: add engine read queries (feeds, headlines, article detail)"
```

---

### Task 4: API endpoints — feeds, articles, detail, favicon, thumbnail, sources

**Files:**
- Create: `src/api/feeds.rs`, `src/api/articles.rs`, `src/api/sources.rs`, `src/api/error.rs`
- Modify: `src/api/mod.rs` (router with State<AppState>)
- Modify: `src/lib.rs` (add `AppState`, wire engine into run() — actually keep run() wiring for Task 5; here add `AppState` + adjust `router` signature)
- Test: `tests/api.rs` integration tests

**Interfaces:**
- Consumes: `Engine` methods from Tasks 1-3, DTOs, news-flash models.
- Produces:
  - `src/lib.rs`: `#[derive(Clone)] pub struct AppState { pub engine: engine::Engine }`
  - `src/api/mod.rs`: `pub fn router(state: AppState) -> axum::Router` (replaces `router(config: Config)`)
  - `src/api/error.rs`: `pub struct ApiError { status: StatusCode, code: &'static str, message: String }` implementing `IntoResponse` returning `{error:{code,message}}` JSON. `impl From<anyhow::Error> for ApiError` (500, code "internal"), plus constructors.
  - Routes:
    - `GET /api/feeds` → `feeds::list(State) -> Json<Vec<FeedSummary>>`
    - `GET /api/articles?feed=&category=&offset=&limit=` → `articles::list` (defaults: offset 0, limit 30; build ArticleFilter with `order=Some(NewestFirst)`, `order_by=Some(Published)`; category→CategoryID, feed→FeedID)
    - `GET /api/articles/:id` → `articles::detail`
    - `GET /api/favicon/:feed_id` → `articles::favicon` (bytes or 404)
    - `GET /api/thumbnail/:article_id` → `articles::thumbnail`
    - `POST /api/sources` `{url, title?, category_id?}` → `sources::add`
    - `POST /api/sources/:id/refresh` → `sources::refresh`
  - Engine methods (add to Task 4 scope, used by handlers):
    - `pub async fn get_favicon(&self, feed_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>>` — `load_icon_from_db(&FeedID::new(feed_id))` via with_nf; return `(format.unwrap_or("image/x-icon"), highres.or(lowres))` or None.
    - `pub async fn get_article_thumbnail(&self, article_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>>` — `nf.get_article_thumbnail(&ArticleID::new(article_id), &self.client).await`; on Ok(Some(t)) with `t.data` Some → `(format.unwrap_or("image/jpeg"), data)`; else None.

- [ ] **Step 1: Write the failing integration test `tests/api.rs`**

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use feedea::api;
use feedea::config::Config;
use feedea::engine::Engine;
use feedea::AppState;
use std::path::PathBuf;
use tower::ServiceExt;

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>API Feed</title>
    <link>http://127.0.0.1/</link>
    <description>api</description>
    <item>
      <title>API Article</title>
      <link>http://example.com/one</link>
      <guid isPermaLink="false">api-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>API body.</description>
    </item>
  </channel>
</rss>
"#;

mod feed_server;

async fn spawn_app() -> (String, axum::Router) {
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
    let dir = std::env::temp_dir().join(format!("feedea-api-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let router = api::router(AppState { engine: engine.clone() });
    (server.url, router)
}

#[tokio::test]
async fn add_source_sync_and_read_articles() {
    let (feed_url, app) = spawn_app().await;

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"API Feed"}}"#)))
            .unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["title"], "API Feed");

    let refresh_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri("/api/sources/refresh-all")
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let list_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?offset=0&limit=10").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let articles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = articles.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "API Article");

    let id = arr[0]["id"].as_str().unwrap().to_string();
    let detail_resp = app.clone()
        .oneshot(Request::builder().uri(&format!("/api/articles/{id}")).body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
}
```

Notes:
- `tests/api.rs` CAN import the integration-test helper from `tests/feed_server.rs` via `mod feed_server;`. So KEEP `tests/feed_server.rs` for the API tests, and ALSO have the crate-internal helper for engine unit tests (Task 2). Both exist; they are separate copies (integration vs unit test crates cannot share code). This is acceptable duplication in test code; do not over-engineer.
- Add a `POST /api/sources/refresh-all` endpoint (calls `engine.sync_all()`) — used by the test and useful for "sync now". Add it to `sources.rs`.
- `AppState` must be `pub` in `src/lib.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test api`
Expected: FAIL (compile error — routes/AppState missing).

- [ ] **Step 3: Implement error.rs, feeds.rs, articles.rs, sources.rs, router**

`src/api/error.rs`:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::NOT_FOUND, code: "not_found", message: message.into() }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::BAD_REQUEST, code: "bad_request", message: message.into() }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal", message: message.into() }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(%e, "request failed");
        ApiError::internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": { "code": self.code, "message": self.message } }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
```

`src/api/feeds.rs`:

```rust
use axum::extract::State;
use axum::Json;

use crate::api::error::ApiResult;
use crate::dto::FeedSummary;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<FeedSummary>>> {
    Ok(Json(state.engine.get_feeds().await?))
}
```

`src/api/articles.rs`:

```rust
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::http::{StatusCode, header};
use serde::Deserialize;

use crate::api::error::{ApiError, ApiResult};
use crate::dto::{ArticleDetail, Headline};
use crate::AppState;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, FeedID, OrderBy};

#[derive(Deserialize)]
pub struct ListParams {
    pub feed: Option<String>,
    pub category: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn list(State(state): State<AppState>, Query(params): Query<ListParams>) -> ApiResult<Json<Vec<Headline>>> {
    let mut filter = ArticleFilter::default();
    filter.order = Some(ArticleOrder::NewestFirst);
    filter.order_by = Some(OrderBy::Published);
    filter.limit = Some(params.limit.unwrap_or(30).clamp(1, 200));
    filter.offset = Some(params.offset.unwrap_or(0).max(0));
    if let Some(feed) = params.feed {
        filter.feeds = Some(vec![FeedID::new(&feed)]);
    }
    if let Some(category) = params.category {
        filter.categories = Some(vec![CategoryID::new(&category)]);
    }
    Ok(Json(state.engine.get_headlines(filter).await?))
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<ArticleDetail>> {
    state.engine.get_article_detail(&id).await.map(Json).map_err(ApiError::from)
}

pub async fn favicon(State(state): State<AppState>, Path(feed_id): Path<String>) -> Response {
    match state.engine.get_favicon(&feed_id).await {
        Ok(Some((content_type, data))) => (
            [(header::CONTENT_TYPE, content_type)],
            data,
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn thumbnail(State(state): State<AppState>, Path(article_id): Path<String>) -> Response {
    match state.engine.get_article_thumbnail(&article_id).await {
        Ok(Some((content_type, data))) => (
            [(header::CONTENT_TYPE, content_type)],
            data,
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
```

`src/api/sources.rs`:

```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::api::error::ApiResult;
use crate::dto::FeedSummary;
use crate::AppState;

#[derive(Deserialize)]
pub struct AddSourceRequest {
    pub url: String,
    pub title: Option<String>,
    pub category_id: Option<String>,
}

pub async fn add(State(state): State<AppState>, Json(req): Json<AddSourceRequest>) -> ApiResult<Json<FeedSummary>> {
    Ok(Json(state.engine.add_feed(&req.url, req.title, req.category_id).await?))
}

pub async fn refresh(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let count = state.engine.fetch_feed(&id).await?;
    Ok(Json(serde_json::json!({ "new_articles": count })))
}

pub async fn refresh_all(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let counts = state.engine.sync_all().await?;
    Ok(Json(serde_json::json!({ "feeds": counts })))
}
```

`src/api/mod.rs`:

```rust
pub mod articles;
pub mod error;
pub mod feeds;
pub mod health;
pub mod sources;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/feeds", get(feeds::list))
        .route("/api/articles", get(articles::list))
        .route("/api/articles/:id", get(articles::detail))
        .route("/api/favicon/:feed_id", get(articles::favicon))
        .route("/api/thumbnail/:article_id", get(articles::thumbnail))
        .route("/api/sources", post(sources::add))
        .route("/api/sources/:id/refresh", post(sources::refresh))
        .route("/api/sources/refresh-all", post(sources::refresh_all))
        .with_state(state)
}
```

In `src/lib.rs`: add `pub mod dto;` (from Task 2), add `#[derive(Clone)] pub struct AppState { pub engine: engine::Engine }`. Note: `run()` still calls `api::router(config)` in Task 4 — fix its signature reference after adding AppState. Simplest: in Task 4, change `run()`'s call to construct an engine + AppState (moves run() wiring forward; the scheduler task is Task 5). Update `run()`:

```rust
pub async fn run(config: Config) -> anyhow::Result<()> {
    config.ensure_data_dir()?;
    let engine = engine::Engine::new(&config).await?;
    let state = AppState { engine };
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("feedea {} listening on {}", crate::version(), listener.local_addr()?);
    axum::serve(listener, api::router(state)).await?;
    Ok(())
}
```

Update `tests/health.rs` to the new `router(state)` signature — build an Engine + AppState there too (or change it to use a helper). Simplest: update tests/health.rs to construct an Engine + AppState.

- [ ] **Step 4: Implement Engine favicon/thumbnail methods**

In `src/engine/mod.rs`:

```rust
    pub async fn get_favicon(&self, feed_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let feed_id = FeedID::new(feed_id);
        let icon = self.with_nf(move |nf| nf.load_icon_from_db(&feed_id)).await?;
        let data = icon.highres.or(icon.lowres);
        Ok(data.map(|bytes| (icon.format.unwrap_or_else(|| "image/x-icon".to_string()), bytes)))
    }

    pub async fn get_article_thumbnail(&self, article_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let article_id = news_flash::models::ArticleID::new(article_id);
        match self.nf.get_article_thumbnail(&article_id, &self.client).await? {
            Some(thumbnail) => {
                let format = thumbnail.format.unwrap_or_else(|| "image/jpeg".to_string());
                Ok(thumbnail.data.map(|d| (format, d)))
            }
            None => Ok(None),
        }
    }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test api` and `cargo test --test health`
Expected: PASS. Then full `cargo test`.

- [ ] **Step 6: Commit**

```bash
git add src/api/ src/lib.rs src/engine/mod.rs tests/api.rs tests/feed_server.rs && git commit -m "Phase 2: add feeds/articles/sources API endpoints"
```

---

### Task 5: Sync scheduler + wire into run()

**Files:**
- Modify: `src/engine/sync.rs`
- Modify: `src/lib.rs` (`run()` spawns scheduler)

**Interfaces:**
- Consumes: `Engine` (Clone), `Engine::sync_all`.
- Produces:
  - `pub const DEFAULT_SYNC_INTERVAL: std::time::Duration` (30 minutes)
  - `pub async fn scheduler_loop(engine: Engine, interval: std::time::Duration)` — infinite tokio interval task; on tick calls `engine.sync_all()`, logs warn on error.
  - `run()` spawns `tokio::spawn(sync::scheduler_loop(engine.clone(), sync::DEFAULT_SYNC_INTERVAL))`.

- [ ] **Step 1: Write the failing test (in `src/engine/sync.rs` tests module)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::engine::Engine;

    #[test]
    fn default_interval_is_thirty_minutes() {
        assert_eq!(DEFAULT_SYNC_INTERVAL, std::time::Duration::from_secs(30 * 60));
    }

    #[tokio::test]
    async fn scheduler_updates_last_sync() {
        let server = crate::engine::tests::FeedServer::start(
            crate::engine::tests::RSS.to_string(),
            6,
        );
        let dir = std::env::temp_dir().join(format!("feedea-sched-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
        let engine = Engine::new(&config).await.unwrap();
        engine.add_feed(&server.url, Some("Sched Feed".into()), None).await.unwrap();

        let handle = tokio::spawn(scheduler_loop(engine.clone(), std::time::Duration::from_millis(50)));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        handle.abort();

        let unread = engine
            .with_nf(|nf| nf.unread_count_all())
            .await
            .unwrap();
        assert_eq!(unread, 2);
        server.stop();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test engine::sync::tests`
Expected: FAIL — module has no tests / scheduler_loop doesn't exist.

- [ ] **Step 3: Implement sync.rs**

```rust
use std::time::Duration;

use crate::engine::Engine;

pub const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(30 * 60);

pub async fn scheduler_loop(engine: Engine, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if let Err(error) = engine.sync_all().await {
            tracing::warn!(%error, "scheduled sync failed");
        }
    }
}
```

- [ ] **Step 4: Wire scheduler into run() in src/lib.rs**

```rust
pub async fn run(config: Config) -> anyhow::Result<()> {
    config.ensure_data_dir()?;
    let engine = engine::Engine::new(&config).await?;
    tokio::spawn(engine::sync::scheduler_loop(engine.clone(), engine::sync::DEFAULT_SYNC_INTERVAL));
    let state = AppState { engine };
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("feedea {} listening on {}", crate::version(), listener.local_addr()?);
    axum::serve(listener, api::router(state)).await?;
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test engine::sync::tests`
Expected: PASS. Then `cargo test` full. Then smoke check: `cargo run -- --data-dir /tmp/feedea-phase2-smoke &` → `curl -s localhost:3000/api/health` → ok; kill.

- [ ] **Step 6: Commit**

```bash
git add src/engine/sync.rs src/lib.rs && git commit -m "Phase 2: add periodic sync scheduler"
```

---

## Self-Review Notes

- Spec coverage: Phase 2 deliverable = headless engine, add source, sync pulls articles, feeds + headlines readable, favicon/thumbnail endpoints. All present across Tasks 1-5. Not in this phase (Phase 3): auth, OPML, discovery fallback for homepage URLs, HTML rewrite/image proxy, saved/notes, search, categories CRUD.
- Placeholder scan: the `engine/sync.rs` stub is replaced in Task 5. All steps contain real code.
- Type consistency: `FeedSummary` used across engine + API; `ArticleFilter` field names match news-flash 3.2.0 (`order`, `order_by`, `feeds`, `categories`, `limit`, `offset`); `Read`/`Marked` compared via `==` against enum variants. `AppState { engine }` used in router signature throughout.
- Known accepted design decision (not a defect): async news-flash methods are awaited directly under the mutation lock; only sync `get_*`/counts and `create()` use spawn_blocking. Rationale documented in Global Constraints.
- `FeedServer` helper is duplicated between `tests/feed_server.rs` (integration tests) and the crate-internal `#[cfg(test)]` tests module (unit tests) because Rust cannot share code across test crates; intentional.
