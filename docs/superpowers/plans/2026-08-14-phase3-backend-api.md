# Phase 3: Full Backend API — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the `/api/*` surface per spec §4: auth/session, overview, timeline with paging + search + suggestions, saved (notes/tags/months), categories CRUD, sources CRUD + OPML, settings, media endpoints, and the image proxy + HTML rewrite pass.

**Architecture:** Extends the Phase 2 `Engine` actor + `AppState`. Adds: an auth layer (argon2 password, session cookies) wrapping all `/api/*` except login/session; a read-only sidecar query module (`src/engine/queries.rs`) that opens news-flash's `database.sqlite` read-only (WAL-safe) to run the FTS search fix and per-category totals; app-DB-backed saved/notes/tags/settings/sessions; news-flash category/source CRUD + OPML via the engine's mutation guard; and a `GET /img` proxy + article HTML rewrite.

**Tech Stack:** adds argon2 0.5.3, rand 0.10.2 (session tokens), sha2 0.11 (token hashing), scraper 0.27 (HTML rewrite). Existing: axum 0.8.9, rusqlite 0.40.2, chrono, news-flash 3.2.0, reqwest 0.13.4.

## Global Constraints

- No comments in code unless a task explicitly shows them.
- All synchronous news-flash calls run via `with_nf`/`spawn_blocking`. Async news-flash mutations run under `engine.mutation_guard()`. Image decode/encode (favicon/thumbnail) and FTS matching are CPU-heavy → `spawn_blocking`.
- The read-only sidecar connection to news-flash's `database.sqlite` (WAL mode) is the ONLY way search and per-category totals work (news-flash FTS is broken upstream and has no count API). Keep all schema-coupling SQL in `src/engine/queries.rs`.
- `ArticleFilter.order` must be `Some(NewestFirst)` + `order_by = Some(Published)` whenever deterministic ordering is needed.
- DTOs in `src/dto.rs`. Error envelope `{error:{code,message}}` via `src/api/error.rs`.
- Session cookie: name `rssea_session`, HttpOnly, `SameSite=Lax`, path `/`.
- Default keep-articles: `None` (keep everything). Default sync interval: 30 min.
- All `/api/*` routes require a valid session EXCEPT `POST /api/login` and `GET /api/session`.
- Commit after each task with the exact message given.

---

### Task 1: Auth module — password setup, sessions, login/logout/session middleware

**Files:**
- Create: `src/auth.rs`
- Modify: `src/app_db/mod.rs` (sessions + password_hash accessors), `src/dto.rs`, `src/lib.rs` (add `pub mod auth;`), `src/api/mod.rs` (login/logout/session routes + middleware), `src/api/error.rs` (401/403 helpers)
- Modify: `Cargo.toml` (argon2, rand, sha2)
- Test: `tests/auth.rs` integration + unit tests in `src/auth.rs`

**Interfaces:**
- Consumes: `AppState`, `AppDb`, `Config`.
- Produces:
  - `src/auth.rs`:
    - `pub fn hash_password(password: &str) -> anyhow::Result<String>` (argon2, random salt)
    - `pub fn verify_password(password: &str, hash: &str) -> bool`
    - `pub fn generate_token() -> String` (32 random bytes, hex)
    - `pub fn sha256_hex(s: &str) -> String`
    - `pub async fn ensure_password_setup(state: &AppState) -> anyhow::Result<()>` — on startup, if no password_hash in settings, generates a random password, hashes + stores it, logs it to stdout ("Initial password: ...") so the user can log in once. Idempotent (only if absent).
  - AppDb additions (rusqlite): `create_session(&mut self, token_hash: &str, expires_at: &str) -> Result<()>`, `delete_session(&mut self, token_hash: &str) -> Result<()>`, `session_exists(&self, token_hash: &str) -> Result<bool>`, `get_setting`/`set_setting` (exist), `password_hash(&self) -> Result<Option<String>>`, `set_password_hash(&mut self, hash: &str) -> Result<()>`.
  - `src/api/mod.rs`: router gains `POST /api/login`, `POST /api/logout`, `GET /api/session`, and a middleware applied to all `/api/*` routes that checks the `rssea_session` cookie.
  - `AppState` gains `pub app_db: std::sync::Arc<tokio::sync::Mutex<app_db::AppDb>>`.

- [ ] **Step 1: Add deps + AppDb accessors**

Cargo.toml deps: `argon2 = "0.5.3"`, `rand = "0.10.2"`, `sha2 = "0.11.0"`.

In `src/app_db/mod.rs` add methods (use `chrono::Utc::now()` string for timestamps):

```rust
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

    pub fn password_hash(&self) -> anyhow::Result<Option<String>> {
        self.get_setting("password_hash")
    }

    pub fn set_password_hash(&mut self, hash: &str) -> anyhow::Result<()> {
        self.set_setting("password_hash", hash)
    }
```

Add unit tests in `src/app_db/mod.rs` tests for create/delete/expired-session behavior.

- [ ] **Step 2: Run app_db tests to verify they fail (TDD red)**

Run: `cargo test app_db::tests`
Expected: FAIL — methods don't exist.

- [ ] **Step 3: Implement src/auth.rs**

```rust
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;
use rand::Rng;

use crate::AppState;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(anyhow::Error::from)?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub async fn ensure_password_setup(state: &AppState) -> anyhow::Result<()> {
    let mut app_db = state.app_db.lock().await;
    if app_db.password_hash()?.is_none() {
        let password = generate_token();
        let hash = hash_password(&password)?;
        app_db.set_password_hash(&hash)?;
        eprintln!("========================================================");
        eprintln!("rssea initial password: {password}");
        eprintln!("log in at /api/login (use the web UI) and change it in Settings");
        eprintln!("========================================================");
    }
    Ok(())
}
```

Note: `rand::rng()` is the rand 0.10 API (`rand::rng()` returns `ThreadRng`). If rand 0.10 uses `rand::thread_rng()` instead, use whatever the installed version exposes (check docs.rs / compile error and adjust). `fill` needs `rand::RngCore` — `use rand::RngCore;` or `use rand::Rng;` (Rng extends RngCore).

- [ ] **Step 4: Add login/logout/session handlers + middleware**

In `src/api/mod.rs` (or a new `src/api/auth.rs`):

