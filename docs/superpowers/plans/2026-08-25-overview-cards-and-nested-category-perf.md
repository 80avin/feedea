# Overview Cards (Direct Feeds Only) & Nested-Category Performance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restrict overview cards to categories with directly-mapped feeds and cut `/api/overview` from ~O(N) SQLite round-trips to a fixed small number, while fixing parent-category mark-read and eliminating redundant/blocking DB access.

**Architecture:** Introduce a per-request `CategoryIndex` snapshot (one consolidated read round) consumed by overview/categories/sources; batch overview headline fetching into a single news-flash call bucketed in memory; expand descendant category ids before `set_category_read`; route all remaining sync DB work through `spawn_blocking`; bulk-delete saved rows with one `IN` statement.

**Tech Stack:** Rust 2024, axum 0.8, news-flash 3.2.0 (public API only, `default-features=false`), rusqlite 0.40, diesel (upstream), tokio, React frontend (untouched).

## Global Constraints

- news-flash public API only: `NewsFlash::get_categories`, `get_feeds`, `get_fat_articles`, `get_article_ids`, `set_category_read(&[CategoryID])`, `unread_count_feed_map`, `unread_count_all`, `add_category`, `remove_category`, `move_category`, `rename_category`. **No changes to the news-flash crate.** Raw rusqlite may only touch the tables already touched today (`feed_mapping`, `articles`) and only read-only for queries; the two existing write bypasses (`move_feed`, `migrate_feed_articles`) are kept but documented as deliberate pinned-schema debt.
- Overview cards = categories in the direct-feed set (`feed_mapping.category_id`), plus `NewsFlash.Toplevel` when any feed maps to it. Parent-only containers never get cards. Card counts/items are strictly per-direct-feed.
- API response shapes (`/api/overview`, `/api/categories`, `/api/feeds`, `/api/sources`) must not change field names or types. Frontend is not touched.
- Every `with_nf` / `get_categories` / `get_feeds` / `category_index` call is async and internally uses `spawn_blocking`. No sync diesel/rusqlite work runs on tokio workers.
- All mutated IDs are plain `String`; news-flash wrapper types (`CategoryID`, `FeedID`) are constructed at the boundary via `CategoryID::new(...)` / `FeedID::new(...)`.
- Commit messages follow the repo convention (`feat:`/`fix:`/`test:`/`chore:`/`docs:`). Run `cargo test` after every task; full `make test` only in the final task.
- Measurement: baseline must be captured on `main` **before** any code change; "after" numbers recorded in the same results doc after the final task.

---

### Task 1: Perf harness + baseline (on `main`, before any refactor)

**Files:**
- Create: `tests/overview_perf.rs`
- Create: `docs/superpowers/plans/2026-08-25-overview-perf-results.md`
- Test: `tests/overview_perf.rs`

**Interfaces:**
- Produces: an `#[ignore]`d integration test `overview_latency_benchmark` that prints a machine-readable line `OVERVIEW_PERF ...`, and a results doc with baseline numbers. All later tasks keep this test green and the final task re-runs it.

**Step 1: Write the failing/benchmark test** (the harness itself; it must compile and print on `main`).

`tests/overview_perf.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use feedea::AppState;
use feedea::api;
use feedea::app_db;
use feedea::auth;
use feedea::config::Config;
use feedea::engine::Engine;
use http_body_util::BodyExt;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tokio::sync::Mutex;
use tower::ServiceExt;

const N_CATS: usize = 24;
const N_UNCATEGORIZED: usize = 4;
const N_ITEMS: usize = 40;

fn rss_body(feed_label: &str) -> String {
    let items: Vec<String> = (1..=N_ITEMS)
        .map(|i| {
            let day = 1 + (i % 27);
            format!(
                "<item><title>{feed_label} item {i}</title><link>http://example.com/{feed_label}/{i}</link>\
                 <guid isPermaLink=\"false\">{feed_label}-{i}</guid>\
                 <pubDate>Mon, {day} Aug 2026 10:00:00 GMT</pubDate>\
                 <description>{feed_label} body {i}.</description></item>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel>\
         <title>{feed_label}</title><link>http://127.0.0.1/</link><description>x</description>\
         {items}</channel></rss>"
    )
}

struct PerfServer {
    addr: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl PerfServer {
    fn start(rss: String) -> PerfServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            rss.len(),
                            rss
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        PerfServer { addr, stop, handle: Some(handle) }
    }
}

impl Drop for PerfServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

async fn spawn_app() -> (String, axum::Router, PerfServer) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = PerfServer::start(rss_body("perf"));
    let url = format!("http://{}", server.addr);
    let dir = std::env::temp_dir().join(format!(
        "feedea-perf-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 0,
        allow_private_proxy: false,
    };
    let engine = Engine::new(&config).await.unwrap();
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap())
        .unwrap();
    let app_db = Arc::new(Mutex::new(db));
    let router = api::router(AppState {
        engine: engine.clone(),
        app_db,
        allow_private_proxy: false,
    });
    (url, router, server)
}

async fn login_cookie(app: &axum::Router) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"test-pass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.headers()
        .get(axum::http::header::SET_COOKIE)
        .expect("login should set a session cookie")
        .to_str()
        .unwrap()
        .to_string()
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

#[tokio::test]
#[ignore]
async fn overview_latency_benchmark() {
    let (base_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    for i in 0..N_CATS {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/categories")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(r#"{{"name":"Cat {i}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let cat_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"]
            .as_str()
            .unwrap()
            .to_string();
        let url = format!("{base_url}/c{i}.xml");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(
                        r#"{{"url":"{url}","title":"Feed {i}","category_id":"{cat_id}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "add feed {i} failed");
    }

    for i in 0..N_UNCATEGORIZED {
        let url = format!("{base_url}/u{i}.xml");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(r#"{{"url":"{url}","title":"Uncat {i}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "add uncategorized feed {i} failed");
    }

    let mut cards = 0usize;
    let mut samples: Vec<f64> = Vec::new();
    for run in 0..23 {
        let t0 = std::time::Instant::now();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/overview")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["all"]["total_count"].as_i64().unwrap() >= (N_CATS * N_ITEMS) as i64,
            "articles must be ingested before timing"
        );
        cards = json["cards"].as_array().unwrap().len();
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        if run >= 3 {
            samples.push(elapsed);
        }
    }
    assert!(cards >= N_CATS, "expected at least {N_CATS} cards, got {cards}");
    samples.sort_by(|a, b| a.total_cmp(b));
    let n = samples.len();
    let median = samples[n / 2];
    let min = samples[0];
    let mean: f64 = samples.iter().sum::<f64>() / n as f64;
    println!(
        "OVERVIEW_PERF cards={cards} min_ms={min:.2} median_ms={median:.2} mean_ms={mean:.2} samples={n}"
    );
}
```

