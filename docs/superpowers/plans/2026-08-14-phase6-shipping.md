# Phase 6: Integration & Shipping — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rssea as a single self-contained executable. The built frontend (`frontend/dist`) is embedded into the Rust binary via `rust-embed`, served at `/` with an SPA fallback so deep links work, alongside the existing `/api/*` and `/img`. PWA icons added. Build/run/test Makefile targets made real. README + systemd notes. End-to-end smoke test of the built binary.

**Architecture:** The Rust axum server gains a static-assets handler backed by `rust-embed` (folder `frontend/dist`, built by bun before `cargo build`). The router serves: `/api/*`, `/img`, and `/img`-adjacent, plus static files at `/` — with an SPA fallback (any non-API, non-/img GET that doesn't match a static file → serve `index.html` so `/feeds/:id` deep links work). `make build` runs `bun run build` then `cargo build --release`. A `--data-dir` + port are the only runtime config. README documents dev (two-process) vs prod (single binary), systemd unit, and first-run password.

**Tech Stack:** rust-embed 8.12.0 (Rust), existing frontend (Vite + vite-plugin-pwa). Icons: generate a simple SVG→PNG icon set (or use `@vite-pwa/assets-generator` if quick; else hand-write a small set of PNG icons in `frontend/public/`).

## Global Constraints

- No comments in code unless a task explicitly shows them.
- The SPA fallback MUST NOT intercept `/api/*` or `/img*` — those stay on their existing handlers. Fallback applies only to GET requests that don't match an API/proxy route or an actual static asset (path with a file extension that exists).
- The frontend `dist/` is embedded at compile time — the release binary is a single file with no external assets. Dev keeps the two-process flow (vite proxy) unchanged.
- PWA manifest `icons` filled with real icons; `background_color` should match the app theme (the app is dark-themed; the manifest currently says `#ffffff` — fix it, and check `theme_color`).
- The embedded server must handle `index.html` at `/`, asset paths (hashed names), and the SPA fallback for client-side routes.
- Add `Content-Type` correctly for served assets (rust-embed needs a mime guesser or a small map).
- Commit after each task with the exact message given.

---

### Task 1: Embed frontend assets with rust-embed + SPA fallback

**Files:**
- Modify: `Cargo.toml` (add `rust-embed = "8.12.0"`, `mime_guess = "2"` if needed), `src/assets.rs` (new), `src/lib.rs` (wire static handler + fallback), `src/api/mod.rs` (static routes)
- Create: `src/static/` placeholder note (the `frontend/dist` folder is the source; rust-embed embeds it at compile time — verify the build.rs/embed pattern)
- Test: `tests/static.rs` (integration): GET `/` → 200 index.html; GET a hashed asset → 200 + correct content-type; GET `/feeds/<someid>` (SPA fallback) → 200 index.html; GET `/api/health` still works; GET `/img?...` still works.

**Interfaces:**
- Consumes: `frontend/dist` (must exist at compile time — document that `make build` runs bun first).
- Produces:
  - `src/assets.rs`: `#[derive(RustEmbed)] #[folder = "frontend/dist"] struct Assets;` + `pub fn serve(path: &str) -> Option<Response>` returning the file bytes with a guessed mime type, or `index.html` for the SPA fallback.
  - In `src/api/mod.rs` (or a top-level router in lib.rs): a fallback route `get(fallback)` that: for paths starting `/api` or `/img` → pass through (should never reach here, but guard); else if a real asset exists → serve it; else serve `index.html`. Implement via `Router::fallback` so it only catches unmatched routes.

- [ ] **Step 1: Verify `frontend/dist` exists and is committed** — check the Phase 4/5 build output is present or add a build step. The repo should NOT rely on a pre-existing dist: `make build` must build it. For the Rust compile to succeed, either (a) commit a built `dist/` (bad — churn), or (b) use a build script that errors with a clear message if dist is missing, or (c) use `rust-embed` with `debug-embed` off + a `build.rs` that runs nothing but the Makefile guarantees the order. Choose (c): document in the plan that `make build` and `make dev` must run bun before cargo; the integration test for this task builds dist first (or checks the existing one). Simplest robust choice: a `build.rs` that checks `frontend/dist/index.html` exists and prints a clear error if not (so a bare `cargo build` fails with "run bun run build first" instead of a confusing embed error).

- [ ] **Step 2: Write the failing test `tests/static.rs`** (GET /, asset, SPA fallback, /api/health, /img).
- [ ] **Step 3: Implement `src/assets.rs` + fallback + build.rs guard**.
- [ ] **Step 4: Verify** — `bun run build` (dist present) then `cargo test --test static` + full suite; confirm content-types and fallback.
- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock build.rs src/assets.rs src/lib.rs src/api/mod.rs tests/static.rs && git commit -m "Phase 6: embed frontend assets with rust-embed and SPA fallback"
```

---

### Task 2: PWA icons + manifest fixes

**Files:**
- Modify: `frontend/vite.config.ts` (manifest icons + background_color/theme_color), `frontend/public/` (add `pwa-192x192.png`, `pwa-512x512.png`, `maskable-512x512.png`, `favicon.svg`/`.ico`)
- Test: build → inspect generated `manifest.webmanifest`; CDP: installable PWA (manifest has icons; `display: standalone`).

**Interfaces:**
- Consumes: nothing new.
- Produces: real icons referenced in the manifest with correct `sizes`/`purpose`; `background_color` dark to match the app; `theme_color` verified.

- [ ] **Step 1: Generate icons** — simplest: create an SVG logo (a simple RSS/feed glyph) and rasterize to 192/512 + maskable using whatever tool is available (ImageMagick `convert`, `rsvg-convert`, or `@vite-pwa/assets-generator` via bun). If no rasterizer is available, embed a small base64 PNG or use the PWA assets generator package. Keep it simple and deterministic.
- [ ] **Step 2: Update vite.config.ts manifest** (icons array, background_color `#0a0a0a` or the actual dark color used, theme_color) and place the icons in `frontend/public/`.
- [ ] **Step 3: Verify** — `bun run build` → inspect `dist/manifest.webmanifest`; CDP installability check (Lighthouse or manual: manifest loads, icons resolve, no console errors).
- [ ] **Step 4: Commit**

```bash
git add frontend/vite.config.ts frontend/public && git commit -m "Phase 6: add PWA icons and fix manifest theme"
```

---

### Task 3: Makefile build/run/test workflow + README + systemd notes

**Files:**
- Modify: `Makefile` (build order: bun then cargo; run; test incl. frontend; clean; data dir), `.gitignore` (keep frontend/dist ignored — the embed is at compile time, not committed)
- Create: `README.md`, `docs/selfhosting.md` (or fold into README): dev flow (two terminals), prod flow (single binary), first-run password, systemd unit example, data dir layout, config env vars (`RSSEA_DATA_DIR`, `RSSEA_HOST`, `RSSEA_PORT`, `RSSEA_ALLOW_PRIVATE_PROXY`), troubleshooting.

**Interfaces:**
- Consumes: the Phase 6 binary.
- Produces: `make build` → single executable; `make run` → runs it; `make test` → cargo + frontend checks (with the guard from Phase 1); `make dev` → two-process dev (backend + vite proxy) — update the Phase 1 stub to actually start vite.

- [ ] **Step 1: Rewrite the Makefile** — verify the Phase 1 Makefile's current targets (dev/build/run/test/clean/install-watch) and make them correct for the embedded build. `make dev` should run backend (cargo run) + frontend (bun run dev) concurrently — use a simple `&`/`wait` or a small script. `make build` = `cd frontend && bun run build && cd .. && cargo build --release`. Keep the frontend-presence guard in `make test`.
- [ ] **Step 2: Write README.md + docs** (dev/prod, first-run, env, systemd, troubleshooting). Include the GPL-3.0 note (news-flash is GPL-3.0-or-later) so distribution is done right.
- [ ] **Step 3: Verify** — `make build` produces `target/release/rssea`; run it, curl `/` (200, html), `/api/health`, `/feeds/anything` (200, SPA fallback); `make test` passes.
- [ ] **Step 4: Commit**

```bash
git add Makefile README.md docs/ .gitignore && git commit -m "Phase 6: add single-binary build workflow, README, and self-hosting docs"
```

---

### Task 4: End-to-end smoke test of the release binary

**Files:**
- Create: `docs/smoke-test.md` (or a `scripts/smoke.sh`) — a checklist/script that exercises the built binary.
- Test: run the release binary against a fresh data dir, drive the real flows.

**Interfaces:**
- Consumes: the Phase 6 binary.
- Produces: evidence that the shipped artifact works standalone: first-run password in logs, login via API, add a feed, sync, overview/saved/settings round-trips, static + SPA fallback, PWA manifest. Optionally run against a public feed URL if network is available, else the local RSS fixture approach.

- [ ] **Step 1: Write `scripts/smoke.sh`** — starts the binary with a temp data dir, waits for /api/health, POSTs /api/session, POSTs /api/login with the printed initial password (parse from logs), adds a local fixture feed (or public feed), triggers refresh-all, asserts overview non-empty, saves an article, GETs / (200) + a deep link (200), then kills the process. Exit non-zero on any failed assert.
- [ ] **Step 2: Run it against a real `cargo build --release`** and fix any failures (this is the real verification of the whole pipeline).
- [ ] **Step 3: Commit**

```bash
git add scripts/smoke.sh docs/smoke-test.md && git commit -m "Phase 6: add end-to-end smoke test for the release binary"
```

---

## Self-Review Notes

- Spec coverage: Phase 6 completes the roadmap — single binary that embeds the frontend and serves web+api (spec §1 goal), `make build`/`dev`/`test` (spec §8), PWA icons + manifest, README/systemd. The last parked items from Phases 4-5 (silent mutation errors, slide-over a11y, Overview items inert, misc polish) are documented in the roadmap as post-ship polish — NOT blocking.
- Placeholder scan: no TBDs. The `src/static/` note in Task 1 is resolved by the build.rs guard decision.
- Type consistency: the fallback route must not shadow `/api/*` or `/img` — verified by the tests in Task 1 and the smoke test. `frontend/dist` is git-ignored; `make build` guarantees the order.
- Design decisions (not defects): SPA fallback only for non-API GETs; `Content-Type` via mime_guess; build.rs errors with a clear message if dist is missing; icons as simple generated PNGs; GPL-3.0 note in README (news-flash is GPL-3.0-or-later).