```rust
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth;
use crate::AppState;

pub const SESSION_COOKIE: &str = "rssea_session";
const SESSION_TTL_SECS: i64 = 30 * 24 * 3600;

#[derive(Deserialize)]
pub struct LoginRequest { pub password: String }

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> Response {
    let hash = state.app_db.lock().await.password_hash().unwrap_or(None);
    let Some(hash) = hash else {
        return (StatusCode::FORBIDDEN, Json(json!({"error": {"code": "setup_required", "message": "no password configured"}}))).into_response();
    };
    if !auth::verify_password(&req.password, &hash) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": {"code": "unauthorized", "message": "wrong password"}}))).into_response();
    }
    let token = auth::generate_token();
    let token_hash = auth::sha256_hex(&token);
    let expires = (chrono::Utc::now() + chrono::Duration::seconds(SESSION_TTL_SECS)).to_rfc3339();
    let mut app_db = state.app_db.lock().await;
    let _ = app_db.create_session(&token_hash, &expires);
    drop(app_db);
    (
        StatusCode::OK,
        [(
            axum::http::header::SET_COOKIE,
            format!(
                "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECS}"
            ),
        )],
        Json(json!({"ok": true})),
    ).into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_value(&headers, SESSION_COOKIE) {
        let token_hash = auth::sha256_hex(&token);
        state.app_db.lock().await.delete_session(&token_hash).ok();
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::SET_COOKIE,
            format!("{SESSION_COOKIE}=; HttpOnly; Path=/; Max-Age=0"),
        )],
        Json(json!({"ok": true})),
    ).into_response()
}

pub async fn session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let authed = is_authenticated(&state, &headers).await;
    let setup_required = state.app_db.lock().await.password_hash().ok().flatten().is_none();
    (
        StatusCode::OK,
        Json(json!({
            "authenticated": authed,
            "version": crate::version(),
            "setup_required": setup_required,
        })),
    ).into_response()
}

pub async fn is_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else { return false };
    let token_hash = auth::sha256_hex(&token);
    state.app_db.lock().await.session_exists(&token_hash).unwrap_or(false)
}

pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let mut it = part.trim().splitn(2, '=');
        if it.next() == Some(name) {
            return it.next().map(|v| v.to_string());
        }
    }
    None
}
```

Middleware (in `src/api/mod.rs`):

```rust
pub async fn require_auth(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if crate::api::auth::is_authenticated(&state, req.headers()).await {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": {"code": "unauthorized", "message": "not logged in"}}))).into_response()
    }
}
```

Wire in `router`: the auth-protected routes get `.route_layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))`. Login/logout/session are OUTSIDE that layer.

- [ ] **Step 5: Wire AppState + run() + ensure_password_setup**

In `src/lib.rs`: `AppState` becomes:

```rust
#[derive(Clone)]
pub struct AppState {
    pub engine: engine::Engine,
    pub app_db: std::sync::Arc<tokio::sync::Mutex<app_db::AppDb>>,
}
```

`run()` opens the app DB (`app_db::AppDb::open(&config.data_dir)?`), wraps in `Arc<Mutex<_>>`, calls `auth::ensure_password_setup(&state).await`, then builds `AppState` and serves. Update `tests/health.rs` + `tests/api.rs` `spawn_app` to construct AppState with an app_db.

- [ ] **Step 6: Write integration tests `tests/auth.rs`**

