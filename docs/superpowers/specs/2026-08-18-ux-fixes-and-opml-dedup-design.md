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

- The only marker-less *content* list in the app is the Settings install-reasons list (`Settings.tsx`); add `list-disc` + left padding. (Several app-chrome lists — timeline, sidebar trees, sources, search suggestions — are also marker-less, but that is intentional; they use flex/divide layout.)
- Reader content already restores bullets via `.article-prose` in `globals.css`. Audit remaining content lists and add `list-disc`/`list-decimal` utility classes where needed.
- Rule: content lists get explicit list utilities; app-chrome lists (timeline, sidebar trees) remain marker-less by design.

## Item 3 — OPML duplicate handling

**Decision (user):** full two-phase preview + resolution dialog with migration.

### Backend

- Add the `opml` crate as a direct dependency (already a transitive dependency of news-flash).
- New module `src/engine/opml_import.rs`:
  - Parse the OPML string into outlines (categories + feeds) using the `opml` crate. Each feed outline gets a stable 0-based `index` over feed outlines in the file (used as the conflict key; deterministic across parses of the same string).
  - Normalize feed URLs (scheme, host case, default port, trailing slash, fragment).
  - Classify each feed outline against existing feeds (via `Engine::get_feeds`) and against earlier outlines in the same file. The classification unit is the **exact feed-id string** (`feed_id` in news-flash is the raw `xml_url` string). Cases:
    - **Exact duplicate** — normalized URL, title, and category all match an existing feed → auto-skip.
    - **Intra-file** — same normalized URL earlier in the file: if title+category identical → auto-skip; if different → surfaced as a conflict with a single synthetic match (no migration possible).
    - **URL-identical conflict** — the OPML entry's raw xml_url *equals* an existing feed's `feed_url` string, but title or category differs. news-flash would overwrite the same row (`replace_into`), so this is an update, not a duplication.
    - **URL-variant conflict** — normalized URL matches an existing feed (or its website) but the raw xml_url string differs (http/https, www, trailing slash, etc.). This is a true duplicate.
    - **New** — import normally.
- Change `POST /api/sources/import-opml` (in `src/api/sources.rs`) to a two-phase endpoint with body `{ opml, resolutions? }`:
  - **No `resolutions`**: parse + classify. If there are conflicts, return `{ status: "conflicts", conflicts: [...] }` with **no DB writes**. If there are no conflicts, import immediately and return `{ status: "imported", added, skipped, migrated: 0 }`.
  - **With `resolutions`**: build a cleaned OPML (re-serialize via the `opml` crate keeping all categories and only the feed outlines to add), import via `Engine::import_opml`, then apply the resolution side-effects. Return `{ status: "imported", added, skipped, migrated, conflicts_resolved }`.
- **Contract:**

  ```
  OpmlFeed     = { index: number, title: string, url: string, category: string }
  ExistingFeed = { id: string, title: string, url: string | null, category: string }
  Conflict     = { key: number, type: "url-identical" | "url-variant" | "intra-file",
                   opml: OpmlFeed, matches: ExistingFeed[] }
  Resolution   = { key: number, action: "keep-new" | "keep-existing" | "skip",
                   keep_existing_feed_id?: string }
  ```

  - `conflicts` phase-1 response: `{ status: "conflicts", conflicts: Conflict[], stats: { new: number, exact_duplicates: number } }`.
  - `Resolution.keep_existing_feed_id` is required when `action === "keep-existing"` and must be one of the conflict's `matches[].id`.
  - **`keep-new` side effects by conflict type:**
    - `url-variant`: import the entry (new feed id = raw xml_url). For every existing feed in `matches` with a real DB id, run `UPDATE articles SET feed_id = <new> WHERE feed_id = <old>` then `Engine::remove_feed(old)`. This is the migration path.
    - `url-identical`: no new feed exists (same id); importing updates the existing row's label. If the OPML category differs from the existing feed's category, `Engine::move_feed` the existing feed to the new category. No migration, no removal.
    - `intra-file`: the occurrence wins in the cleaned OPML (per normalized URL, the resolved occurrence is emitted, defaulting to the first).
  - **`keep-existing`**: drop the OPML entry; leave all existing feeds untouched. (Edge case: if a conflict has 2+ matches and the user keeps one, the other matched existing feed is left as-is — no destructive cleanup.)
  - **`skip`**: drop the OPML entry, import nothing for it.
