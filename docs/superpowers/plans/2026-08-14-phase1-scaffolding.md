# Phase 1: Project Scaffolding — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A buildable Rust backend skeleton that `make dev` runs on :3000 serving `/api/health`, with app DB initialization, config/CLI parsing, and logging.

**Architecture:** Binary crate (axum server) with modules `config`, `app_db`, `api`, `engine` (stub for Phase 2), `auth` (stub). A Makefile coordinates backend + frontend dev workflows. The frontend is out of scope for this phase (frontend scaffold is Phase 4) — `make dev` runs backend only for now, with the frontend target stubbed and documented.

**Tech Stack:** Rust (edition 2024, nightly 1.99), axum 0.8.9, tokio 1.53.1 (full), serde/serde_json, rusqlite 0.40.2 (bundled), tracing + tracing-subscriber, clap 4.6.6, url 2.5.8, reqwest 0.13.4 (matching news-flash; added now, used in Phase 2). news-flash 3.2.0.

## Global Constraints

- Backend is async axum; all news-flash and CPU-heavy work must eventually run via `spawn_blocking` (not needed yet in this phase).
- Two SQLite DBs: news-flash's (`data/database.sqlite`, owned by engine) and our sidecar (`data/rssea.sqlite`, owned by app_db). Linked by news-flash string IDs.
- `reqwest` must be version 0.13 (news-flash's own Client type; 0.12 will not compile against `nf.sync`/`nf.add_feed`).
- Default data dir: `~/.local/share/rssea`, overridable via CLI/env.
- No comments in code unless the task explicitly shows them.
- Keep module structure per spec §9 (paths below).
- Commit after each task with the exact message given.

---

### Task 1: Restructure crate to binary + declare dependencies

**Files:**
- Modify: `Cargo.toml`
- Create: `src/main.rs`
- Modify: `src/lib.rs` (keep as library root that re-exports modules; `main.rs` uses the lib)

**Interfaces:**
- Consumes: nothing.
- Produces: binary target `rssea`; lib crate `rssea` with modules `config`, `app_db`, `api`, `engine`, `auth` (public); `src/main.rs` calls `rssea::run()`.

- [ ] **Step 1: Rewrite `Cargo.toml`**

```toml
[package]
name = "rssea"
version = "0.1.0"
edition = "2024"

[dependencies]
news-flash = "3.2.0"
axum = "0.8.9"
tokio = { version = "1.53.1", features = ["full"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
rusqlite = { version = "0.40.2", features = ["bundled"] }
clap = { version = "4.6.6", features = ["derive", "env"] }
url = "2.5.8"
reqwest = "0.13.4"
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }

[dev-dependencies]
tower = "0.5"
http-body-util = "0.1"
```

- [ ] **Step 2: Write the failing test (in `src/main.rs` or a test module)**

Add a trivial integration test that asserts the binary's lib exposes a `run` function type. Concretely, add to `src/lib.rs`:

```rust
pub mod api;
pub mod app_db;
pub mod auth;
pub mod config;
pub mod engine;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

Add to `Cargo.toml` under `[lib]` nothing extra (default works). Now write the test in `src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert_eq!(crate::version(), "0.1.0");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test`
Expected: FAIL — because the modules `api`, `app_db`, `auth`, `config`, `engine` do not exist yet (compile error), or the test fails.

- [ ] **Step 4: Create the module skeletons**

Create empty module files so the lib compiles:

- `src/api/mod.rs` → `pub mod health;` placeholder with a `pub fn router() -> axum::Router {}` stub returning `axum::Router::new()` (will be filled in Task 4; for now a comment-free stub that compiles).
- `src/app_db/mod.rs` → empty `pub mod schema;` stub.
- `src/auth/mod.rs` → empty file.
- `src/config/mod.rs` → empty file.
- `src/engine/mod.rs` → empty file.
- `src/main.rs`:

```rust
fn main() {
    println!("rssea {}", rssea::version());
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test`
Expected: PASS (2 tests: `version_is_set` and the pre-existing `it_works` — keep `it_works` or delete it; if deleted, expect 1 test). Also `cargo build` succeeds.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "Phase 1: restructure to binary crate with module skeleton"
```

---

### Task 2: Config module (CLI/env parsing + data dir resolution)

**Files:**
- Modify: `src/config/mod.rs`
- Test: `src/config/mod.rs` (unit tests inline)

**Interfaces:**
- Consumes: `clap::Parser`.
- Produces:
  - `pub struct Config { pub data_dir: PathBuf, pub host: String, pub port: u16 }`
  - `impl Config { pub fn parse() -> Self }` (from clap, reading `--data-dir`, `--host`, `--port`; env defaults `RSSEA_DATA_DIR`, `RSSEA_HOST`, `RSSEA_PORT` via clap's `env` feature)
  - `impl Config { pub fn data_file(&self, name: &str) -> PathBuf }` → `data_dir.join(name)`
  - `pub fn default_data_dir() -> PathBuf` → `~/.local/share/rssea` on Linux (use `dirs`-free approach: `std::env::var("HOME")`), falling back to `.` if unset.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_data_dir_uses_home() {
        let home = std::env::var("HOME").expect("HOME set in test env");
        assert_eq!(
            default_data_dir(),
            PathBuf::from(format!("{home}/.local/share/rssea"))
        );
    }

    #[test]
    fn data_file_joins_under_data_dir() {
        let cfg = Config {
            data_dir: PathBuf::from("/tmp/rssea-test"),
            host: "127.0.0.1".into(),
            port: 3000,
        };
        assert_eq!(cfg.data_file("rssea.sqlite"), PathBuf::from("/tmp/rssea-test/rssea.sqlite"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config::tests`
Expected: FAIL — `Config`, `default_data_dir`, `data_file` don't exist.

- [ ] **Step 3: Implement**

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "rssea", version, about = "Self-hosted feed aggregator")]
pub struct Cli {
    #[arg(long, env = "RSSEA_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, env = "RSSEA_HOST", default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, env = "RSSEA_PORT", default_value_t = 3000)]
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn parse() -> Self {
        let cli = Cli::parse();
        Config {
            data_dir: cli.data_dir.unwrap_or_else(default_data_dir),
            host: cli.host,
            port: cli.port,
        }
    }

    pub fn data_file(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    pub fn ensure_data_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }
}

pub fn default_data_dir() -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/rssea"),
        _ => PathBuf::from("."),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test config::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/mod.rs && git commit -m "Phase 1: add config module with CLI/env parsing"
```

---

### Task 3: App DB initialization (sidecar schema)

**Files:**
- Modify: `src/app_db/mod.rs`
- Create: `src/app_db/schema.sql`
- Test: `src/app_db/mod.rs` (unit tests)

**Interfaces:**
- Consumes: `Config` (for data dir), rusqlite.
- Produces:
  - `pub struct AppDb { pub conn: rusqlite::Connection }`
  - `pub fn open(data_dir: &Path) -> anyhow::Result<AppDb>` (anyhow not in deps yet — use `Box<dyn std::error::Error>` or add `anyhow = "1"`. Prefer adding anyhow to Cargo.toml; it's standard.) Creates `rssea.sqlite` with WAL + FK on, runs `schema.sql`.
  - Schema tables (per spec §3.2): `saved`, `saved_tags`, `tags`, `sessions`, `settings`.
  - `impl AppDb { pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> }`
  - `impl AppDb { pub fn get_setting(&self, key: &str) -> Result<Option<String>> }`

- [ ] **Step 1: Add anyhow to Cargo.toml**

Add `anyhow = "1"` to `[dependencies]`.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rssea-test-{}", std::process::id()));
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
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test app_db::tests`
Expected: FAIL — `open`, `AppDb`, methods don't exist.

- [ ] **Step 4: Implement schema.sql**

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS saved (
    article_id TEXT PRIMARY KEY NOT NULL,
    saved_at TEXT NOT NULL,
    note TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS saved_tags (
    article_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (article_id, tag)
);

CREATE TABLE IF NOT EXISTS tags (
    tag TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
```

- [ ] **Step 5: Implement app_db/mod.rs**

```rust
use std::path::Path;

use rusqlite::Connection;

pub struct AppDb {
    pub conn: Connection,
}

impl AppDb {
    pub fn open(data_dir: &Path) -> anyhow::Result<AppDb> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("rssea.sqlite");
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(AppDb { conn })
    }

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
```

`schema.sql` lives at `src/app_db/schema.sql` next to `mod.rs` and is loaded via
`include_str!`. Do NOT declare `pub mod schema;` — the SQL is included inline at
compile time.

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test app_db::tests`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app_db/ Cargo.toml Cargo.lock && git commit -m "Phase 1: add sidecar app DB with schema"
```

---

### Task 4: Axum router + /api/health + logging + main wiring

**Files:**
- Modify: `src/api/mod.rs`
- Create: `src/api/health.rs`
- Modify: `src/main.rs`
- Test: `src/api/health.rs` (unit tests with tower ServiceExt) and/or `tests/health.rs` integration test

**Interfaces:**
- Consumes: `Config`, axum, tower (dev).
- Produces:
  - `pub fn router(config: Config) -> axum::Router` in `src/api/mod.rs`
  - `GET /api/health` → `200 {"status":"ok","version":"0.1.0"}` (JSON)
  - `pub async fn run(config: Config) -> anyhow::Result<()>` in `src/lib.rs` — binds listener, starts axum with tracing, serves.

- [ ] **Step 1: Write the failing test**

Create `tests/health.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::config::Config;
use std::path::PathBuf;
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok_with_version() {
    let cfg = Config {
        data_dir: PathBuf::from("/tmp/rssea-health-test"),
        host: "127.0.0.1".into(),
        port: 3000,
    };
    let app = rssea::api::router(cfg);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], "0.1.0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test health`
Expected: FAIL — `rssea::api::router` doesn't exist / doesn't take Config.

- [ ] **Step 3: Implement api/health.rs and api/mod.rs**

`src/api/health.rs`:

```rust
use axum::Json;
use serde_json::{Value, json};

pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": crate::version(),
    }))
}
```

`src/api/mod.rs`:

```rust
pub mod health;

use crate::config::Config;
use axum::routing::get;

pub fn router(_config: Config) -> axum::Router {
    axum::Router::new().route("/api/health", get(health::health))
}
```

- [ ] **Step 4: Implement run() in src/lib.rs**

```rust
pub async fn run(config: Config) -> anyhow::Result<()> {
    config.ensure_data_dir()?;
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("rssea {} listening on {}", crate::version(), listener.local_addr()?);
    let app = api::router(config);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 5: Wire main.rs**

```rust
use rssea::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rssea=info,tower_http=info".into()),
        )
        .init();

    let config = Config::parse();
    rssea::run(config).await
}
```

(Note: `Config::parse()` uses clap `Parser`; main calls it before run.)

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --test health`
Expected: PASS. Also `cargo build` clean.

- [ ] **Step 7: Manual smoke check**

Run: `cargo run -- --data-dir /tmp/rssea-smoke &` then `curl -s localhost:3000/api/health`; expect `{"status":"ok","version":"0.1.0"}`; kill the process.

- [ ] **Step 8: Commit**

```bash
git add src/lib.rs src/main.rs src/api/ tests/ && git commit -m "Phase 1: add axum router with /api/health and main wiring"
```

---

### Task 5: Makefile + dev workflow + engine/auth stubs

**Files:**
- Create: `Makefile`
- Modify: `.gitignore`
- Modify: `src/engine/mod.rs`, `src/auth/mod.rs` (tiny compile-check stubs)

**Interfaces:**
- Consumes: nothing new.
- Produces: `make dev`, `make build`, `make run`, `make test`, `make clean`. `.gitignore` covers `/target`, `/frontend/node_modules`, `/frontend/dist`, `/data`, `*.sqlite*`.

- [ ] **Step 1: Write Makefile**

```makefile
.PHONY: dev build run test clean frontend-dev frontend-build

DATA_DIR ?= $(HOME)/.local/share/rssea
BACKEND_PORT ?= 3000
FRONTEND_PORT ?= 5173

dev: backend-dev frontend-dev

backend-dev:
	cargo watch -x "run -- --data-dir $(DATA_DIR)"

frontend-dev:
	@echo "Frontend dev server (bun) is added in Phase 4. Run: cd frontend && bun run dev"

build:
	cd frontend && bun run build
	cargo build --release

run:
	cargo run --release -- --data-dir $(DATA_DIR)

test:
	cargo test
	cd frontend && bun run typecheck && bun run lint

clean:
	cargo clean
	rm -rf frontend/dist frontend/node_modules

$(shell mkdir -p $(DATA_DIR))
```

Note: `cargo watch` requires the `cargo-watch` tool; note in a comment-less way that if absent, run `cargo install cargo-watch` — put this in the Makefile as a target `install-watch`:

```makefile
install-watch:
	cargo install cargo-watch
```

- [ ] **Step 2: Update .gitignore**

```
/target
/data
*.sqlite
*.sqlite-wal
*.sqlite-shm
/frontend/node_modules
/frontend/dist
```

- [ ] **Step 3: Engine and auth stubs**

`src/engine/mod.rs`:

```rust
pub mod sync;
```

`src/engine/sync.rs`:

```rust
// Placeholder: sync engine will own NewsFlash and spawn_blocking bridge (Phase 2).
```

`src/auth/mod.rs`:

```rust
// Placeholder: password setup, session cookie, middleware (Phase 3).
```

- [ ] **Step 4: Verify everything builds**

Run: `cargo build` and `cargo test`
Expected: PASS. `make test` should also pass (frontend targets fail gracefully since `frontend/` has no package.json yet — guard `make test` to skip frontend if `frontend/package.json` missing; update Makefile test target accordingly):

```makefile
test:
	cargo test
	@if [ -f frontend/package.json ]; then cd frontend && bun run typecheck && bun run lint; else echo "frontend not present; skipping frontend checks"; fi
```

- [ ] **Step 5: Commit**

```bash
git add Makefile .gitignore src/engine/ src/auth/ && git commit -m "Phase 1: add Makefile dev workflow and module stubs"
```

---

## Self-Review Notes

- Spec coverage: Phase 1 delivers scaffolding only (spec §8, §9). Nothing else is claimed.
- Placeholder scan: the two comment-only stubs (`engine/sync.rs`, `auth/mod.rs`) are deliberate "exists so the crate compiles" stubs, replaced in Phases 2–3. All other steps contain real code.
- Type consistency: `Config { data_dir, host, port }` used identically in Task 2, 4, and tests/health.rs. `router(config: Config)` signature consistent.
- Dependency floor: axum 0.8.9, tokio 1.53.1, rusqlite 0.40.2, clap 4.6.6, reqwest 0.13.4 — exact pins as verified from crates.io.