Tests: (1) `GET /api/session` with no cookie → `{"authenticated": false, "setup_required": false}` after `ensure_password_setup` (which sets a password); (2) `POST /api/login` with wrong password → 401; (3) `POST /api/login` with correct password → 200 + sets `rssea_session` cookie; (4) `GET /api/feeds` WITHOUT cookie → 401; (5) `GET /api/feeds` WITH cookie → 200. Need a helper that captures the Set-Cookie from login and re-sends it. Use `reqwest` is NOT available for tests (it's a dependency but tests use tower oneshot); use `tower::ServiceExt` with `axum::http::header::COOKIE`.

For test (1), seed the app DB with a known password hash directly (call `hash_password("test-pass")` and `set_password_hash`) rather than relying on the random initial password.

- [ ] **Step 7: Run all tests, verify**

Run: `cargo test --test auth`, then `cargo test` full, `cargo clippy --all-targets`.
Expected: PASS, clippy clean.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/auth.rs src/lib.rs src/app_db/mod.rs src/api/ tests/auth.rs && git commit -m "Phase 3: add password auth with sessions and login/logout/session API"
```

---

### Task 2: Read-only sidecar queries — search (FTS fix) + per-category totals

**Files:**
- Create: `src/engine/queries.rs`
- Modify: `src/lib.rs` (`pub mod engine` already includes submodule; add `pub mod queries;` in engine/mod.rs), `src/dto.rs`, `src/engine/mod.rs`
- Test: `src/engine/queries.rs` unit tests (seeded against a real news-flash DB)

**Interfaces:**
- Consumes: `Engine` (for news-flash DB path via `config`), rusqlite.
- Produces in `src/engine/queries.rs`:
  - `pub struct SearchHit { pub article_id: String, pub title: Option<String>, pub feed_id: String, pub date: String, pub thumbnail_url: Option<String> }`
  - `pub fn search(db_path: &Path, query: &str, limit: i64) -> anyhow::Result<Vec<SearchHit>>` — read-only connection, runs `SELECT a.article_id, a.title, a.feed_id, a.date, a.thumbnail_url FROM articles a WHERE a.rowid IN (SELECT rowid FROM fts_table WHERE fts_table MATCH ?1) ORDER BY a.date DESC LIMIT ?2` with `news_flash::util::prepare_search_term(query)`.
  - `pub struct CategoryTotals { pub category_id: String, pub total: i64, pub unread: i64 }`
  - `pub fn category_totals(db_path: &Path) -> anyhow::Result<Vec<CategoryTotals>>` — `SELECT fm.category_id, COUNT(a.article_id), SUM(CASE WHEN a.unread = 1 THEN 1 ELSE 0 END) FROM feed_mapping fm LEFT JOIN articles a ON a.feed_id = fm.feed_id GROUP BY fm.category_id`.
  - `pub fn total_article_count(db_path: &Path) -> anyhow::Result<i64>` — `SELECT COUNT(*) FROM articles`.
  - `pub fn open_readonly(db_path: &Path) -> anyhow::Result<rusqlite::Connection>` — `rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)`.

- [ ] **Step 1: Write the failing test (in `src/engine/queries.rs` tests module)**

The test seeds a real news-flash DB via the existing engine test helpers (add_feed + sync with `RSS`), then opens a read-only connection to `data_dir/engine/data/database.sqlite` and asserts:
- `search` for a term in the article body (e.g. "Alpha") returns ≥1 hit with the correct `article_id`;
- `search` for a term NOT present returns empty;
- `category_totals` has a row for `NewsFlash.Toplevel` with total ≥ 2 and unread ≥ 2;
- `total_article_count` ≥ 2.

Use the engine's `#[cfg(test)]` FeedServer + RSS helpers (they are in `src/engine/mod.rs` tests module; replicate or reuse).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::engine::Engine;
    use std::path::PathBuf;

    #[tokio::test]
    async fn search_and_counts_against_newsflash_db() {
        let server = crate::engine::tests::FeedServer::start(crate::engine::tests::RSS.to_string(), 10);
        let dir = std::env::temp_dir().join(format!("rssea-queries-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir.clone(), host: "127.0.0.1".into(), port: 0 };
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
```

Note: `RSS` and `FeedServer` in `crate::engine::tests` — if they aren't `pub`, make them `pub` (one-word change). The `tmp_dir` helper there is pid-keyed; the queries test uses its own unique name.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test engine::queries::tests`
Expected: FAIL — module/functions don't exist (or FTS query errors).

- [ ] **Step 3: Implement src/engine/queries.rs**

```rust
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
    let rows = stmt.query_map([], |row| {
        Ok(CategoryTotals {
            category_id: row.get(0)?,
            total: row.get(1)?,
            unread: row.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn total_article_count(db_path: &Path) -> anyhow::Result<i64> {
    let conn = open_readonly(db_path)?;
    Ok(conn.query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))?)
}
```

Note: `SUM(...)` returns `Option<i64>` in rusqlite (nullable); `.get(2)` may need `Option<i64>` then `.unwrap_or(0)`. Handle it: change the map to `let unread: Option<i64> = row.get(2)?; ... unread: unread.unwrap_or(0)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test engine::queries::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/engine/queries.rs src/engine/mod.rs src/lib.rs && git commit -m "Phase 3: add read-only sidecar queries for search and category totals"
```

---

### Task 3: Overview endpoint

**Files:**
- Modify: `src/api/mod.rs`, `src/dto.rs`, `src/api/` (new `overview.rs`)
- Modify: `src/engine/mod.rs` (helper to compute per-category unread from feed map)
- Test: `tests/overview.rs`

**Interfaces:**
- Consumes: `Engine` (`get_feeds`, `unread_count_feed_map`, `get_categories`, `get_headlines`), `engine::queries` (totals), config for db path.
- Produces:
  - DTO `CategoryCard { category_id: String, name: String, total_count: i64, unread_count: i64, items: Vec<Headline> }` in `src/dto.rs`.
  - `GET /api/overview` → `{ "cards": [CategoryCard...], "all": { total_count, unread_count } }`.
  - Engine helper: `pub async fn category_unread_map(&self) -> anyhow::Result<HashMap<String, i64>>` — from `unread_count_feed_map` + feed mappings, sum per category (expand descendant feeds via mappings).

- [ ] **Step 1: Write the failing test `tests/overview.rs`**

Reuse the RSS fixture; add a feed + sync; call `GET /api/overview` with a valid session cookie (seed a known password, login, extract cookie). Assert: cards contains a card with `name == "NewsFlash.Toplevel"` (or whatever the top-level category resolves to), `total_count >= 2`, `items.len() >= 1`. Assert `all.total_count >= 2`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test overview`
Expected: FAIL — route/DTO missing.

- [ ] **Step 3: Implement engine category_unread_map + overview handler**

Engine:

```rust
    pub async fn category_unread_map(&self) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let (_, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let mut out: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for m in mappings {
            let feed_id = m.feed_id.as_str().to_string();
            let cat = m.category_id.as_str().to_string();
            let count = unread.get(&m.feed_id).copied().unwrap_or(0);
            *out.entry(cat).or_insert(0) += count;
        }
        Ok(out)
    }
```

Handler `src/api/overview.rs`:

```rust
use axum::extract::State;
use axum::Json;
use serde_json::{Value, json};

use crate::api::error::ApiResult;
use crate::dto::{CategoryCard, Headline};
use crate::AppState;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, OrderBy};

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let (categories, category_mappings) = state.engine.get_categories().await?;
    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let totals = crate::engine::queries::category_totals(&db_path)?;
    let category_unread = state.engine.category_unread_map().await?;

    let mut cards = Vec::new();
    for cat in categories {
        let mut filter = ArticleFilter::default();
        filter.order = Some(ArticleOrder::NewestFirst);
        filter.order_by = Some(OrderBy::Published);
        filter.limit = Some(10);
        filter.categories = Some(vec![cat.category_id.clone()]);
        let items = state.engine.get_headlines(filter).await?;
        cards.push(CategoryCard {
            category_id: cat.category_id.as_str().to_string(),
            name: cat.label,
            total_count: totals.iter().find(|t| t.category_id == cat.category_id.as_str()).map(|t| t.total).unwrap_or(0),
            unread_count: category_unread.get(cat.category_id.as_str()).copied().unwrap_or(0),
            items,
        });
    }

    let all_total = crate::engine::queries::total_article_count(&db_path)?;
    let all_unread = state.engine.with_nf(|nf| nf.unread_count_all()).await?;

    Ok(Json(json!({
        "cards": cards,
        "all": { "total_count": all_total, "unread_count": all_unread },
    })))
}
```

Notes:
- `CategoryCard` must be `Serialize`. `Headline` already is.
- `state.engine.data_dir()` — add a public accessor to `Engine`: `pub fn data_dir(&self) -> &std::path::Path` (store `data_dir: PathBuf` in Engine at construction from config; or store config clone). Add this field in this task.
- `category_mappings` unused here (totals come from queries); `_category_mappings` to avoid warning.
- Register route `GET /api/overview`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test overview`, then full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/overview.rs src/api/mod.rs src/engine/mod.rs src/dto.rs tests/overview.rs && git commit -m "Phase 3: add overview endpoint with per-category cards"
```

---

### Task 4: Timeline filters (saved/tag/search) + search suggestions + full search

**Files:**
- Modify: `src/api/articles.rs`, `src/engine/mod.rs`, `src/api/mod.rs`, `src/dto.rs`
- Create: `src/api/search.rs`
- Test: extend `tests/api.rs` or new `tests/search.rs`

**Interfaces:**
- Consumes: `engine::queries::search`, `get_headlines`, app DB (`saved_tags`).
- Produces:
  - `GET /api/articles` extended query params: `saved=` (bool → `marked`), `tag=` (name → app DB `saved_tags` → `ArticleFilter.ids`), `search=` (union: FTS via queries + feed-name match).
  - `GET /api/search/suggestions?q=` → `{ "suggestions": [Headline...] }` top 5-8.
  - Engine: `pub async fn get_headlines_by_ids(&self, ids: Vec<String>) -> anyhow::Result<Vec<Headline>>`, `pub async fn search(&self, q: &str, limit: i64) -> anyhow::Result<Vec<Headline>>` (uses queries + maps to Headline + feed_title), `pub async fn search_suggestions(&self, q: &str) -> anyhow::Result<Vec<Headline>>`.
  - AppDb: `pub fn article_ids_for_tag(&self, tag: &str) -> anyhow::Result<Vec<String>>` (SELECT article_id FROM saved_tags WHERE tag=?).

- [ ] **Step 1: Write the failing test (extend tests/api.rs)**

After adding + syncing a feed with a known body term (the RSS fixture body "Alpha body."), test: `GET /api/articles?search=Alpha` returns ≥1 item; `GET /api/search/suggestions?q=Alpha` returns ≥1 suggestion; `GET /api/articles?search=zzznope` returns empty. (Requires a valid session cookie like other tests.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test api`
Expected: FAIL — search param unsupported / no suggestions route.

- [ ] **Step 3: Implement**

AppDb method:

```rust
    pub fn article_ids_for_tag(&self, tag: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT article_id FROM saved_tags WHERE tag = ?1")?;
        let mut rows = stmt.query(rusqlite::params![tag])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row.get(0)?);
        }
        Ok(out)
    }
```

Engine additions:

```rust
    pub async fn get_headlines_by_ids(&self, ids: Vec<String>) -> anyhow::Result<Vec<Headline>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.into_iter().map(news_flash::models::ArticleID::new).collect::<Vec<_>>();
        let mut filter = news_flash::models::ArticleFilter::default();
        filter.order = Some(news_flash::models::ArticleOrder::NewestFirst);
        filter.order_by = Some(news_flash::models::OrderBy::Published);
        filter.ids = Some(ids);
        self.get_headlines(filter).await
    }

    pub async fn search(&self, q: &str, limit: i64) -> anyhow::Result<Vec<Headline>> {
        let db_path = self.data_dir().join("engine/data/database.sqlite");
        let hits = crate::engine::queries::search(&db_path, q, limit)?;
        let ids = hits.into_iter().map(|h| h.article_id).collect();
        self.get_headlines_by_ids(ids).await
    }
```

Note: `queries::search` returns article_ids; re-fetch via get_headlines_by_ids so Headline has feed_title etc. (get_headlines re-sorts by date desc, fine).

articles.rs list handler additions: parse `saved`, `tag`, `search` params.

- `saved` (string "true"/"false" or "1"/"0"): set `filter.marked`.
- `tag`: `state.app_db.lock().await.article_ids_for_tag(&tag)?` → if empty return `[]`; else set `filter.ids`.
- `search`: call `engine.search(&q, limit)` and return those (skip the news-flash filter path). Keep `search` mutually exclusive with feed/category/tag/saved for v1 (document; if both present, search wins).

search.rs suggestions handler:

```rust
pub async fn suggestions(State(state): State<AppState>, Query(params): Query<SuggestParams>) -> ApiResult<Json<Value>> {
    let q = params.q.unwrap_or_default();
    let suggestions = state.engine.search(&q, 8).await?;
    Ok(Json(json!({ "suggestions": suggestions })))
}
```

Register `GET /api/search/suggestions`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test api`, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/articles.rs src/api/search.rs src/api/mod.rs src/engine/mod.rs src/app_db/mod.rs src/dto.rs tests/api.rs && git commit -m "Phase 3: add search suggestions, full-text search, saved and tag timeline filters"
```

---

### Task 5: Saved items — save/unsave, note+tags, saved page (month grouping), tags list

**Files:**
- Modify: `src/api/articles.rs`, `src/engine/mod.rs`, `src/api/mod.rs`, `src/dto.rs`, `src/app_db/mod.rs`
- Create: `src/api/saved.rs`
- Test: `tests/saved.rs`

**Interfaces:**
- Consumes: app DB (`saved`, `saved_tags`, `tags`), `Engine` (mark/unmark, get_headlines_by_ids), `ArticleFilter`.
- Produces:
  - `POST /api/articles/:id/save {note?, tags?: string[]}` → marks article, upserts `saved`, replaces `saved_tags`, upserts `tags`.
  - `PUT /api/articles/:id/save` — same (idempotent edit).
  - `DELETE /api/articles/:id/save` — unmark, delete `saved` + `saved_tags`.
  - `GET /api/saved?offset=&limit=` → `{ "months": [{ "month": "2026-08", "items": [Headline...] }], "total": N }`, ordered by saved_at desc, grouped by month.
  - `GET /api/tags` → `{ "tags": ["tag1", ...] }`.
  - AppDb: `save_article(&mut self, article_id, note: Option<&str>, tags: &[String]) -> Result<()>`, `unsave_article(&mut self, article_id) -> Result<()>`, `saved_articles(&self, offset, limit) -> Result<(Vec<(String, String)>, i64)>` (article_id, saved_at + total), `note_and_tags(&self, article_id) -> Result<(Option<String>, Vec<String>)>`, `all_tags(&self) -> Result<Vec<String>>`.
  - Engine: `pub async fn mark_article_saved(&self, id: &str, saved: bool) -> anyhow::Result<()>` (calls `set_article_marked`).

- [ ] **Step 1: Write the failing test `tests/saved.rs`**

Seed a feed + sync; save an article with note+tags via POST; assert: GET /api/saved returns it in the current month with the note; GET /api/tags contains the tag; GET /api/articles/:id returns the note+tags; DELETE removes it (GET /api/saved empty).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test saved`
Expected: FAIL.

- [ ] **Step 3: Implement app_db methods + engine mark + handlers**

app_db methods (use `INSERT OR REPLACE` for saved; delete+insert for saved_tags; `INSERT OR IGNORE` for tags):

```rust
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
```

Engine mark:

```rust
    pub async fn mark_article_saved(&self, id: &str, saved: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let article_id = news_flash::models::ArticleID::new(id);
        let marked = if saved { news_flash::models::Marked::Marked } else { news_flash::models::Marked::Unmarked };
        self.nf.set_article_marked(&[article_id], marked, &self.client).await?;
        Ok(())
    }
```

Handlers `src/api/saved.rs`:

- `save(Path(id), Json({note?, tags?}))`: `engine.mark_article_saved(&id, true)` + `app_db.save_article(&id, note, tags)`.
- `unsave(Path(id))`: `engine.mark_article_saved(&id, false)` + `app_db.unsave_article(&id)`.
- `list(Query({offset, limit}))`: `app_db.saved_articles(offset, limit)` → ids → `engine.get_headlines_by_ids(ids)` → group by month (YYYY-MM from saved_at) preserving order. Build `months` array.
- `tags`: `app_db.all_tags()`.

DTO additions: `ArticleDetail` gains `note: Option<String>` and `tags: Vec<String>` (populated in `get_article_detail` from app DB — but engine doesn't have app_db access! **Design**: the API layer enriches the detail with note/tags from app_db after calling engine, rather than engine doing it. In the `detail` handler, after `engine.get_article_detail`, read `app_db.note_and_tags` and set fields. Since `ArticleDetail` is `Serialize`, make note/tags fields and have the handler fill them.)

Routes: `POST /api/articles/:id/save`, `PUT /api/articles/:id/save`, `DELETE /api/articles/:id/save`, `GET /api/saved`, `GET /api/tags`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test saved`, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/saved.rs src/api/articles.rs src/api/mod.rs src/engine/mod.rs src/app_db/mod.rs src/dto.rs tests/saved.rs && git commit -m "Phase 3: add saved items with notes and tags, month-grouped listing"
```

---

### Task 6: Categories CRUD + tree

**Files:**
- Create: `src/api/categories.rs`
- Modify: `src/api/mod.rs`, `src/engine/mod.rs`, `src/dto.rs`
- Test: `tests/categories.rs`

**Interfaces:**
- Consumes: Engine (add_category, rename_category, remove_category, get_categories, set_category_read), queries (totals).
- Produces:
  - DTO `CategoryNode { category_id, name, parent_id, unread_count, children: Vec<CategoryNode> }`.
  - `GET /api/categories` → `[CategoryNode...]` (tree rooted at children of `NewsFlash.Toplevel`; include the pseudo-root itself as a top node or flatten — decide: return `{ "categories": [CategoryNode...] }` where each top-level node includes its descendants; `NewsFlash.Toplevel` itself is rendered as a synthetic "All" node with the top-level feeds).
  - `POST /api/categories {name, parent_id?}` → new CategoryNode.
  - `PATCH /api/categories/:id {name?, parent_id?}` → rename and/or re-parent.
  - `DELETE /api/categories/:id {remove_children?: bool}` (default false → reparent children up).
  - `POST /api/categories/:id/read` → mark all read.
  - Engine: `pub async fn add_category(&self, name: &str, parent: Option<&str>) -> anyhow::Result<()>`, `pub async fn rename_category(&self, id: &str, name: &str) -> anyhow::Result<()>`, `pub async fn remove_category(&self, id: &str, remove_children: bool) -> anyhow::Result<()>`, `pub async fn mark_category_read(&self, id: &str) -> anyhow::Result<()>`, plus a `get_category_tree()` helper returning the CategoryNode tree (building from get_categories + category_unread_map).

- [ ] **Step 1: Write the failing test `tests/categories.rs`**

Create a category via POST; GET /api/categories returns it; PATCH rename works; POST /:id/read returns 200; DELETE works; GET no longer lists it. Requires session cookie.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test categories`
Expected: FAIL.

- [ ] **Step 3: Implement**

Engine wrappers (each takes mutation guard):

```rust
    pub async fn add_category(&self, name: &str, parent: Option<&str>) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let parent = parent.map(news_flash::models::CategoryID::new);
        self.nf.add_category(name, parent.as_ref(), &self.client).await?;
        Ok(())
    }
    pub async fn rename_category(&self, id: &str, name: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.rename_category(&news_flash::models::CategoryID::new(id), name, &self.client).await?;
        Ok(())
    }
    pub async fn remove_category(&self, id: &str, remove_children: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.remove_category(&news_flash::models::CategoryID::new(id), remove_children, &self.client).await?;
        Ok(())
    }
    pub async fn mark_category_read(&self, id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.set_category_read(&[news_flash::models::CategoryID::new(id)], &self.client).await?;
        Ok(())
    }
```

Tree builder: read `get_categories()` (categories + mappings), `category_unread_map()`, build children by `parent_id` adjacency. Root parent = `NewsFlash.Toplevel`.

Handlers in categories.rs per interfaces. Register routes.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test categories`, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/categories.rs src/api/mod.rs src/engine/mod.rs src/dto.rs tests/categories.rs && git commit -m "Phase 3: add categories CRUD and tree"
```

---

### Task 7: Sources CRUD + discovery + OPML import/export

**Files:**
- Modify: `src/api/sources.rs`, `src/engine/mod.rs`, `src/api/mod.rs`, `src/dto.rs`
- Test: `tests/sources.rs`

**Interfaces:**
- Consumes: Engine (add_feed, remove_feed, rename_feed, move_feed, fetch_feed, set_feed_read, import_opml, export_opml), feed_parser discovery.
- Produces:
  - `GET /api/sources` → grouped by category: `{ "groups": [{ "category_id", "category_name", "feeds": [FeedSummary...] }] }`.
  - `POST /api/sources` — **improved**: run `feed_parser::download_and_parse_feed(url, ...)` FIRST; if `SingleFeed`, use its `feed_url` + `label`; if `MultipleFeeds`, take the first (or return 409 with choices); if parse fails, fall back to plain `add_feed` (direct feed URL). Then `add_feed(discovered_url, Some(label), category_id)`.
  - `POST /api/sources/discover {url}` → `{ "title": ..., "feed_url": ..., "alternatives": [...] }`.
  - `POST /api/sources/import-opml {opml}` → 200 `{ "imported": true }`; calls `import_opml(opml, true, client)`.
  - `GET /api/sources/export-opml` → 200 text/xml.
  - `PATCH /api/sources/:id {title?, category_id?}` → rename and/or move.
  - `DELETE /api/sources/:id` → remove + prune app DB saved rows.
  - `POST /api/sources/:id/read` → set_feed_read.
  - Engine: `pub async fn rename_feed(&self, id, title) -> Result<()>`, `pub async fn move_feed(&self, id, to_category) -> Result<()>`, `pub async fn remove_feed(&self, id) -> Result<()>`, `pub async fn mark_feed_read(&self, id) -> Result<()>`, `pub async fn import_opml(&self, opml: &str) -> Result<()>`, `pub async fn export_opml(&self) -> Result<String>`, `pub async fn discover(&self, url: &str) -> Result<Discovered>`.
  - `pub struct Discovered { pub title: Option<String>, pub feed_url: Option<String>, pub alternatives: Vec<(String, String)> }` (label, url).

- [ ] **Step 1: Write the failing test `tests/sources.rs`**

Add a feed (direct URL), GET /api/sources shows it; PATCH rename; POST /:id/read; DELETE removes it; import-opml with a small OPML containing the feed URL works; export-opml returns non-empty xml. For discover: POST /api/sources/discover with the direct feed URL returns a title.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test sources`
Expected: FAIL.

- [ ] **Step 3: Implement**

Engine discovery:

```rust
pub struct Discovered {
    pub title: Option<String>,
    pub feed_url: Option<String>,
    pub alternatives: Vec<(String, String)>,
}

    pub async fn discover(&self, url: &str) -> anyhow::Result<Discovered> {
        let url = news_flash::models::Url::parse(url)?;
        let id = news_flash::models::FeedID::new(url.as_str());
        let semaphore = self.nf.get_semaphore();
        let parsed = news_flash::feed_parser::download_and_parse_feed(&url, &id, None, semaphore, &self.client).await;
        match parsed {
            Ok(news_flash::feed_parser::ParsedUrl::SingleFeed(feed)) => {
                Ok(Discovered {
                    title: Some(feed.label),
                    feed_url: feed.feed_url.map(|u| u.to_string()),
                    alternatives: Vec::new(),
                })
            }
            Ok(news_flash::feed_parser::ParsedUrl::MultipleFeeds(feeds)) => {
                let first = feeds.first();
                Ok(Discovered {
                    title: first.map(|f| f.label.clone()),
                    feed_url: first.and_then(|f| f.feed_url.clone()).map(|u| u.to_string()),
                    alternatives: feeds.into_iter().map(|f| (f.label, f.feed_url.map(|u| u.to_string()).unwrap_or_default())).collect(),
                })
            }
            Err(_) => Ok(Discovered { title: None, feed_url: None, alternatives: Vec::new() }),
        }
    }
```

Sources handlers per interfaces. The `add` handler: `engine.discover(url)` first; if `feed_url` present, `add_feed(feed_url, Some(title or req.title), category_id)`; else `add_feed(original url, req.title, category_id)` (direct feed URL path). On MultipleFeeds with choices and no explicit choice, prefer the first.

Prune saved rows on delete: get article ids for the feed BEFORE remove (via `get_article_ids(ArticleFilter{feeds})`), then `app_db.unsave_article` each. Add an Engine helper `pub async fn feed_article_ids(&self, feed_id: &str) -> Result<Vec<String>>`.

Register routes.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test sources`, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/sources.rs src/engine/mod.rs src/api/mod.rs src/dto.rs tests/sources.rs && git commit -m "Phase 3: add sources CRUD, feed discovery, and OPML import/export"
```

---

### Task 8: Settings endpoint

**Files:**
- Create: `src/api/settings.rs`
- Modify: `src/api/mod.rs`, `src/engine/mod.rs`, `src/app_db/mod.rs`, `src/dto.rs`
- Test: `tests/settings.rs`

**Interfaces:**
- Consumes: app DB settings, Engine (database_size, get_feeds len, unread_count_all, get_keep_articles_duration/set), queries (total_article_count).
- Produces:
  - `GET /api/settings` → `{ "theme": ..., "sync_interval_minutes": ..., "keep_articles_days": Option<i64>, "stats": { "feeds": n, "articles": n, "unread": n, "database_size_bytes": n, "last_sync": ... } }`.
  - `PATCH /api/settings {theme?, sync_interval_minutes?, keep_articles_days?}` — theme/sync stored in app DB; keep_articles_days → `engine.set_keep_articles_duration(Some/None)`.
  - `POST /api/settings/password {current_password, new_password}` → verify current, set new.
  - AppDb: `theme()`, `set_theme()`, `sync_interval_minutes()` (default 30), `set_sync_interval_minutes()`.
  - Engine: `pub async fn keep_articles_days(&self) -> Result<Option<i64>>`, `pub async fn set_keep_articles_days(&self, days: Option<i64>) -> Result<()>`, `pub async fn database_size_bytes(&self) -> Result<u64>`.

- [ ] **Step 1: Write the failing test `tests/settings.rs`**

GET /api/settings returns defaults (theme unset → null, sync_interval 30, keep_articles null); PATCH theme to "dark" → GET reflects; PATCH sync_interval to 15 → reflects; POST /api/settings/password with wrong current → 401, correct → 200 (then login with new password works).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings`
Expected: FAIL.

- [ ] **Step 3: Implement**

app_db settings accessors:

```rust
    pub fn theme(&self) -> anyhow::Result<Option<String>> { self.get_setting("theme") }
    pub fn set_theme(&mut self, theme: &str) -> anyhow::Result<()> { self.set_setting("theme", theme) }
    pub fn sync_interval_minutes(&self) -> anyhow::Result<i64> {
        Ok(self.get_setting("sync_interval_minutes")?.and_then(|s| s.parse().ok()).unwrap_or(30))
    }
    pub fn set_sync_interval_minutes(&mut self, minutes: i64) -> anyhow::Result<()> {
        self.set_setting("sync_interval_minutes", &minutes.to_string())
    }
```

Engine:

```rust
    pub async fn keep_articles_days(&self) -> anyhow::Result<Option<i64>> {
        let dur = self.nf.get_keep_articles_duration().await;
        Ok(dur.map(|d| d.num_days()))
    }
    pub async fn set_keep_articles_days(&self, days: Option<i64>) -> anyhow::Result<()> {
        let dur = days.map(chrono::Duration::days);
        self.nf.set_keep_articles_duration(dur).await?;
        Ok(())
    }
    pub async fn database_size_bytes(&self) -> anyhow::Result<u64> {
        let size = self.with_nf(|nf| nf.database_size()).await?;
        Ok(size.on_disk as u64)
    }
```

Handlers in settings.rs per interfaces. `PATCH /api/settings` uses a serde struct with all-Option fields; `keep_articles_days: None` in JSON is ambiguous with "absent" — use `Option<Option<i64>>` for that field or a sentinel. Simplest: accept `keep_articles_days: Option<Option<i64>>` (JSON `null` → Some(None) → keep-everything; absent → None → no change).

Register routes.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test settings`, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/api/settings.rs src/api/mod.rs src/engine/mod.rs src/app_db/mod.rs src/dto.rs tests/settings.rs && git commit -m "Phase 3: add settings endpoint with theme, sync interval, retention, and password change"
```

---

### Task 9: Image proxy (`GET /img`) + article HTML rewrite

**Files:**
- Create: `src/proxy.rs`, `src/engine/content.rs`
- Modify: `src/api/mod.rs` (route `/img`), `src/engine/mod.rs` (expose db path), `src/dto.rs`, `src/api/articles.rs` (detail returns rewritten HTML)
- Test: `src/engine/content.rs` unit tests + `tests/proxy.rs`

**Interfaces:**
- Consumes: reqwest, scraper, news-flash `relative_url_evaluater`.
- Produces:
  - `src/proxy.rs`: `pub async fn proxy_image(Query({u}), State(state)) -> Response` — validates `u` is http/https, fetches upstream with a shared reqwest client, streams bytes with content-type; 400 on bad scheme, 502 on upstream failure, 404 if not found.
  - `src/engine/content.rs`: `pub fn rewrite_html(html: &str, base_url: &str) -> String` — absolute-izes all `a href`, `img src`/`srcset`; rewrites `<img src=...>` to `/img?u=<encoded absolute>`; adds `data-original` with the original absolute URL. Returns rewritten HTML. Unit-testable (pure function).
  - Engine: `pub async fn render_article_content(&self, article: &ArticleDetail) -> anyhow::Result<ArticleDetail>` — takes an ArticleDetail, applies `rewrite_html` to its `html` using its `url` as base.
  - articles.rs `detail` handler: after building ArticleDetail, run `engine.render_article_content` on it.

- [ ] **Step 1: Write the failing test for rewrite_html (unit)**

Test cases: (a) `<a href="/rel">` becomes absolute against base; (b) `<img src="https://x/y.png">` becomes `/img?u=<encoded>` and keeps `data-original`; (c) relative img src resolves; (d) external absolute stays absolute.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test engine::content::tests`
Expected: FAIL.

- [ ] **Step 3: Implement content.rs**

Use the `scraper` crate:

```rust
use scraper::{Html, Selector};
use url::Url;

pub fn rewrite_html(html: &str, base_url: &str) -> String {
    let base = match Url::parse(base_url) {
        Ok(b) => b,
        Err(_) => return html.to_string(),
    };
    let doc = Html::parse_fragment(html);
    let a_sel = Selector::parse("a").unwrap();
    let img_sel = Selector::parse("img").unwrap();

    for a in doc.select(&a_sel) {
        if let Some(href) = a.value().attr("href") {
            if let Ok(abs) = base.join(href) {
                let _ = a.value().set_attr("href", abs.as_str());
            }
        }
    }
    for img in doc.select(&img_sel) {
        if let Some(src) = img.value().attr("src") {
            if let Ok(abs) = base.join(src) {
                let encoded = percent_encode(abs.as_str().as_bytes(), NON_ALPHANUMERIC);
                let _ = img.value().set_attr("data-original", abs.as_str());
                let _ = img.value().set_attr("src", &format!("/img?u={encoded}"));
            }
        }
    }
    doc.html()
}
```

Note: `scraper`'s `Html::parse_fragment` + `doc.html()` re-serializes. `percent_encode` via `url::percent_encoding::{percent_encode, NON_ALPHANUMERIC}`. `Selector::parse` returns Result — unwrap is fine for static selectors (or propagate). In tests, verify the rewrite by searching for `/img?u=` and `data-original`.

- [ ] **Step 4: Implement proxy.rs**

```rust
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
pub struct ImgParams { pub u: Option<String> }

pub async fn proxy_image(Query(params): Query<ImgParams>, State(state): State<AppState>) -> Response {
    let Some(u) = params.u else {
        return (StatusCode::BAD_REQUEST, "missing u").into_response();
    };
    let Ok(parsed) = url::Url::parse(&u) else {
        return (StatusCode::BAD_REQUEST, "invalid url").into_response();
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return (StatusCode::BAD_REQUEST, "only http/https allowed").into_response();
    }
    let client = state.engine.client();
    match client.get(parsed.as_str()).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
            }
            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_GATEWAY, "read failed").into_response(),
            };
            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "upstream unavailable").into_response(),
    }
}
```

Register `GET /img` route (should it require auth? Images are embedded in article HTML served to the PWA; keep `/img` unauthenticated to avoid cache/CORS friction, but note it in a comment-free way — decide: unauthenticated for v1, documented in Self-Review). Actually — for privacy, `/img` proxying is fine unauthenticated since it only fetches what article HTML references. Keep it OUTSIDE the auth layer.

- [ ] **Step 5: Implement render_article_content + detail wiring**

Engine:

```rust
    pub async fn render_article_content(&self, article: &mut crate::dto::ArticleDetail) -> anyhow::Result<()> {
        if let (Some(html), Some(base)) = (&article.html, &article.url) {
            article.html = Some(crate::engine::content::rewrite_html(html, base));
        }
        Ok(())
    }
