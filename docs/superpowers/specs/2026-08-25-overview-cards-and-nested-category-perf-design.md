# Overview Cards (Direct Feeds Only) & Nested-Category Performance — Design

Date: 2026-08-25
Status: Approved (sections 1–2 approved by user in session)
Scope: Workstream A. A follow-up workstream ("anonymous read-only access") is
described under Future Work and gets its own spec.

## Problem statements

1. **Overview shows container categories.** `GET /api/overview`
   (`src/api/overview.rs:21-29`) emits one card per category returned by
   news-flash, including parent categories that exist only to hold
   sub-categories. Requirement: cards only for categories with feeds **directly
   mapped** to them (`feed_mapping.category_id`), plus `NewsFlash.Toplevel`
   when any feed maps to it (user decision: keep the Uncategorized card).

2. **Nested-category orchestration is slow and has a correctness bug.**
   Verified audit findings (all confirmed against feedea and news-flash 3.2.0
   source):

   | ID | Finding | Evidence |
   |----|---------|----------|
   | B1 | `/api/overview` issues ~6 sequential SQLite round-trips **per card** (per-card `get_fat_articles` re-reads both mapping tables each call) plus redundant refetches; ≈190 statements at 30 categories | news-flash `database/mod.rs:1048-1056`; `src/engine/mod.rs:508-515`; `src/api/overview.rs:21-48` |
   | B2 | Marking a parent category read ignores nested children: upstream `set_category_read` matches direct `feed_mapping` rows only, never recurses through `category_mapping` | news-flash `database/mod.rs:745-759`; `src/engine/mod.rs:439-445` |
   | B3 | Redundant fetches: `get_category_tree` double-fetches its inputs via `category_unread_map`; `PATCH /api/categories/{id}` builds the full tree up to 3× | `src/engine/mod.rs:457-459`, `314-352`; `src/api/categories.rs:58,66,81` |
   | B5 | Blocking diesel/rusqlite calls escape `spawn_blocking`: `delete_empty_categories` sync reads, raw-SQL reads in async handlers, `move_feed`, `migrate_feed_articles` | `src/engine/mod.rs:392-393,209-218`; `src/engine/queries.rs`; `src/engine/opml_import.rs:357` |
   | B6 | Serial per-article `unsave_article` DELETEs while holding the app_db mutex on category/feed deletion | `src/api/categories.rs:94-97`; `src/api/sources.rs:132-135` |
   | B4 | Raw rusqlite bypass of news-flash's private schema (`move_feed`, `migrate_feed_articles`) — no public-API alternative exists upstream; accepted as documented debt | `src/engine/mod.rs:211-218`; `src/engine/opml_import.rs:359-362` |

   Non-findings (audited, OK as-is): tree construction is flat-fetch +
   in-memory recursion, cycle-guarded; `ArticleFilter{categories}` descendant
   expansion inside news-flash is correct; no caching layer exists today
   (deliberately not added here — see Alternatives).

## Goals

- Overview cards = categories with ≥1 directly-mapped feed (+ Toplevel card).
- `/api/overview` statement count independent of category count (~11 total).
- Parent mark-read covers all descendants within news-flash's public contract.
- No blocking DB calls on tokio workers outside `spawn_blocking`.
- Remove triplicated category-hierarchy logic behind one shared type.
- Measured before/after latency for `/api/overview`.

## Non-goals

- No cache/memoization layer.
- No changes to news-flash (upstream).
- No frontend changes required (server-driven payloads keep their shapes).
- Not touching anonymous access (Future Work below).

## Design

### 1. Card selection rule

Cards = exactly the set of ids appearing as `category_id` in `feed_mapping`
(the "direct-feed set"), plus `NEWSFLASH_TOPLEVEL` if any feed maps directly
to it. Categories acting only as containers are excluded. The existing
predicate template in `delete_empty_categories`
(`src/engine/mod.rs:395-402`) moves into the shared index (below).

### 2. Strict-direct card semantics

Every card — including Toplevel — shows only its own direct feeds' data:

- **items**: per-card parallel fetch of the card's own newest articles via the
  card's resolved feeds (see 3).
- **total_count / unread_count**: per-feed article totals
  (`queries::per_feed_totals_map`) summed over the card's *resolved* feeds —
  the same feed→card assignment used for items, so counts and items can never
  diverge (a feed with a stale duplicate mapping counts in exactly one card).

Rationale: today the Toplevel card effectively duplicates "All articles"
(`ArticleFilter{categories:[toplevel]}` recursively expands into every
top-level category; a rolled-up toplevel count equals the global total).
Rolled-up counts remain used where they belong: the sidebar / category tree
(`CategoryNode.unread_count`).

NOTE (2026-08-26): the original "single batched query with a global window"
was rejected after end-to-end verification on a real database (it starved
categories whose articles fell outside the global newest-N window). Per-card
parallel fetch replaced it. See the plan's Task 4 for the final approach.

### 3. Batched headline fetch