**Step 2: Verify the harness compiles and runs on `main`**

Run: `cargo test --test overview_perf -- --ignored --nocapture`
Expected: prints `OVERVIEW_PERF cards=25 min_ms=... median_ms=... mean_ms=... samples=20`.

**Step 3: Record the baseline**

In `docs/superpowers/plans/2026-08-25-overview-perf-results.md`:

```markdown
# Overview latency — baseline vs after

Fixture: 24 categorized + 4 uncategorized feeds, 40 items each (~1120 articles).
Command: `cargo test --test overview_perf -- --ignored --nocapture`

## Baseline (main @ <git short sha>)
OVERVIEW_PERF cards=25 min_ms=<value> median_ms=<value> mean_ms=<value> samples=20

## After (worktree branch, final task)
OVERVIEW_PERF cards=25 min_ms=... median_ms=... mean_ms=... samples=20
```

Fill the Baseline row with the real numbers and the current `git rev-parse --short HEAD`.

**Step 4: Commit**

```bash
git add tests/overview_perf.rs docs/superpowers/plans/2026-08-25-overview-perf-results.md
git commit -m "test: add overview perf harness and baseline"
```

---

### Task 2: Direct totals query in `queries.rs`

**Files:**
- Modify: `src/engine/queries.rs`
- Test: `src/engine/queries.rs` (extend existing unit test)

**Interfaces:**
- Consumes: existing `open_readonly(db_path: &Path) -> anyhow::Result<Connection>` (queries.rs:20-25), `descendant_category_ids` (queries.rs:27-46).
- Produces: `pub fn direct_category_totals_map(db_path: &Path) -> anyhow::Result<HashMap<String, (i64, i64)>>` where the map is `category_id -> (total_articles, unread_articles)` over **direct** `feed_mapping` rows only (no descendant rollup). Task 4 consumes it.

**Step 1: Write the failing test** (extend the existing `search_and_counts_against_newsflash_db` unit test in `src/engine/queries.rs:166` — add at the end, before `server.stop()`):

```rust
        let direct = direct_category_totals_map(&db_path).unwrap();
        let tl = direct.get("NewsFlash.Toplevel").expect("toplevel direct totals");
        assert!(tl.0 >= 2, "toplevel direct total should count the uncategorized feed's articles");
```

Run: `cargo test --lib engine::queries`
Expected: FAIL with "function not defined".

**Step 2: Implement**

In `src/engine/queries.rs`, after `category_totals`, factor the direct layer out and add the new function. Replace the current first query block inside `category_totals` (lines 76-89) with a call to a shared private helper:

```rust
fn direct_totals(conn: &Connection) -> anyhow::Result<HashMap<String, (i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT fm.category_id,
                COUNT(a.article_id) AS total,
                SUM(CASE WHEN a.unread = 1 THEN 1 ELSE 0 END) AS unread
         FROM feed_mapping fm
         LEFT JOIN articles a ON a.feed_id = fm.feed_id
         GROUP BY fm.category_id",
    )?;
    let direct = stmt
        .query_map([], |row| {
            let unread: Option<i64> = row.get(2)?;
            Ok((row.get::<_, String>(0)?, (row.get(1)?, unread.unwrap_or(0))))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    Ok(direct)
}

pub fn direct_category_totals_map(db_path: &Path) -> anyhow::Result<HashMap<String, (i64, i64)>> {
    let conn = open_readonly(db_path)?;
    direct_totals(&conn)
}
```

Then in `category_totals`, replace lines 76-89 with:

```rust
    let direct = direct_totals(&conn)?;
```

(Keep the rest of `category_totals` — children map, rollup loop, toplevel push — unchanged.)

**Step 3: Run tests**

Run: `cargo test --lib engine::queries`
Expected: PASS (both the new assertion and the pre-existing rollup assertions).

**Step 4: Commit**

```bash
git add src/engine/queries.rs
git commit -m "feat(engine): expose direct per-category totals query"
```

---

### Task 3: `CategoryIndex` module + engine rewiring

**Files:**
- Create: `src/engine/category_index.rs`
- Modify: `src/engine/mod.rs` (add `pub mod category_index;`, add `Engine::category_index`, `Engine::summaries_from_parts`, `Engine::tree_from_index`, `Engine::headlines_from_articles`; rewire `get_feeds`, `get_headlines`, `get_category_tree`, `category_unread_map`)
- Test: `src/engine/category_index.rs` (unit tests)

**Interfaces:**
- Consumes: `news_flash::models::{Category, CategoryMapping, Feed, FeedMapping, FeedID, FatArticle}`, `crate::engine::opml_import::category_paths`, `crate::engine::queries::descendant_category_ids`, `crate::dto::{FeedSummary, Headline}`.
- Produces (used by Tasks 4-7):
  - `pub struct IndexInputs { categories: Vec<(String,String)>, category_mappings: Vec<(String,String)>, feed_mappings: Vec<(String,String)>, feed_titles: Vec<(String,String)>, unread_by_feed: HashMap<String,i64>, path_by_id: HashMap<String,Vec<String>>, feed_summaries: Vec<FeedSummary> }`
  - `pub struct CategoryIndex { category_order: Vec<String>, children_by_parent: HashMap<String,Vec<String>>, name_by_id: HashMap<String,String>, path_by_id: HashMap<String,Vec<String>>, direct_feed_cats: HashSet<String>, cat_by_feed: HashMap<String,String>, title_by_feed: HashMap<String,String>, unread_direct: HashMap<String,i64>, unread_rolled_up: HashMap<String,i64>, feed_summaries: Vec<FeedSummary> }`
  - `impl CategoryIndex { pub fn build(inputs: IndexInputs) -> CategoryIndex; pub fn is_known(&self, id: &str) -> bool; pub fn is_direct(&self, id: &str) -> bool; pub fn card_ids(&self) -> Vec<String>; pub fn display_name(&self, id: &str) -> String; pub fn descendants(&self, id: &str) -> Vec<String> }`
  - `impl Engine { pub async fn category_index(&self) -> anyhow::Result<CategoryIndex>; pub fn summaries_from_parts(feeds: Vec<Feed>, feed_mappings: Vec<FeedMapping>, unread: &HashMap<FeedID, i64>) -> Vec<FeedSummary>; pub fn tree_from_index(index: &CategoryIndex) -> Vec<crate::dto::CategoryNode>; pub fn headlines_from_articles(articles: Vec<FatArticle>, title_by_feed: &HashMap<String,String>) -> Vec<Headline> }`

