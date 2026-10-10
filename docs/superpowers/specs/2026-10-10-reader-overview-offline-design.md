# Feedea reader / overview / offline-cache design

Date: 2026-10-10
Status: approved in-chat, awaiting spec review
Scope: 3 independent pieces, implement in order reader → overview → offline.

## 1. Intent

- Mobile reader must not be obscured by bottom bars.
- Overview must stay usable: sort toggle (alpha / recent) + hide-read toggle.
- Offline reading: cache overview headlines + full article bodies, plus most
  recent N articles. Configurable in Settings with two toggles + N.

Non-goals: full offline write support (mark-read/saving offline queues),
shared-device hardening beyond cache-clear on logout.

## 2. Reader overlap fix (bounded)

Observation: `MobileLayout.tsx` already hides `BottomNav` when a reader is
open. The overlap is Reader's own `fixed inset-x-0 bottom-0` action bar
(`Reader.tsx`) covering the end of article content.

Change (`frontend/src/components/Reader.tsx` only):

- Keep the fixed action bar (mobile UX), fix spacing:
  - content container gets bottom padding covering bar height + safe area,
    e.g. `pb-[calc(5rem+env(safe-area-inset-bottom))]` plus
    `scroll-padding-bottom` equivalent so scrolled-to-end content clears bar.
  - bar itself gets `pb-[env(safe-area-inset-bottom)]` so it sits above the
    home indicator.
- No change to `MobileLayout` / `DesktopLayout`.

Acceptance: on a mobile viewport the last paragraph and note box scroll fully
above the action bar; desktop unchanged.

## 3. Overview sort + hide-read (bounded)

Current: cards arrive in server order (`CategoryIndex::card_ids`: categories
table order filtered to direct set). Client renders as-is (`Overview.tsx`).

### 3.1 Sort toggle

- Client-side only, no backend change.
- Toolbar row under the Overview header: segmented `Alpha | Recent`, subtle
  styling matching existing cards.
- Persist in `localStorage` key `feedea:overview-sort` (`alpha` default).
- `Recent` = sort cards desc by `max(item.date)`; ties / empty cards fall back
  to name comparison. `Alpha` = server order untouched.

### 3.2 Hide-read (server unread-only, per user choice)

- New query param: `GET /api/overview?unread_only=1`.
- Backend (`src/api/overview.rs`): when set, per-card `ArticleFilter` adds
  `unread: Some(Read::Unread)`. Totals (`total_count`, `unread_count`) and the
  `all` block are unchanged so headers keep showing counts.
- Frontend (`state/hooks.ts`, `Overview.tsx`): `useOverview(unreadOnly: bool)`,
  query key `["overview", flag]`; toggle switch "Hide read" persisted in
  `localStorage` key `feedea:overview-hide-read` (default off).
- Cards with zero items under the filter show an "All caught up" placeholder
  instead of disappearing (avoids layout jump).

Useful tests only (per user instruction): backend unit test that the
`unread_only` filter maps to `Read::Unread` for card queries and leaves totals
untouched; frontend typecheck/lint (no brittle snapshot tests).

## 4. Offline cache (architectural, approach A: Workbox runtime + warmer)

Rejected: (B) TanStack persist-client — extra dep, no image caching;
(C) custom IndexedDB store — most code/invalidation burden.

### 4.1 Service worker (`frontend/vite.config.ts`)

Extend `VitePWA.workbox` with `runtimeCaching` (same-origin only):

- `/api/overview`, `/api/articles*` (list + detail): `NetworkFirst`,
  `networkTimeoutSeconds: 3`, expiration 7 days, max ~200 entries,
  `cacheableResponse: [200]`.
- `/img/*`, `/api/thumbnail*` or equivalent thumbnail paths:
  `StaleWhileRevalidate`, 30-day expiration.
- App shell stays as-is (`globPatterns`, `navigateFallback`).

Note: cookie-authenticated GETs are cached per user. Mitigation: on logout,
delete runtime API caches via `caches.delete()` before redirect.

### 4.2 Warmer hook (frontend)

New `useOfflineWarmer` (or equivalent module under `frontend/src/pwa/`):

- Reads offline settings from `useSettings()`.
- After `useOverview()` success: if `offline_cache_overview`, fetch detail
  (`/api/articles/:id`) for each overview card item id (deduped).
- If `offline_cache_recent`: fetch `GET /api/articles?limit=N` (global newest,
  `NewestFirst` by published — existing `articles::list` default), then fetch
  details for those ids (deduped against overview set).
- Cap total detail fetches (e.g. `min(N + overviewItems, 250)`), use
  `Promise.allSettled` with small concurrency, silent failures (offline-first:
  never block rendering).
- Trigger on overview mount + on settings change; never on every render.

This warms both the Workbox HTTP cache (real network responses) and the
TanStack query cache, so the Reader opens offline.

### 4.3 Settings (backend + frontend)

Backend uses the existing generic `settings` kv table (`app_db/mod.rs`):

- New keys + accessors: `offline_cache_overview: bool` (default true),
  `offline_cache_recent: bool` (default false), `offline_recent_count: i64`
  (default 50, clamp 1–200 on write).
- `GET /api/settings` includes the three fields; `PATCH /api/settings`
  accepts/validates them (`UpdateSettings` extended).
- `dto::Settings` extended with the same three fields.

Frontend `Settings.tsx`: new "Offline" section with two switches + numeric
input for N (disabled when recent toggle off), wired through
`useUpdateSettings` patch + existing save affordance.

Acceptance: enable both toggles, load overview online, go offline (devtools),
reload → overview list renders from SW, opening a cached article shows full
body; disabling toggles stops warming (no assertion on eviction timing).

## 5. Execution order and verification

Order: (1) reader padding → (2) overview backend param + frontend toggles →
(3) SW config + settings + warmer.

Verification per piece:

- Reader: manual mobile-viewport scroll check; `bun run typecheck`.
- Overview: `cargo test` (new filter test), `bun run typecheck` + `lint`.
- Offline: `cargo test`, frontend typecheck/lint, manual devtools-offline
  reload test.

Test policy (user): delegate test-writing to implementation agents; write only
useful tests (backend filter mapping, settings validation/clamp). No snapshot
or exhaustive UI tests.
