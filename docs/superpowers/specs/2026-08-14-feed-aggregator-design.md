# rssea — Feed Aggregator (PWA + Rust backend)

Date: 2026-08-14
Status: Approved design

A single-binary, self-hosted feed aggregator. An async Rust backend (axum + news-flash)
aggregates, regularly syncs, and serves a web + API. The frontend is an installable PWA
built with React + HeroUI, managed by bun.

## 1. Goals & non-goals

### Goals
- Single binary that embeds the frontend and serves it together with a JSON API.
- Background, scheduled sync of subscribed RSS/Atom feeds.
- Rich reading experience: 3-panel desktop layout, bottom-nav mobile layout, reader
  view with rendered HTML and image proxying.
- Full-text search with live suggestions and on-enter full search.
- Saved items with notes and tags, grouped by month.
- OPML import/export, manual source add with title auto-fetch.
- Installable PWA fetching data from the API as needed.

### Non-goals (v1)
- Multi-user / accounts.
- Offline article data caching (service worker caches app shell only, not API data).
- Push notifications.
- External feed-service backends (FreshRSS, Feedly, etc.). Only local RSS/Atom.

## 2. Deployment & access model

- **Self-hosted server, single user.** Runs on an always-on box (NAS, VPS, Raspberry Pi).
- **Single password login.** First-run generates a random setup token printed to logs
  (or set via env `RSSEA_PASSWORD`). Browser POSTs password, receives an HttpOnly
  session cookie. All `/api/*` (except login/session) requires the session.
- Password stored as argon2 hash in our app DB.
- Data dir default `~/.local/share/rssea`, overridable via CLI/env.

## 3. Architecture (Approach A: news-flash engine + custom sidecar)

Two SQLite databases, linked by news-flash string IDs (`feed_id`, `article_id`,
`category_id`, `tag_id`):

```
┌──────────────────────────────────────────────────────────────┐
│  rssea  (single Rust binary: axum server)                    │
│                                                              │
│  ┌───────────┐  ┌────────────────────────────────────────┐   │
│  │  REST API │  │  Sync engine (wraps news-flash)        │   │
│  │  (axum)   │  │  - NewsFlash instance (local_rss)      │   │
│  │           │  │  - periodic sync scheduler (tokio)     │   │
│  └─────┬─────┘  └──────────┬─────────────────────────────┘   │
│        │                     │                                │
│        │        ┌────────────┴──────────────┐                 │
│        │        │  Service layer            │                 │
│        │        │  (spawn_blocking bridge)  │                 │
│        │        └────────────┬──────────────┘                 │
│        │                      │                                │
│  ┌─────┴──────────────────────┴──────┐                        │
│  │  news-flash DB (SQLite, engine's) │                        │
│  │  - feeds, articles, categories    │                        │
│  │  - tags, favicons, thumbnails     │                        │
│  └───────────────────────────────────┘                        │
│                                                                 │
│  ┌───────────────────────────────────┐   ┌──────────────────┐  │
│  │  App DB (SQLite, our own schema)  │   │  Static assets   │  │
│  │  - notes, saved-timestamps        │   │  (embedded PWA)  │  │
│  │  - settings, sessions, proxy cache│   └──────────────────┘  │
│  └───────────────────────────────────┘                          │
│  Static: embedded frontend → served at /                       │
│  API: /api/* → JSON; images proxied at /img?u=...              │
└──────────────────────────────────────────────────────────────┘
```

### 3.1 News-flash engine ownership
- `NewsFlash` instance (`local_rss` plugin) created headless, held by one long-lived
  actor (tokio task) that serializes access.
- All calls into news-flash and CPU-heavy work (parsing, scraping, HTML rewrite) run
  via `spawn_blocking` to avoid blocking the async runtime.
- news-flash provides: feed fetch/discovery, dedup, favicons, thumbnails, tags,
  categories, read/marked flags, FTS4 search, OPML import/export, article scraping
  (feature-gated), per-host rate limiting.

### 3.2 Custom sidecar (app DB) schema
Linked by news-flash IDs. Contents:
- `saved` — `article_id` (PK, FK to news-flash article id), `saved_at`, `note`
  (nullable), `updated_at`.
- `saved_tags` — `(article_id, tag)` PK; free-form tags applied at save time.
- `tags` — distinct tag names for autocomplete in the UI.
- `sessions` — session tokens (hashed), created_at, expires_at.
- `settings` — key/value app settings (sync interval, keep-articles duration, theme,
  password hash).
- `proxy_cache` — optional on-disk cache metadata for proxied images.