```

detail handler: build detail, call render, return.

- [ ] **Step 6: Add proxy test + run all tests**

Add `tests/proxy.rs`: spin the FeedServer (or a plain TCP server serving a tiny PNG/JPEG bytes), hit `GET /img?u=<that url>` with the bytes URL, assert 200 + correct content-type. Also assert `/img?u=ftp://...` → 400.

Run: `cargo test engine::content::tests`, `cargo test --test proxy`, full `cargo test`, `cargo clippy --all-targets`.

- [ ] **Step 7: Commit**

```bash
git add src/proxy.rs src/engine/content.rs src/engine/mod.rs src/api/mod.rs src/api/articles.rs tests/proxy.rs && git commit -m "Phase 3: add image proxy and article HTML rewrite pass"
```

---

### Task 10: Error-mapping refinement + article read/unread toggle + mark-all-read + final integration pass

**Files:**
- Modify: `src/api/error.rs` (NewsFlashError → HTTP mapping: Syncing→409, Offline→503, NotFound→404, Thumbnail/GrabContent→502), `src/api/articles.rs`, `src/engine/mod.rs`, `src/api/mod.rs`
- Test: extend `tests/api.rs`

**Interfaces:**
- Produces:
  - `From<NewsFlashError>` for `ApiError` (or a mapping helper) handling: `Syncing`→409, `Offline`→503, `DatabaseError::Query(NotFound)`→404, `Thumbnail`/`GrabContent`/`Icon`→502, `OPML`→400, else 500.
  - `POST /api/articles/:id/read {read?: bool}` → `set_article_read`.
  - `POST /api/articles/:id/unread`.
  - `POST /api/read-all` → `set_all_read`.
  - Engine: `pub async fn set_article_read(&self, id, read) -> Result<()>`, `pub async fn mark_all_read(&self) -> Result<()>`.
  - `GET /api/feeds/:id/unread` — count for a single feed (or omit; decide — omit if not needed).