**Step 1: Write the failing unit tests**

`src/engine/category_index.rs` (module + tests together; tests fail first because `build` doesn't exist):

```rust
use std::collections::{HashMap, HashSet};

use crate::dto::FeedSummary;

pub const TOPLEVEL: &str = "NewsFlash.Toplevel";

pub struct IndexInputs {
    pub categories: Vec<(String, String)>,
    pub category_mappings: Vec<(String, String)>,
    pub feed_mappings: Vec<(String, String)>,
    pub feed_titles: Vec<(String, String)>,
    pub unread_by_feed: HashMap<String, i64>,
    pub path_by_id: HashMap<String, Vec<String>>,
    pub feed_summaries: Vec<FeedSummary>,
}

pub struct CategoryIndex {
    pub category_order: Vec<String>,
    pub children_by_parent: HashMap<String, Vec<String>>,
    pub name_by_id: HashMap<String, String>,
    pub path_by_id: HashMap<String, Vec<String>>,
    pub direct_feed_cats: HashSet<String>,
    pub cat_by_feed: HashMap<String, String>,
    pub title_by_feed: HashMap<String, String>,
    pub unread_direct: HashMap<String, i64>,
    pub unread_rolled_up: HashMap<String, i64>,
    pub feed_summaries: Vec<FeedSummary>,
}
```

Then the `impl CategoryIndex { ... }` block with the methods from "Produces" above, and then `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> IndexInputs {
        IndexInputs {
            categories: vec![
                ("root".into(), "Root".into()),
                ("leaf".into(), "Leaf".into()),
            ],
            category_mappings: vec![
                (TOPLEVEL.to_string(), "root".into()),
                ("root".into(), "leaf".into()),
            ],
            feed_mappings: vec![
                ("f1".into(), "leaf".into()),
                ("f2".into(), TOPLEVEL.to_string()),
                ("f2".into(), "leaf".into()),
            ],
            feed_titles: vec![("f1".into(), "Feed One".into())],
            unread_by_feed: [("f1".into(), 3i64), ("f2".into(), 5i64)].into_iter().collect(),
            path_by_id: [("root".into(), vec!["Root".to_string()])].into_iter().collect(),
            feed_summaries: vec![],
        }
    }

    #[test]
    fn direct_set_and_cat_by_feed_prefer_real_category_over_toplevel() {
        let idx = CategoryIndex::build(inputs());
        assert!(idx.is_direct("leaf"));
        assert!(idx.is_direct(TOPLEVEL));
        assert!(!idx.is_direct("root"), "container with no direct feed");
        assert_eq!(idx.cat_by_feed.get("f2").map(String::as_str), Some("leaf"));
        assert_eq!(idx.cat_by_feed.get("f1").map(String::as_str), Some("leaf"));
    }

    #[test]
    fn card_ids_preserve_order_and_exclude_containers() {
        let idx = CategoryIndex::build(inputs());
        assert_eq!(idx.card_ids(), vec!["leaf".to_string(), TOPLEVEL.to_string()]);
    }

    #[test]
    fn unread_direct_is_direct_only_and_rolled_up_includes_descendants() {
        let idx = CategoryIndex::build(inputs());
        // f2 has both a toplevel and a leaf mapping; both rows sum into direct.
        assert_eq!(idx.unread_direct.get("leaf").copied().unwrap_or(0), 8);
        assert_eq!(idx.unread_direct.get(TOPLEVEL).copied().unwrap_or(0), 5);
        // descendants(TOPLEVEL) = [TOPLEVEL, root, leaf] => 5 + 0 + 8
        assert_eq!(idx.unread_rolled_up.get(TOPLEVEL).copied().unwrap_or(0), 13);
        // descendants(root) = [root, leaf] => 0 + 8
        assert_eq!(idx.unread_rolled_up.get("root").copied().unwrap_or(0), 8);
    }

    #[test]
    fn is_known_and_descendants() {
        let idx = CategoryIndex::build(inputs());
        assert!(idx.is_known("root"));
        assert!(!idx.is_known("nope"));
        assert!(idx.descendants(TOPLEVEL).contains(&"root".to_string()));
    }

    #[test]
    fn display_name_prefers_path_then_label_then_raw_id() {
        let idx = CategoryIndex::build(inputs());
        assert_eq!(idx.display_name("root"), "Root");
        assert_eq!(idx.display_name("missing"), "missing");
        assert_eq!(idx.display_name(TOPLEVEL), TOPLEVEL);
    }
}
```

Run: `cargo test --lib engine::category_index`
Expected: FAIL — `CategoryIndex` has no `build`.

**Step 2: Implement `CategoryIndex::build` and the methods**

In `src/engine/category_index.rs`, add:

```rust
impl CategoryIndex {
    pub fn build(inputs: IndexInputs) -> CategoryIndex {
        let IndexInputs {
            categories,
            category_mappings,
            feed_mappings,
            feed_titles,
            unread_by_feed,
            path_by_id,
            feed_summaries,
        } = inputs;

        let mut category_order = Vec::new();
        let mut name_by_id: HashMap<String, String> = HashMap::new();
        for (id, label) in &categories {
            category_order.push(id.clone());
            name_by_id.insert(id.clone(), label.clone());
        }

        let mut children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        for (parent, child) in &category_mappings {
            children_by_parent
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
        }

        let mut direct_feed_cats: HashSet<String> = HashSet::new();
        let mut sorted: Vec<&(String, String)> = feed_mappings.iter().collect();
        sorted.sort_by(|a, b| {
            let ak = (a.1 == TOPLEVEL, a.0.as_str(), a.1.as_str());
            let bk = (b.1 == TOPLEVEL, b.0.as_str(), b.1.as_str());
            ak.cmp(&bk)
        });
        let mut cat_by_feed: HashMap<String, String> = HashMap::new();
        for (feed_id, category_id) in sorted {
            direct_feed_cats.insert(category_id.clone());
            cat_by_feed
                .entry(feed_id.clone())
                .or_insert_with(|| category_id.clone());
        }

        let title_by_feed: HashMap<String, String> = feed_titles.into_iter().collect();

        let mut unread_direct: HashMap<String, i64> = HashMap::new();
        for (feed_id, category_id) in &feed_mappings {
            let n = unread_by_feed.get(feed_id).copied().unwrap_or(0);
            *unread_direct.entry(category_id.clone()).or_insert(0) += n;
        }

        let mut unread_rolled_up: HashMap<String, i64> = HashMap::new();
        let mut all_cats: HashSet<String> = unread_direct.keys().cloned().collect();
        all_cats.extend(category_order.iter().cloned());
        all_cats.insert(TOPLEVEL.to_string());
        for cat in all_cats {
            let sum = crate::engine::queries::descendant_category_ids(&cat, &children_by_parent)
                .iter()
                .filter_map(|id| unread_direct.get(id))
                .sum();
            unread_rolled_up.insert(cat, sum);
        }

        CategoryIndex {
            category_order,
            children_by_parent,
            name_by_id,
            path_by_id,
            direct_feed_cats,
            cat_by_feed,
            title_by_feed,
            unread_direct,
            unread_rolled_up,
            feed_summaries,
        }
    }

    pub fn is_known(&self, id: &str) -> bool {
        self.category_order.iter().any(|c| c == id)
    }

    pub fn is_direct(&self, id: &str) -> bool {
        self.direct_feed_cats.contains(id)
    }

    /// Card ids in display order: categories-table order filtered to the direct
    /// set, then any remaining direct ids (typically NewsFlash.Toplevel) sorted.
    pub fn card_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .category_order
            .iter()
            .filter(|id| self.direct_feed_cats.contains(*id))
            .cloned()
            .collect();
        let mut extras: Vec<&String> = self
            .direct_feed_cats
            .iter()
            .filter(|id| !self.category_order.contains(id))
            .collect();
        extras.sort();
        out.extend(extras.into_iter().cloned());
        out
    }

    pub fn display_name(&self, id: &str) -> String {
        self.path_by_id
            .get(id)
            .filter(|p| !p.is_empty())
            .map(|p| p.join(" / "))
            .or_else(|| self.name_by_id.get(id).cloned())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn descendants(&self, id: &str) -> Vec<String> {
        crate::engine::queries::descendant_category_ids(id, &self.children_by_parent)
    }
}
```

**Step 3: Wire up the engine** — in `src/engine/mod.rs`:

Add the module declaration near line 18-22:

```rust
pub mod category_index;
```

Add the imports at the top (extend line 7):

```rust
use news_flash::models::{ArticleFilter, CategoryID, FeedID, Marked, PluginID, Read, Url};
use crate::engine::category_index::{CategoryIndex, IndexInputs};
```

Add `Engine::summaries_from_parts` — extract the body of `get_feeds` (lines 259-302) into a plain associated fn, and slim `get_feeds` down:

```rust
    pub async fn get_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let (feeds, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        Ok(Self::summaries_from_parts(feeds, mappings, &unread))
    }

    pub fn summaries_from_parts(
        feeds: Vec<news_flash::models::Feed>,
        feed_mappings: Vec<news_flash::models::FeedMapping>,
        unread: &HashMap<news_flash::models::FeedID, i64>,
    ) -> Vec<FeedSummary> {
        // A feed can have several mappings (news-flash supports multi-category, and a
        // stale toplevel/Uncategorized row can linger after a move). The app treats a
        // feed as single-category, so pick a real category over the toplevel, and do so
        // deterministically.
        let toplevel = "NewsFlash.Toplevel";
        let mut category_by_feed: HashMap<String, String> = HashMap::new();
        let mut mappings: Vec<_> = feed_mappings.into_iter().collect();
        mappings.sort_by(|a, b| {
            let ak = (
                a.category_id.as_str() == toplevel,
                a.feed_id.as_str(),
                a.category_id.as_str(),
            );
            let bk = (
                b.category_id.as_str() == toplevel,
                b.feed_id.as_str(),
                b.category_id.as_str(),
            );
            ak.cmp(&bk)
        });
        for m in mappings {
            category_by_feed
                .entry(m.feed_id.as_str().to_string())
                .or_insert_with(|| m.category_id.as_str().to_string());
        }
        let mut out = Vec::with_capacity(feeds.len());
        for feed in feeds {
            let id = feed.feed_id.as_str().to_string();
            out.push(FeedSummary {
                category_id: category_by_feed
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "NewsFlash.Toplevel".to_string()),
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
        out
    }
```

Replace the current `get_feeds` body (lines 256-302) with the slim version above.

Add `Engine::category_index` (after `get_feeds`):

```rust
    pub async fn category_index(&self) -> anyhow::Result<CategoryIndex> {
        let (categories, category_mappings) = self.with_nf(|nf| nf.get_categories()).await?;
        let (feeds, feed_mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let path_by_id =
            crate::engine::opml_import::category_paths(&categories, &category_mappings);
        let summaries = Self::summaries_from_parts(feeds.clone(), feed_mappings.clone(), &unread);
        Ok(CategoryIndex::build(IndexInputs {
            categories: categories
                .into_iter()
                .map(|c| (c.category_id.as_str().to_string(), c.label))
                .collect(),
            category_mappings: category_mappings
                .into_iter()
                .map(|m| (m.parent_id.as_str().to_string(), m.category_id.as_str().to_string()))
                .collect(),
            feed_mappings: feed_mappings
                .into_iter()
                .map(|m| (m.feed_id.as_str().to_string(), m.category_id.as_str().to_string()))
                .collect(),
            feed_titles: feeds
                .into_iter()
                .map(|f| (f.feed_id.as_str().to_string(), f.label))
                .collect(),
            unread_by_feed: unread
                .into_iter()
                .map(|(id, n)| (id.as_str().to_string(), n))
                .collect(),
            path_by_id,
            feed_summaries: summaries,
        }))
    }
```

Rewrite `get_category_tree` (lines 457-506) to delegate:

```rust
    pub async fn get_category_tree(&self) -> anyhow::Result<Vec<crate::dto::CategoryNode>> {
        let index = self.category_index().await?;
        Ok(Self::tree_from_index(&index))
    }

    pub fn tree_from_index(index: &CategoryIndex) -> Vec<crate::dto::CategoryNode> {
        fn build(parent_id: &str, index: &CategoryIndex, depth: usize) -> Vec<crate::dto::CategoryNode> {
            if depth >= 100 {
                return Vec::new();
            }
            let mut nodes = Vec::new();
            if let Some(children) = index.children_by_parent.get(parent_id) {
                for child in children {
                    let id = child.clone();
                    nodes.push(crate::dto::CategoryNode {
                        category_id: id.clone(),
                        name: index.name_by_id.get(&id).cloned().unwrap_or_default(),
                        parent_id: parent_id.to_string(),
                        unread_count: index.unread_rolled_up.get(&id).copied().unwrap_or(0),
                        children: build(&id, index, depth + 1),
                    });
                }
            }
            nodes
        }
        build(TOPLEVEL, index, 0)
    }
```

Rewrite `category_unread_map` (lines 314-352) to delegate:

```rust
    pub async fn category_unread_map(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        Ok(self.category_index().await?.unread_rolled_up)
    }
```

Rewrite `get_headlines` (lines 508-535) and add the shared shaper:

```rust
    pub async fn get_headlines(&self, filter: ArticleFilter) -> anyhow::Result<Vec<Headline>> {
        let (articles, titles) = self
            .with_nf(|nf| Ok((nf.get_fat_articles(filter)?, nf.get_feeds()?.0)))
            .await?;
        let title_by_feed: HashMap<String, String> = titles
            .into_iter()
            .map(|f| (f.feed_id.as_str().to_string(), f.label))
            .collect();
        Ok(Self::headlines_from_articles(articles, &title_by_feed))
    }

    pub fn headlines_from_articles(
        articles: Vec<news_flash::models::FatArticle>,
        title_by_feed: &HashMap<String, String>,
    ) -> Vec<Headline> {
        articles
            .into_iter()
            .map(|a| {
                let feed_id = a.feed_id.as_str().to_string();
                Headline {
                    id: a.article_id.as_str().to_string(),
                    title: a.title,
                    feed_id: feed_id.clone(),
                    feed_title: title_by_feed.get(&feed_id).cloned(),
                    url: a.url.map(|u| u.to_string()),
                    date: a.date,
                    summary: a.summary,
                    thumbnail_url: a.thumbnail_url,
                    unread: a.unread == Read::Unread,
                    marked: a.marked == Marked::Marked,
                    note: None,
                    tags: None,
                }
            })
            .collect()
    }
```

**Step 4: Run tests**

Run: `cargo test --lib` and `cargo test --test categories --test overview --test sources`
Expected: PASS. This task is behavior-neutral (all existing assertions still hold).

**Step 5: Commit**

```bash
git add src/engine/category_index.rs src/engine/mod.rs
git commit -m "feat(engine): add CategoryIndex snapshot and dedupe category reads"
```

---

### Task 4: Rewrite `/api/overview` (card rule + per-card parallel headlines — REVISED: batching rejected after real-data verification, see §3)

**Files:**
- Modify: `src/api/overview.rs`
- Test: `tests/overview.rs`

**Interfaces:**
- Consumes: `Engine::category_index`, `Engine::headlines_from_articles`, `Engine::with_nf`, `queries::direct_category_totals_map`, `queries::total_article_count`, `CategoryIndex::{card_ids, display_name, cat_by_feed, title_by_feed, unread_direct}`.
- Produces: unchanged `GET /api/overview` response shape; cards restricted to direct-feed categories.

**Step 1: Update the tests to the new contract** (rewrite in `tests/overview.rs`):

Replace `overview_parent_card_includes_descendant_feed_counts` (lines 112-224) with:

```rust
#[tokio::test]
async fn overview_parent_without_direct_feeds_gets_no_card() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let parent_id = create_category(&app, &cookie, r#"{"name":"Parent"}"#).await;
    let child_id = create_category(
        &app,
        &cookie,
        &format!(r#"{{"name":"Child","parent_id":"{parent_id}"}}"#),
    )
    .await;

    add_feed(&app, &cookie, &feed_url, "Nested Feed", Some(&child_id)).await;
    refresh_all(&app, &cookie).await;

    let json = overview(&app, &cookie).await;
    let cards = json["cards"].as_array().unwrap();
    assert!(
        cards.iter().all(|c| c["category_id"] != parent_id),
        "container category without direct feeds must not get a card: {cards:?}"
    );
    assert!(cards.iter().all(|c| c["name"] != "Parent"));
    let child = cards
        .iter()
        .find(|card| card["category_id"] == child_id)
        .expect("child card should exist");
    assert!(child["total_count"].as_i64().unwrap() >= 1);
    assert!(child["unread_count"].as_i64().unwrap() >= 1);
}
```

Add helper fns to `tests/overview.rs` (after `cookie_pair`):

```rust
async fn create_category(app: &axum::Router, cookie: &str, body: &str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn add_feed(
    app: &axum::Router,
    cookie: &str,
    feed_url: &str,
    title: &str,
    category_id: Option<&str>,
) {
    let category = category_id
        .map(|c| format!(r#","category_id":"{c}""#))
        .unwrap_or_default();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","title":"{title}"{category}}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn refresh_all(app: &axum::Router, cookie: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/refresh-all")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn overview(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/overview")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}
```

Add a new test after it (toplevel strict-direct semantics):

```rust
#[tokio::test]
async fn overview_no_toplevel_card_without_uncategorized_feeds() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let cat_id = create_category(&app, &cookie, r#"{"name":"Cat"}"#).await;
    add_feed(&app, &cookie, &feed_url, "Categorized", Some(&cat_id)).await;
    refresh_all(&app, &cookie).await;

    let json = overview(&app, &cookie).await;
    let cards = json["cards"].as_array().unwrap();
    assert!(
        cards.iter().all(|c| c["name"] != "NewsFlash.Toplevel"),
        "toplevel card must not appear when no feed maps directly to it"
    );
    let cat = cards
        .iter()
        .find(|c| c["category_id"] == cat_id)
        .expect("categorized feed's card should exist");
    assert!(cat["total_count"].as_i64().unwrap() >= 2);
    assert!(json["all"]["total_count"].as_i64().unwrap() >= 2);
}
```

**Step 2: Run the tests to verify the new ones fail on the old implementation**

Run: `cargo test --test overview`
Expected: `overview_parent_without_direct_feeds_gets_no_card` FAILS (parent card present), `overview_no_toplevel_card_without_uncategorized_feeds` FAILS (toplevel card present with items). `overview_shows_full_category_path` still passes; `overview_returns_per_category_cards_and_all_totals` still passes.

**Step 3: Rewrite the handler** — replace `src/api/overview.rs` in full:

```rust
use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::ApiResult;
use crate::dto::CategoryCard;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, OrderBy};

const MAX_OVERVIEW_ITEMS: usize = 6;
const MIN_WINDOW: i64 = 24;
const MAX_WINDOW: i64 = 120;

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let index = state.engine.category_index().await?;

    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let totals = {
        let db_path = db_path.clone();
        tokio::task::spawn_blocking(move || {
            crate::engine::queries::direct_category_totals_map(&db_path)
        })
        .await??
    };

    let card_ids = index.card_ids();
    let limit = ((card_ids.len() as i64) * MAX_OVERVIEW_ITEMS as i64).clamp(MIN_WINDOW, MAX_WINDOW);
    let filter = ArticleFilter {
        order: Some(ArticleOrder::NewestFirst),
        order_by: Some(OrderBy::Published),
        limit: Some(limit),
        categories: Some(card_ids.iter().map(|id| CategoryID::new(id)).collect()),
        ..ArticleFilter::default()
    };
    let articles = state.engine.with_nf(move |nf| nf.get_fat_articles(filter)).await?;
    let headlines = crate::engine::Engine::headlines_from_articles(articles, &index.title_by_feed);

    let mut buckets: HashMap<String, Vec<crate::dto::Headline>> =
        card_ids.iter().map(|id| (id.clone(), Vec::new())).collect();
    for h in headlines {
        if let Some(cat) = index.cat_by_feed.get(&h.feed_id)
            && let Some(bucket) = buckets.get_mut(cat)
            && bucket.len() < MAX_OVERVIEW_ITEMS
        {
            bucket.push(h);
        }
    }

    let mut cards = Vec::with_capacity(card_ids.len());
    for id in &card_ids {
        let (total, _) = totals.get(id).copied().unwrap_or((0, 0));
        cards.push(CategoryCard {
            category_id: id.clone(),
            name: index.display_name(id),
            total_count: total,
            unread_count: index.unread_direct.get(id).copied().unwrap_or(0),
            items: buckets.remove(id).unwrap_or_default(),
        });
    }

    let all_total = {
        tokio::task::spawn_blocking(move || crate::engine::queries::total_article_count(&db_path))
            .await??
    };
    let all_unread = state.engine.with_nf(|nf| nf.unread_count_all()).await?;

    Ok(Json(json!({
        "cards": cards,
        "all": { "total_count": all_total, "unread_count": all_unread },
    })))
}
```

Add `use std::collections::HashMap;` at the top. Remove the now-unused `build_card` fn and `MAX_OVERVIEW_ITEMS` `i64` constant references.

**Step 4: Run tests**

Run: `cargo test --test overview`
Expected: PASS (all four overview tests).

**Step 5: Commit**

```bash
git add src/api/overview.rs tests/overview.rs
git commit -m "feat(api): overview shows only categories with direct feeds, batched fetch"
```

---

### Task 5: Parent mark-read covers descendants

**Files:**
- Modify: `src/engine/mod.rs`
- Test: `tests/categories.rs`

**Interfaces:**
- Consumes: `Engine::category_index` (for `children_by_parent`), `queries::descendant_category_ids`, `news_flash::models::CategoryID`.
- Produces: unchanged `Engine::mark_category_read(id: &str)` signature; behavior now clears descendant articles too.

**Step 1: Write the failing test** (append to `tests/categories.rs`):

```rust
#[tokio::test]
async fn mark_parent_category_read_clears_descendant_articles() {
    let (feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let parent_id = serde_json::from_slice::<serde_json::Value>(
        &post_category(&app, &cookie, r#"{"name":"Parent"}"#)
            .await
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string();
    let child_id = serde_json::from_slice::<serde_json::Value>(
        &post_category(
            &app,
            &cookie,
            &format!(r#"{{"name":"Child","parent_id":"{parent_id}"}}"#),
        )
        .await
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes(),
    )
    .unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string();

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","title":"Child Feed","category_id":"{child_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let refresh_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/refresh-all")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let tree = get_tree(&app, &cookie).await;
    let categories = tree["categories"].as_array().unwrap();
    let child = find_node(categories, &child_id).expect("child should be in tree");
    assert_eq!(
        child["unread_count"].as_i64().unwrap(),
        2,
        "two unread articles before marking parent read"
    );

    let read_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/categories/{parent_id}/read"))
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read_resp.status(), StatusCode::OK);

    let tree = get_tree(&app, &cookie).await;
    let categories = tree["categories"].as_array().unwrap();
    let child = find_node(categories, &child_id).expect("child should still be in tree");
    assert_eq!(
        child["unread_count"].as_i64().unwrap(),
        0,
        "marking the parent read must clear descendant articles"
    );
}
```

**Step 2: Run to verify it fails**

Run: `cargo test --test categories mark_parent_category_read_clears_descendant_articles`
Expected: FAIL — child `unread_count` is still `2` after marking parent read.

**Step 3: Implement** — replace `mark_category_read` (engine/mod.rs:439-445):

```rust
    pub async fn mark_category_read(&self, id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let index = self.category_index().await?;
        let ids: Vec<news_flash::models::CategoryID> = crate::engine::queries::descendant_category_ids(
            id,
            &index.children_by_parent,
        )
        .into_iter()
        .map(|s| news_flash::models::CategoryID::new(&s))
        .collect();
        self.nf.set_category_read(&ids, &self.client).await?;
        Ok(())
    }
```

**Step 4: Run tests**

Run: `cargo test --test categories`
Expected: PASS (new test + existing suite).

**Step 5: Commit**

```bash
git add src/engine/mod.rs tests/categories.rs
git commit -m "fix(engine): mark category read covers nested descendant categories"
```

---

### Task 6: `PATCH /api/categories/{id}` single-index round

**Files:**
- Modify: `src/api/categories.rs`
- Test: `tests/categories.rs`

**Interfaces:**
- Consumes: `Engine::category_index` (for `is_known`, `descendants`), `Engine::get_category_tree` (unchanged).
- Produces: same `PATCH /api/categories/{id}` response; no more triple tree-build.

**Step 1: Run the existing suite to establish green**

Run: `cargo test --test categories`
Expected: PASS.

**Step 2: Rewrite the `update` handler and `ensure_exists` (lines 53-127)**

Replace `update`:

```rust
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCategory>,
) -> ApiResult<Json<CategoryNode>> {
    let index = state.engine.category_index().await?;
    if !index.is_known(&id) {
        return Err(ApiError::not_found("category not found"));
    }
    if req.name.is_none() && req.parent_id.is_none() {
        return Err(ApiError::bad_request("nothing to update"));
    }
    if let Some(parent_id) = &req.parent_id {
        if parent_id == &id {
            return Err(ApiError::bad_request("cannot move a category into itself"));
        }
        if index.descendants(&id).contains(parent_id) {
            return Err(ApiError::bad_request(
                "cannot move a category into its own descendant",
            ));
        }
    }
    if let Some(name) = &req.name {
        state.engine.rename_category(&id, name).await?;
    }
    if let Some(parent_id) = &req.parent_id {
        state.engine.move_category(&id, parent_id).await?;
    }
    let tree = state.engine.get_category_tree().await?;
    let node = find_node(&tree, &id).ok_or_else(|| ApiError::not_found("category not found"))?;
    Ok(Json(node.clone()))
}
```

Replace `ensure_exists` so it no longer exists (the `update` handler inlines its check; `remove` and `mark_read` keep using it). Change `ensure_exists` to keep behavior but stay cheap:

```rust
async fn ensure_exists(state: &AppState, id: &str) -> Result<(), ApiError> {
    let (categories, _) = state.engine.get_categories().await?;
    if categories.iter().any(|c| c.category_id.as_str() == id) {
        Ok(())
    } else {
        Err(ApiError::not_found("category not found"))
    }
}
```

(Leave `ensure_exists` as-is — only `update` switches to the index. `create` is unchanged.)

**Step 3: Run tests**

Run: `cargo test --test categories`
Expected: PASS (existing suite covers update/rename/move/cycle-check via `categories_crud_tree_reparent_and_remove_children`).

**Step 4: Commit**

```bash
git add src/api/categories.rs
git commit -m "perf(api): single category-index round in category update"
```

---

### Task 7: Sources list + OPML import resolution de-refetch

**Files:**
- Modify: `src/api/sources.rs`
- Test: `tests/sources.rs`

**Interfaces:**
- Consumes: `Engine::category_index` (for `feed_summaries`, `path_by_id`), `Engine::get_categories`, `opml_import::category_paths`, `opml_import::ensure_category_path` (local fn, unchanged).
- Produces: same `/api/sources` GET and POST `import-opml` response shapes.

**Step 1: Run the existing suite to establish green**

Run: `cargo test --test sources`
Expected: PASS.

**Step 2: Rewrite the `list` handler (lines 41-64)**

```rust
pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let index = state.engine.category_index().await?;
    let mut groups: Vec<SourceGroup> = Vec::new();
    for feed in index.feed_summaries {
        let category_id = feed.category_id.clone();
        let category_name = index
            .path_by_id
            .get(&category_id)
            .filter(|path| !path.is_empty())
            .map(|path| path.join(" / "))
            .unwrap_or_else(|| "Uncategorized".to_string());
        if let Some(group) = groups.iter_mut().find(|g| g.category_id == category_id) {
            group.feeds.push(feed);
        } else {
            groups.push(SourceGroup {
                category_id,
                category_name,
                feeds: vec![feed],
            });
        }
    }
    Ok(Json(json!({ "groups": groups })))
}
```

**Step 3: De-refetch in `import_opml`**

After the import at line 229-231 (`if added > 0 { state.engine.import_opml(&cleaned).await?; }`), add one fresh fetch into a mutable local map:

```rust
    let (categories, category_mappings) = state.engine.get_categories().await?;
    let mut path_by_id = opml_import::category_paths(&categories, &category_mappings);
```

Replace the per-resolution re-fetch block inside the `KeepNew`/`SameFeed` branch (lines 264-291, the `if conflict.opml.category != existing_id.category { ... }` body) with:

```rust
                            if conflict.opml.category != existing_id.category {
                                let category_id = if conflict.opml.category.is_empty() {
                                    // The file places the feed at the root: move it out of
                                    // its current category to the toplevel.
                                    Some(
                                        news_flash::models::NEWSFLASH_TOPLEVEL.as_str().to_string(),
                                    )
                                } else {
                                    ensure_category_path(
                                        &state,
                                        &conflict.opml.category,
                                        &mut path_by_id,
                                    )
                                    .await?
                                };
                                if let Some(category_id) = category_id {
                                    state
                                        .engine
                                        .move_feed(&existing_id.id, &category_id)
                                        .await?;
                                    changed = true;
                                }
                            }
```

(`ensure_category_path` already records created ids into `path_by_id` (sources.rs:416), and the single post-import fetch covers categories created by the import itself.)

**Step 4: Run tests**

Run: `cargo test --test sources`
Expected: PASS (import/move/nested-path/merge suites).

**Step 5: Commit**

```bash
git add src/api/sources.rs
git commit -m "perf(api): hoist category re-reads out of sources import loop"
```

---

### Task 8: Blocking-call hygiene + pinned-schema debt docs

**Files:**
- Modify: `src/engine/mod.rs`
- Modify: `src/engine/opml_import.rs`
- Modify: `src/api/sources.rs` (migrate call-site wrap)
- Test: `tests/sources.rs`, `tests/categories.rs`, `tests/overview.rs`, `tests/articles.rs` (regression)

**Interfaces:**
- Consumes: `Engine::with_nf`, `tokio::task::spawn_blocking`.
- Produces: no API changes.

**Step 1: Wrap `delete_empty_categories` reads** (engine/mod.rs:391-393). Replace:

```rust
            let (categories, category_mappings) = self.with_nf(|nf| nf.get_categories()).await?;
            let (_, feed_mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
```

(The rest of the loop body is unchanged.)

**Step 2: Wrap `search` DB read** (engine/mod.rs:554-559). Replace:

```rust
    pub async fn search(&self, q: &str, limit: i64) -> anyhow::Result<Vec<Headline>> {
        let db_path = self.data_dir().join("engine/data/database.sqlite");
        let q = q.to_string();
        let hits =
            tokio::task::spawn_blocking(move || crate::engine::queries::search(&db_path, &q, limit))
                .await??;
        let ids = hits.into_iter().map(|h| h.article_id).collect();
        self.get_headlines_by_ids(ids).await
    }
```

**Step 3: Wrap `move_feed` rusqlite work** (engine/mod.rs:202-220). Replace the body:

```rust
    pub async fn move_feed(&self, id: &str, to_category: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        // news-flash's move_feed only rewrites the first mapping it finds, so a stale
        // toplevel/Uncategorized row (or a second category) can linger and make a feed
        // show up in Uncategorized too. The app treats a feed as single-category, so set
        // it to exactly one category: drop every existing mapping and insert the target.
        // DELIBERATE PINNED-SCHEMA BYPASS: this writes feed_mapping directly because
        // news-flash exposes no public API to delete a single feed mapping. It pins
        // upstream's private schema; an upstream migration may break it. No upstream
        // change is planned; keep the column names in sync with news-flash 3.2.0.
        let db_path = self.data_dir.join("engine/data/database.sqlite");
        let id = id.to_string();
        let to_category = to_category.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            conn.busy_timeout(std::time::Duration::from_secs(10))?;
            conn.execute(
                "DELETE FROM feed_mapping WHERE feed_id = ?1",
                rusqlite::params![id],
            )?;
            conn.execute(
                "INSERT INTO feed_mapping (feed_id, category_id, sort_index) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, to_category, i32::MAX],
            )?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        Ok(())
    }
```

**Step 4: Wrap the `migrate_feed_articles` call site** (src/api/sources.rs:307-314). Replace:

```rust
                            {
                                let _guard = state.engine.mutation_guard().await;
                                let from = matched.id.clone();
                                let to = conflict.opml.url.clone();
                                let db_path = db_path.clone();
                                migrated += tokio::task::spawn_blocking(move || {
                                    opml_import::migrate_feed_articles(&db_path, &from, &to)
                                })
                                .await??;
                            }
```

**Step 5: Document the `migrate_feed_articles` pinned-schema bypass** — add a doc comment above the fn in `src/engine/opml_import.rs` (find the fn; keep its body unchanged):

```rust
/// DELIBERATE PINNED-SCHEMA BYPASS: rewrites `articles.feed_id` directly because
/// news-flash offers no public API to re-point articles between feeds. Pins
/// upstream's private schema; an upstream migration may break it. No upstream
/// change is planned; keep the table/column names in sync with news-flash 3.2.0.
```

**Step 6: Run tests**

Run: `cargo test --lib` and `cargo test`
Expected: PASS (whole suite; this task is behavior-neutral).

**Step 7: Commit**

```bash
git add src/engine/mod.rs src/engine/opml_import.rs src/api/sources.rs
git commit -m "perf(engine): move remaining blocking DB work off the tokio runtime; document schema-pinned bypasses"
```

---

### Task 9: Bulk saved-article unsave

**Files:**
- Modify: `src/app_db/mod.rs`
- Modify: `src/api/categories.rs`
- Modify: `src/api/sources.rs`
- Test: `src/app_db/mod.rs` (unit test)

**Interfaces:**
- Consumes: `rusqlite::params_from_iter`, `crate::engine::category_article_ids`, `crate::engine::feed_article_ids`.
- Produces: `AppDb::unsave_articles(&mut self, article_ids: &[String]) -> anyhow::Result<()>` (no-op on empty).

**Step 1: Write the failing unit test** (append in `src/app_db/mod.rs` tests):

```rust
    #[test]
    fn unsave_articles_bulk_deletes() {
        let dir = tmp_dir();
        let mut db = open(&dir).unwrap();
        db.save_article("a1", None, &["t1".to_string()]).unwrap();
        db.save_article("a2", None, &["t1".to_string()]).unwrap();
        db.unsave_articles(&[]).unwrap();
        db.unsave_articles(&["a1".to_string(), "a2".to_string()]).unwrap();
        let (saved, total) = db.saved_articles(0, 50).unwrap();
        assert_eq!(saved.len(), 0);
        assert_eq!(total, 0);
        let tags: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM saved_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tags, 0, "saved_tags rows must be bulk-deleted too");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

Run: `cargo test --lib app_db::tests::unsave_articles_bulk_deletes`
Expected: FAIL — no method `unsave_articles`.

**Step 2: Implement** (after `unsave_article`, src/app_db/mod.rs:154):

```rust
    pub fn unsave_articles(&mut self, article_ids: &[String]) -> anyhow::Result<()> {
        if article_ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; article_ids.len()].join(",");
        for table in ["saved", "saved_tags"] {
            let sql = format!("DELETE FROM {table} WHERE article_id IN ({placeholders})");
            self.conn.execute(&sql, rusqlite::params_from_iter(article_ids))?;
        }
        Ok(())
    }