The "saved" flag in the timeline is news-flash's `marked`; our `saved` table stores the
extra save-time metadata (when saved, note, tags). Tags are our own free-form strings,
not news-flash's Tag model, to stay independent of its offline-sync queue.

## 4. REST API (`/api/*`)

All JSON, camelCase, session-cookie protected except login/session. Errors:
`{error: {code, message}}` with appropriate HTTP statuses (400/401/404/409/429/500).

### Auth
- `POST /api/login` `{password}` → sets cookie
- `POST /api/logout`
- `GET /api/session` → whoami, app version, setup status

### Overview
- `GET /api/overview` → `[{categoryId, name, totalCount, unreadCount, items: [headline…10]}]`
  plus implicit "All". Headlines are lightweight: id, title, feed, date, thumbnail, source.

### Feeds / timeline
- `GET /api/feeds` → feeds with unread counts
- `GET /api/articles?feed=&category=&saved=&unread=&tag=&search=&offset=&limit=`
  → timeline headlines (infinite scroll paging)
- `GET /api/articles/:id` → full article (rendered HTML + metadata + our note/tags)
- `PATCH /api/articles/:id` `{read?, saved?}`
- `POST /api/articles/:id/save` `{note?, tags?}`; `PUT /api/articles/:id/save` edits note+tags
- `POST /api/articles/:id/read` toggle; `POST /api/articles/:id/mark-saved`

### Categories
- `GET /api/categories` → tree with unread counts
- `POST/PATCH/DELETE /api/categories[/:id]`
- `POST /api/categories/:id/read` → mark-all-read in category

### Sources
- `GET /api/sources` → grouped by category (with action menu data)
- `POST /api/sources` `{url, title?, categoryId?}` — fetches feed, discovers title
- `POST /api/sources/discover` `{url}` → discovered title (for "Fetch title" button)
- `POST /api/sources/import-opml` `{opml}`
- `GET /api/sources/export-opml`
- `PATCH /api/sources/:id` (rename, recategorize); `DELETE /api/sources/:id`
- `POST /api/sources/:id/refresh`; `POST /api/sources/:id/read` (mark-all-read)

### Saved
- `GET /api/saved` → grouped by month: `[{month: "2026-08", items:[…]}]`, newest first
- `GET /api/tags` → tag list (for autocomplete)

### Settings
- `GET/PATCH /api/settings` — theme, sync interval, keep-articles-duration, change
  password, stats (feeds, articles, DB size, last sync). Server port/address are
  startup config (env/CLI), not runtime settings.

### Search
- `GET /api/search/suggestions?q=` → top 5–8 matches (title + source + thumbnail) for
  the live dropdown, debounced client-side (~300ms).
- Full search on Enter uses `GET /api/articles?search=...` (news-flash FTS +
  source-name match).

### Media
- `GET /img?u=<absolute-url>` → proxied image (see 6).
- `GET /api/favicon/:feedId`, `GET /api/thumbnail/:articleId` → serve news-flash cached blobs.

## 5. Sync engine

- **Scheduler:** tokio interval task. Runs `spawn_blocking` → news-flash `sync()` for
  all feeds, or `fetch_feed(id)` for individual refresh. Default interval: **30 minutes**
  (configurable in Settings). Immediate sync after adding a source, importing OPML, or
  pressing Refresh.
- **Concurrency:** a mutex ensures scheduler and API-triggered refreshes never overlap;
  news-flash serializes internally too.
- **Rate limiting:** reuse news-flash's built-in per-host delays during sync.
- **Content pipeline:** on full-article read, if `scraped_content` missing, call
  news-flash's scraper via `spawn_blocking`, then run our HTML rewrite pass.
- **Article retention:** default **keep everything (no pruning)**. Configurable in
  Settings; saved articles always preserved regardless.

## 6. HTML rewrite & image proxy

- **Rewrite pass** (`engine/content.rs`): parse article HTML (use `scraper` crate +
  news-flash `relative_url_evaluater`):
  - `img src`/`srcset`, `a href` → absolute against article URL
  - `<img>` → `src="/img?u=<abs>&r=<feedId>"` so images route through our proxy
  - preserve `data-original`/title for hover (target URL shown)
- **Proxy endpoint** `GET /img?u=..`: fetch upstream with a cached client
  (ETag/If-Modified-Since), stream back with correct content-type; optional on-disk
  cache with a cap. Only http/https allowed; DNS-rebinding guard.

## 7. Frontend

### Stack
React 18 + TypeScript + HeroUI (React Aria based), `@tanstack/react-query` (server
state + infinite queries), `react-router`, `vite-plugin-pwa` (installable, auto-update),
`clsx`/Tailwind via HeroUI preset. Managed by bun.