- [ ] **Step 1: Implement error mapping**

In `src/api/error.rs` add:

```rust
impl From<news_flash::error::NewsFlashError> for ApiError {
    fn from(e: news_flash::error::NewsFlashError) -> Self {
        use news_flash::error::NewsFlashError;
        match e {
            NewsFlashError::Syncing => ApiError { status: StatusCode::CONFLICT, code: "syncing", message: "sync in progress".into() },
            NewsFlashError::Offline => ApiError { status: StatusCode::SERVICE_UNAVAILABLE, code: "offline", message: "offline".into() },
            NewsFlashError::Database(err) => {
                if matches!(err, news_flash::error::DatabaseError::Query(diesel::result::Error::NotFound)) {
                    ApiError { status: StatusCode::NOT_FOUND, code: "not_found", message: "not found".into() }
                } else {
                    ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal", message: "database error".into() }
                }
            }
            NewsFlashError::OPML(_) => ApiError { status: StatusCode::BAD_REQUEST, code: "bad_opml", message: "invalid opml".into() },
            _ => ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal", message: "internal error".into() },
        }
    }
}
```

Note: verify `news_flash::error::DatabaseError` path (re-exported at crate root via `pub use crate::database::DatabaseError` in error.rs). `diesel` is now a direct dependency (added in Phase 2's final review). `NewsFlashError::OPML` variant exists. Adjust to actual variant names if any differ.

- [ ] **Step 2: Implement read/unread toggles + read-all**

Engine:

```rust
    pub async fn set_article_read(&self, id: &str, read: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let article_id = news_flash::models::ArticleID::new(id);
        let read = if read { news_flash::models::Read::Read } else { news_flash::models::Read::Unread };
        self.nf.set_article_read(&[article_id], read, &self.client).await?;
        Ok(())
    }

    pub async fn mark_all_read(&self) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.set_all_read(&self.client).await?;
        Ok(())
    }
```

Routes: `POST /api/articles/:id/read` (body `{read?: bool}`, default true), `POST /api/articles/:id/unread`, `POST /api/read-all`.

- [ ] **Step 3: Update PATCH /api/articles/:id to accept {read?, saved?}**

`PATCH /api/articles/:id {read?: bool, saved?: bool}` — if read present call set_article_read; if saved present call mark_article_saved + app_db save/unsave.

- [ ] **Step 4: Add integration assertions to tests/api.rs**

After the existing flow: PATCH an article read → GET list shows unread=false for it; POST /api/read-all → unread_count_all == 0 (via a settings-like check or a GET /api/feeds unread sum). Add assertions.

- [ ] **Step 5: Run all tests**

Run: `cargo test`, `cargo test --all-targets`, `cargo clippy --all-targets`. Fix any failures.

- [ ] **Step 6: Commit**

```bash
git add src/api/error.rs src/api/articles.rs src/api/mod.rs src/engine/mod.rs tests/api.rs && git commit -m "Phase 3: refine error mapping, add read/unread toggles and mark-all-read"
```

---

## Self-Review Notes

- Spec coverage: all of §4 (auth, overview, timeline, saved, categories, sources, settings, search, media) + §6 (proxy + rewrite). Not in this phase (Phases 4-6): frontend, PWA, embedding, shipping.
- Placeholder scan: no TBDs. `rand::rng()` API noted as possibly `thread_rng()` — implementer verifies against installed crate. `NewsFlashError` variant names (Syncing/Offline/OPML/Database) must be verified against news-flash error.rs before the From impl compiles.
- Type consistency: `AppState { engine, app_db }` used everywhere; `engine::queries::*` functions take `&Path`; `Engine::data_dir()` added in Task 3 and used by later tasks. DTOs: CategoryCard (Task 3), CategoryNode (Task 6), Discovered (Task 7), ArticleDetail gains note/tags (Task 5).
- Two auth-protection notes: `/img` is deliberately OUTSIDE the auth layer (article HTML embeds it; avoids cache friction) — documented, not a defect. `setup_required` in `/api/session` uses a clean `is_none()` check.
- Known design decisions (not defects): `tag=` filter routes through app DB `saved_tags`; search runs through the read-only sidecar FTS fix; saved-at timestamp is ours alone; `feed`+`category` together union in news-flash.