- Migration SQL is **net-new** (the only precedent, `src/engine/queries.rs`, is read-only via `open_readonly`). Use a direct read-write SQLite connection to `<data_dir>/engine/data/database.sqlite` with `PRAGMA busy_timeout`, executed inside `Engine::mutation_guard` to avoid racing news-flash's sync.
- Migration is safe for saved rows: feedea's `saved` table keys on `article_id`, not `feed_id`.

### Frontend

- `useImportOpml` inspects the response; on `status: "conflicts"` it opens a new `OpmlConflictDialog`.
- `OpmlConflictDialog`: a single modal listing all conflicts (one modal, not one popup per conflict). Each row shows the OPML entry vs. each existing feed (name, url, category) with a keep-choice (default: existing feed; "keep new" triggers migration). Conflict `type` is shown so the user understands whether "keep new" updates the existing feed (`url-identical`) or replaces and migrates it (`url-variant`). Resubmits with `resolutions`, then shows an import summary.

## Item 4 — Clickable source names

- Clickable feed name (links to `/feeds?feed=<id>`) in:
  - reader meta (`Reader.tsx`),
  - timeline items (`TimelinePanel.tsx`),
  - Saved/Overview article items (`ArticleListItem.tsx`, `Saved.tsx`),
  - new **Open feed** action in the article context menu.
- Timeline row restructure (per user decision: clickable row with a primary article link + a separate feed link):
  - Outer element becomes a `<div>` (not a single `<Link>`), keeping `triggerProps` (the context-menu handlers) on it — they are plain event handlers and work on any element.
  - Row navigation to the article is done via `onClick`, `onKeyDown` (Enter/Space, `role="link"`, `tabIndex={0}`) and `onAuxClick` (middle-click opens in a new tab) on the outer div.
  - The article title is a `<Link>` inside the row; the feed-name `<Link>` sits on the meta line below the title and calls `e.stopPropagation()` so it never triggers row navigation.
  - Visual layout (avatar, title, meta, thumbnail, hover state) is preserved.

## Item 5 — Context menus

- New `ArticleContextMenu.tsx` for timeline/article rows:
  - Open in browser · Copy URL · Copy title · Share
  - divider
  - Mark read / Mark unread · Save / Unsave
  - divider
  - Open feed · Mark all read
- Replace `FeedContextMenu` usage in `TimelinePanel` and `ArticleListItem` with the article menu.
- **Decision (user):** add a right-click source menu to sidebar source rows too, reusing the existing `FeedContextMenu` + Rename/Delete dialogs. Sidebar source rows are `FeedLink` in `Sidebar.tsx`; wire `useContextMenu` + the existing `FeedContextMenu`/`RenameDialog`/`DeleteDialog` there (the sidebar already has the `FeedSummary` objects).

## Item 6 — Active filter indicators

- **Feeds page**: filter chip bar above the timeline showing active filters (feed name, category name, search term, and unread/saved/tag when set). Each chip has a remove `×`; a **Clear all** action appears when more than one filter is active.
- **Sidebar active-state derivation** (replaces NavLink's `isActive`, which is brittle when the path is `/feeds/<id>?feed=X`): read `feed`, `category`, `saved`, `unread`, `tag` from search params on a `/feeds*` route.
  - Feed row active iff `searchParams.feed === id`.
  - Category row active iff `searchParams.category === id` AND no `feed` param (precedence: feed > category).
  - "All" row active iff neither `feed` nor `category` is set (saved/unread/tag/search are orthogonal to the sidebar tree).
  - TreeLink/FeedLink compute this with `useSearchParams` + `useLocation` and render `Link` with the conditional classes (the current `bg-accent-soft text-accent-soft-foreground` highlight plus a stronger accent left-border indicator).

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