# rssea — Phased Implementation Roadmap

Date: 2026-08-14
Status: Active (user AFK; driven autonomously with subagent verification)

Spec: `docs/superpowers/specs/2026-08-14-feed-aggregator-design.md`

## Operating principles

- Orchestrator (main session) defines phases, writes per-phase plans, dispatches
  subagents, and verifies claims before settling on them.
- Every phase ends with working, testable software and a commit.
- Risky technical claims are cross-verified by a second subagent before being trusted.
- No scope creep: build to the spec; note deviations in a phase's report.

## Phases

| Phase | Name | Deliverable / Definition of Done |
|-------|------|----------------------------------|
| 0 | **Research & de-risking** | **DONE (2026-08-14).** Verified: news-flash headless viability (YES, `create()` on empty dir works, local_rss needs no login, reqwest 0.13 required, sync idempotent); FTS broken in 3.2.0 (fix: sidecar read-only connection with `rowid IN (SELECT rowid ...)`); no feed autodiscovery via add_feed (use `feed_parser::download_and_parse_feed`); no per-category counts (sidecar SQL JOIN); all news-flash calls blocking → spawn_blocking. Frontend stack verified: HeroUI v3.2.4 (React ≥19, Tailwind v4, no provider/framer-motion), TS 6.0.3, react-router 8.3.0, vite-plugin-pwa 1.3.0 + workbox 7.4.1. See spec §3.1 and the Phase 0 reports. |
| 1 | **Project scaffolding** | **DONE (2026-08-14).** `make dev` runs backend on :3000 serving `GET /api/health` (`{"status":"ok","version":"0.1.0"}`). App DB (`rssea.sqlite`) initializes with saved/saved_tags/tags/sessions/settings. Config/CLI parsing + logging working. Axum router wired. 6/6 tests, clippy clean, merged to main (branch `feat/phase1-scaffolding`). Plan: `docs/superpowers/plans/2026-08-14-phase1-scaffolding.md`. |
| 2 | **Engine bridge + first sync** | `NewsFlash` local_rss instance boots headless in a background actor. Adding a source works; sync pulls articles; feeds + article headlines readable through our API. Favicon/thumbnail endpoints working. |
| 3 | **Full backend API** | Complete `/api/*` surface per spec §4: auth/session, overview, timeline w/ paging+search+suggestions, saved (notes/tags/months), categories, sources CRUD + OPML, settings, media endpoints. Integration tests green. |
| 4 | **Frontend shell + PWA** | Vite + bun + HeroUI scaffold; installable PWA; desktop 3-panel and mobile bottom-nav shells; routing + typed API client + React Query wiring; search bar + infinite-scroll hook infrastructure. |
| 5 | **Frontend pages** | Overview, Feeds/timeline, Reader, Sources, Saved, Settings pages implemented against real API. Search suggestions dropdown + full search. |
| 6 | **Integration & shipping** | Frontend embedded into binary (`rust-embed`), `make build` produces single executable; README + systemd/install notes; end-to-end smoke test of built binary; final review. |

## Dependencies

```
Phase 0 → 1 → 2 → 3
Phase 0 → 4 → 5
Phase 3 + 5 → 6
```

Phase 0 frontend research (4) can proceed before backend phases finish. Phases 2 and 3
must not change the API contract shape that Phase 5 depends on (frozen in Phase 3).

## Cross-phase invariants

- Backend: async axum; all news-flash and CPU-heavy work via `spawn_blocking`.
- Data: two SQLite DBs (news-flash's + our sidecar), linked by news-flash string IDs.
- Auth: single password → HttpOnly session cookie.
- API shape: camelCase JSON, `{error:{code,message}}` envelope, frozen in Phase 3.
- Frontend: React 18 + TS + HeroUI + TanStack Query + react-router + vite-plugin-pwa; managed by bun.
- Paging: offset/limit (page ~30), proactive prefetch.