### Structure
```
frontend/
  package.json / vite.config.ts (dev proxy /api → :3000)
  public/ (manifest, icons) / src/
    main.tsx, App.tsx (shell: desktop 3-panel / mobile bottom nav)
    routes/ (overview, feeds, saved, sources, settings, reader)
    api/ (typed fetch client), state/ (React Query), components/, hooks/
```

### Layout
- **Desktop (lg+):** 3 columns: sidebar (nav + categories + source tree) | timeline |
  reader. Timeline and reader independently scrollable. Overview active → 2 columns.
- **Mobile (<lg):** bottom nav: Feed, Sources, Overview, Saved, Settings. Reader opens
  as slide-up/full page.
- **Sidebar (desktop):** Section 1: Overview, Feeds, Saved. Section 2 (Feeds active):
  Categories (scrollable). Section 3 (Feeds active): Sources tree (scrollable). Then
  spacer → Help, Settings bottom. Lists scroll independently.

### Pages
- **Overview:** category cards; header shows name + total count; body shows 10 items
  (title, age, source); "More" button → Feeds filtered by category. Mobile: search bar top.
- **Feeds/timeline:** search bar top; infinite scroll list; item: source thumbnail
  (favicon/alpha avatar) left, title + age + source + content-thumbnail right. Selected →
  reader pane.
- **Reader:** title + metadata (source, date, original link, age); actions Read/Unread,
  Save, Open, Share — top on desktop, bottom on mobile. Rendered HTML with absolute
  links + proxied images; hover shows target URL. Note + tag editing on save.
- **Sources:** desktop popover from sidebar; mobile full page. Add-source form with
  Fetch-title + OPML import. Sources grouped by category; 3-dot menu (Open, Edit,
  Delete, Refresh, Mark all read).
- **Saved:** grouped by month, newest first; save-time, note snippet, tags; opens reader.
- **Settings:** theme (light/dark/system), sync interval, keep-articles, change password,
  stats (feeds, articles, DB size, last sync), links to repo + issues at bottom.

### Search (live + full)
- Typing → debounced 300ms → `GET /api/search/suggestions?q=` → top 5–8 in a dropdown;
  click navigates to article.
- **Enter** → full search: `GET /api/articles?search=...` filtered timeline.

### Infinite scroll
- `useInfiniteQuery` (offset/limit, page size ~30) + `useInfiniteScroll` hook with a
  sentinel observer. Prefetch next page when within ~250px of the bottom (proactive,
  not after hitting the end). React Query caches pages.

### PWA
- `vite-plugin-pwa`: manifest + icons, installable, auto-update. SW caches app shell
  only; no `/api` caching in v1.

## 8. Build & dev/prod workflow

- `frontend/package.json` scripts: `dev` (vite, HMR), `build` (vite build → dist/),
  `typecheck` (tsc --noEmit), `lint` (eslint), `preview`.
- Root `Makefile` (or `justfile`):
  - `make dev` → backend `cargo run` (:3000) + frontend `bun run dev` (:5173, proxy),
    run concurrently with watch.
  - `make build` → `bun run build` then `cargo build --release` (dist/ embedded via
    `rust-embed`/`include_dir!` at compile time) → single executable.
  - `make run` → run the release binary.
  - `make test` → `cargo test` + `bun typecheck` + `bun run lint`.
- Cargo.lock and bun.lockb committed.

## 9. Crate structure

```
src/
  main.rs        — CLI (serve, init-password), setup, startup
  config.rs      — env/flag/config settings
  auth.rs        — password setup, session cookie, middleware
  engine/
    mod.rs       — owns NewsFlash, spawn_blocking bridge
    sync.rs      — periodic sync scheduler
    content.rs   — HTML rewrite (absolute URLs, proxy images)
  api/
    mod.rs       — axum router
    feeds.rs, categories.rs, articles.rs, saved.rs, sources.rs, settings.rs
  app_db/        — our SQLite schema (rusqlite in worker thread)
    schema.sql, saved.rs, sessions.rs, settings.rs, proxy_cache.rs
  proxy.rs       — image proxy endpoint
  assets.rs      — rust-embed static serving
  static/        — embedded frontend (built by bun)
frontend/        — React + HeroUI + Vite + bun (the PWA)
```

## 10. Error handling & testing

- JSON error envelope; news-flash errors mapped to our codes; per-feed sync errors
  surfaced via news-flash `error_count`/`error_message` in the UI.
- Backend: unit tests for rewrite pass, auth, app DB; integration tests for API routes
  (using a temp data dir).
- Frontend: typecheck + lint in CI; component smoke tests optional later.