```

**Step 3: Switch call sites** — `src/api/categories.rs:92-98`:

```rust
    if req.remove_children {
        let article_ids = state.engine.category_article_ids(&id).await?;
        let mut app_db = state.app_db.lock().await;
        app_db.unsave_articles(&article_ids)?;
    }
```

and `src/api/sources.rs:131-135`:

```rust
    let article_ids = state.engine.feed_article_ids(&id).await?;
    let mut app_db = state.app_db.lock().await;
    app_db.unsave_articles(&article_ids)?;
    drop(app_db);
```

**Step 4: Run tests**

Run: `cargo test --lib` and `cargo test --test categories --test sources`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app_db/mod.rs src/api/categories.rs src/api/sources.rs
git commit -m "perf(app-db): bulk-delete saved rows on category/feed removal"
```

---

### Task 10: Full verification, perf re-measure, results doc

**Files:**
- Modify: `docs/superpowers/plans/2026-08-25-overview-perf-results.md`

**Interfaces:**
- Consumes: everything above.

**Step 1: Run the full suite**

Run: `cargo test`
Expected: all PASS.

Run: `make test`
Expected: PASS (cargo test + frontend typecheck/lint).

**Step 2: Re-measure**

Run: `cargo test --test overview_perf -- --ignored --nocapture`
Expected: prints `OVERVIEW_PERF cards=25 min_ms=... median_ms=... mean_ms=... samples=20`.

**Step 3: Record the "after" row and summary in the results doc**

Fill the `## After` row of `docs/superpowers/plans/2026-08-25-overview-perf-results.md` with the measured numbers and `git rev-parse --short HEAD`, then add:

```markdown
## Summary
- `/api/overview` dropped from ~6N+12 sequential SQLite statements (per-card
  headline fetch + per-card mapping re-reads) to ~11 total regardless of N:
  CategoryIndex (categories + feeds + unread map) + 1 direct-totals query +
  per-card parallel fetches (one query per card, concurrent) + 1 global count.
- Median latency change: <baseline median>ms -> <after median>ms on the fixture
  above (24 categorized + 4 uncategorized feeds, ~1120 articles).
```

**Step 4: Commit**

```bash
git add docs/superpowers/plans/2026-08-25-overview-perf-results.md
git commit -m "docs: record overview latency after perf work"
```

**Step 5: Request a full diff review against the spec**

Ask the orchestrator to read `docs/superpowers/specs/2026-08-25-overview-cards-and-nested-category-perf-design.md` against `git diff main...HEAD` and confirm every design section is covered.