One `get_fat_articles` call with
`categories: Some(all_card_ids_including_toplevel)`,
`order: NewestFirst`, `order_by: Published`,
`limit: clamp(6 × n_cards, 24, 120)`.

Results are bucketed in memory:

- feed→direct-category resolution reuses the deterministic preference logic
  from `Engine::get_feeds` (`src/engine/mod.rs:259-283`: prefer real category
  over stale Toplevel row), factored into the index.
- articles append to their card's bucket in global recency order; first 6 per
  bucket kept.
- Headline shaping (FatArticle→Headline incl. feed titles) refactored so it
  can consume an index-provided title map instead of re-fetching feeds.

Accepted trade-off (user-approved): a low-volume card may show <6 or 0
previews when its newest articles fall outside the global window; counts stay
correct.

### 4. Mark-parent-read fix

`Engine::mark_category_read` expands the requested id via
`queries::descendant_category_ids` (existing, cycle-safe) and passes all ids
in a single `set_category_read(&[ids])` call — one statement, public API only.

### 5. Bulk unsave (B6)

New `AppDb::unsave_articles(&[ids])` executing one
`DELETE FROM saved_articles WHERE article_id IN (…)`. Call sites
(`categories::remove`, `sources::remove`) switch from the serial loop to the
bulk call.

### 6. CategoryIndex (new `src/engine/category_index.rs`)

Built once per request from one consolidated round:
`get_categories()` + `get_feeds()` + `unread_count_feed_map()` (≈5
statements). Contents:

- `children_by_parent`, name map, full-path map (reuses `category_paths`)
- direct-feed-category set (card rule + empty-check reuse)
- deterministic feed→direct-category map
- feed-title map
- per-category unread: **direct** map and **rolled-up** map (descendant sum)

Consumers refactored onto it:

| Consumer | Change |
|---|---|
| `/api/overview` | 1 index + 1 raw-SQL totals read + 1 batched articles call |
| `/api/categories` (tree) | index reused; no second fetch via `category_unread_map` |
| `PATCH /api/categories/{id}` | guard + cycle-check + response share one build |
| `/api/sources` list | paths/categories from index |
| OPML conflict resolution loop | category state updated incrementally in memory; no per-resolution re-fetch |

Expected overview cost: ~11 statements regardless of N (vs ~190 at N=30).

### 7. Blocking-call hygiene (B5)

All sync diesel/rusqlite work runs inside `spawn_blocking` (existing
`with_nf` pattern): `delete_empty_categories`' direct engine reads,
raw-SQL reads (`category_totals`, `total_article_count`, search), `move_feed`,
`migrate_feed_articles`.

### 8. Bypass debt documentation (B4)

Doc-comments on `move_feed` and `migrate_feed_articles` stating they
deliberately pin news-flash's private schema (no public-API equivalent
upstream), so future upstream migrations surface as diagnosable breakage.

## Testing & measurement

- Invert `tests/overview.rs::overview_parent_card_includes_descendant_feed_counts`:
  parent-without-direct-feeds produces **no** card; child card intact.
- Update Toplevel-card test to strict-direct semantics.
- Keep `overview_shows_full_category_path`.
- New test: marking a parent read clears descendant articles (covers B2,
  currently uncovered).
- Existing suites (`cargo test`, frontend typecheck/lint via `make test`) stay green.
- Perf harness: integration test seeding ~25 categories / ~40 feeds / ~1.5k
  articles (reusing existing test seeding helpers), timing median-of-K
  `GET /api/overview`. Run on `main` vs branch; numbers recorded in the PR.

## Risks / mitigations

- Batched window may starve low-volume cards of previews → accepted,
  documented in API behavior; counts unaffected.
- Refactor touches several handlers → covered by existing integration suites
  plus new tests; behavior shapes unchanged.
- Strict-direct Toplevel semantics change what the card displays → intended;
  test updated accordingly.

## Alternatives considered

- **Surgical patches without shared index**: smaller diff, keeps triplicated
  hierarchy logic and redundant fetches. Rejected.
- **Cache/memoized tree in AppState**: fastest steady-state but adds
  invalidation complexity unjustified at this scale. Rejected (YAGNI).
- **Parallelizing per-card queries**: wall-clock gain only, still O(N)
  statements; superseded by batching. Rejected.
- **Raw-SQL top-6-per-category via window functions**: exact previews in one
  statement but deepens schema coupling into article reads (headline shaping
  duplication). Rejected.

## Future work (separate spec)

**Anonymous read-only access** (user decisions recorded here): opt-in server
flag (`--public-read` / `FEEDEA_PUBLIC_READ`, default off); unauthenticated
users get the full read surface (overview, category tree, feeds list, article
list/detail, search, favicons/thumbnails); all mutations remain auth-only.
Implementation sketch from auth audit: split protected router / optional-auth
extractor, `/api/session` flag exposure, frontend gating, expired-session
cleanup while in the area. To be designed after Workstream A lands.
