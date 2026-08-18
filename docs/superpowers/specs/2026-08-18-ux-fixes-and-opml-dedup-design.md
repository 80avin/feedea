# feedea: UX fixes and OPML dedup — design

Date: 2026-08-18
Status: approved

## Overview

Seven targeted improvements to feedea (a Rust + React feed aggregator):

1. Restore list markers and other Tailwind-preflight-removed styles for content.
2. Preserve active feed-list filters when an article is opened.
3. Deduplicate feeds during OPML import, with interactive conflict resolution.
4. Make source names clickable in the reader and article lists.
5. Replace the article-row right-click source menu with a proper article menu; add a source menu to sidebar rows.
6. Show active filter indicators in the feed list and better sidebar highlighting.
7. Improve PWA install/update UX.

## Item 2 — Preserve filters when opening an article

**Decision (user):** keep the `/feeds/<id>` path segment and preserve the query string in article links.

- New frontend helper to build article links: `/feeds/<id>` plus the current search params, e.g. `/feeds/<id>?feed=X&category=Y&q=foo`.
- Applied in `TimelinePanel.tsx`, `ArticleListItem.tsx`, `Saved.tsx`; in `SearchBar` suggestions only when already on a `/feeds*` route.
- `useSelectedArticleId` is unchanged (path segment with `?article=` fallback).
- Sidebar active-state (Item 6) is computed from search params so the filter stays highlighted even when the path is `/feeds/<id>`.

## Item 1 — List markers and preflight removals

- The only bare `<ul>` in the app is the Settings install-reasons list (`Settings.tsx`); add `list-disc` + left padding.
- Reader content already restores bullets via `.article-prose` in `globals.css`. Audit remaining content lists and add `list-disc`/`list-decimal` utility classes where needed.
- Rule: content lists get explicit list utilities; app-chrome lists (timeline, sidebar trees) remain marker-less by design.

## Item 3 — OPML duplicate handling

**Decision (user):** full two-phase preview + resolution dialog with migration.

### Backend

- Add the `opml` crate as a direct dependency (already a transitive dependency of news-flash).
- New module `src/engine/opml_import.rs`:
  - Parse the OPML string into outlines (categories + feeds).
  - Normalize feed URLs (scheme, host case, default port, trailing slash, fragment).
  - Classify each feed outline against existing feeds (via `Engine::get_feeds`) and against earlier outlines in the same file:
    - **Exact duplicate**: normalized URL + title + category all match → auto-skip.
    - **Conflict**: URL matches an existing feed (or repeats earlier in the file) but title/category differ → surfaced for resolution.
    - **New**: import normally.
- Change `POST /api/sources/import-opml` (in `src/api/sources.rs`) to a two-phase endpoint with body `{ opml, resolutions? }`:
  - **Phase 1** (`{ opml }`, no `resolutions`): parse + classify only; return `{ status: "conflicts", conflicts: [...] }`. No DB writes. `conflicts` items describe the OPML entry and each matching existing feed (title, url, category).
  - **Phase 2** (`{ opml, resolutions }`): build a cleaned OPML (all categories; only new feeds plus conflict feeds resolved to "keep new"), import via `Engine::import_opml`, then apply migrations: `UPDATE articles SET feed_id = <new> WHERE feed_id = <old>` (direct SQLite write mirroring `src/engine/queries.rs` but read-write, executed under the `Engine::mutation_guard`), then `Engine::remove_feed(old)`. Return summary `{ added, skipped, migrated }`.
- Exact-duplicate and conflict-URL detection prevents the duplicates news-flash itself permits (its `insert_feeds` uses `replace_into` keyed on `feed_id` = raw xml_url string, so http/https or trailing-slash variants become duplicates).
- Migration is safe for saved rows: feedea's `saved` table keys on `article_id`, not `feed_id`.

### Frontend

- `useImportOpml` inspects the response; on `status: "conflicts"` it opens a new `OpmlConflictDialog`.
- `OpmlConflictDialog`: a single modal listing all conflicts (one modal, not one popup per conflict). Each row shows the OPML entry vs. each existing feed (name, url, category) with a keep-choice (default: existing feed; "keep new" triggers migration). Resubmits with `resolutions`, then shows an import summary.

## Item 4 — Clickable source names

- Clickable feed name (links to `/feeds?feed=<id>`) in:
  - reader meta (`Reader.tsx`),
  - timeline items (`TimelinePanel.tsx`),
  - Saved/Overview article items (`ArticleListItem.tsx`, `Saved.tsx`),
  - new **Open feed** action in the article context menu.
- Timeline rows are restructured so the feed link is not nested inside the row's article link (nested anchors are invalid HTML). **Decision (user):** convert the row from a single `<Link>` to a clickable row with the article title as the primary link plus a separate feed link, preserving visual layout, keyboard behavior, and right-click behavior.

## Item 5 — Context menus

- New `ArticleContextMenu.tsx` for timeline/article rows:
  - Open in browser · Copy URL · Copy title · Share
  - divider
  - Mark read / Mark unread · Save / Unsave
  - divider
  - Open feed · Mark all read
- Replace `FeedContextMenu` usage in `TimelinePanel` and `ArticleListItem` with the article menu.
- **Decision (user):** add a right-click source menu to sidebar source rows too, reusing the existing `FeedContextMenu` + Rename/Delete dialogs.

## Item 6 — Active filter indicators

- **Feeds page**: filter chip bar above the timeline showing active filters (feed name, category name, search term, and unread/saved/tag when set). Each chip has a remove `×`; a **Clear all** action appears when more than one filter is active.
- **Sidebar**: active-state derived from search params (stays highlighted while reading an article), with a stronger active style (accent left-bar + bolder background).

## Item 7 — PWA install and update

- `vite.config.ts`: change `registerType` to `"prompt"`.
- New `usePwa` hook (module-level singleton wrapping `registerSW`):
  - captures `beforeinstallprompt` (existing pattern in `Settings.tsx`),
  - detects installed/standalone via `display-mode: standalone` plus iOS `navigator.standalone`,
  - exposes `needRefresh` / `offlineReady` from `onNeedRefresh` / `onOfflineReady`.
- Settings install section becomes three states: **Installed** (disabled, checkmark) → **Install** (enabled) → **Not installable** (reasons list, now with bullets).
- New global **"Update available — Reload"** banner in `Shell` that calls `updateSW()`.

## Testing / verification

- Rust: extend `tests/` for OPML dedup classification and migration using the existing `FeedServer` test helper against a temp data dir.
- Frontend: `bun run typecheck` and `bun run lint`; manual desktop + mobile pass for layouts and menus.
- PWA: verify install button states and update banner in a browser (dev and built).