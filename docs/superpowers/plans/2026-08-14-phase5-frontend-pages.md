# Phase 5: Frontend Pages — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the frontend pages per spec §7: the Reader view (rendered HTML with proxied images + action buttons + note/tag editing), Overview page (category cards), Sources page/popover (add-source with Fetch-title, OPML import, per-source 3-dot menu), Saved page (note/tag editing), and Settings polish. This makes the PWA feature-complete against the real Phase 3/4 base.

**Architecture:** Extends the Phase 4 shell. The Reader is the centerpiece: a route `/feeds/:articleId` (already wired as the single reader mechanism) rendering rendered HTML from the backend (which already rewrites URLs + proxies images), with a metadata header and an actions row (Read/Unread, Save/Unsave, Open, Share) — top on desktop, bottom on mobile. Saving opens an inline note+tags editor. Sources becomes a desktop popover panel (toggleable from the sidebar) and a mobile full page. Saved gets note/tag editing. Overview renders cards from `useOverview`.

**Tech Stack:** (unchanged from Phase 4) React 19.2.8, HeroUI v3.2.4, @tanstack/react-query 5.101.4, react-router 8.3.0, @heroicons/react 2.2.0, Tailwind v4, bun, vite-plugin-pwa.

## Global Constraints

- No comments in code unless a task explicitly shows them.
- All data via the `frontend/src/api` client + `frontend/src/state` hooks — no raw fetch in components. Ids are URLs: use the existing `encodeId` helper when building paths, and note React Router/URLSearchParams decode params (so raw ids arrive decoded).
- Reader HTML comes from the backend already rewritten (`/api/articles/:id` → `html` with proxied `/img?u=` images + absolute links). Render it safely: use `dangerouslySetInnerHTML` ONLY on that backend-rewritten html (it's trusted app content), or sanitize with a tiny allowlist if you prefer extra safety — pick one and be consistent; do NOT re-sanitize in a way that breaks `/img?u=` or data-original hover.
- Action buttons: desktop top-of-reader, mobile bottom-of-reader. Save opens note+tags editing. Open = `window.open(originalUrl, "_blank")`. Share = `navigator.share` if available, else copy-link fallback.
- The `?article=` legacy fallback still exists in `useSelectedArticleId`; new code navigates via `/feeds/:articleId` paths only.
- Commit after each task with the exact message given.

---

### Task 1: Reader view — metadata header, rendered HTML, actions (Read/Save/Open/Share), note+tags editing

**Files:**
- Create: `frontend/src/components/Reader.tsx` (or `frontend/src/pages/Reader.tsx` replacing the placeholder `ReaderPanel.tsx`), `frontend/src/components/ReaderActions.tsx`, `frontend/src/components/ArticleHtml.tsx`, `frontend/src/components/SaveDialog.tsx`
- Modify: `frontend/src/components/ReaderPanel.tsx` (route it through the real Reader), `frontend/src/state/hooks.ts` (add `useArticleMeta` if needed, `useUpdateNoteTags` mutation)
- Test: browser verification via headless CDP

**Interfaces:**
- Consumes: `useArticle(id)` (exists), `useSaveArticle`, `useUnsaveArticle`, `useMarkRead`, backend `/api/articles/:id`.
- Produces:
  - `Reader.tsx`: given the selected article id, renders: title, metadata header (source name, date, age, original-link), then `ArticleHtml` (the rendered content). If `html` absent, falls back to `summary` (plain text) or `plain_text`. Loading skeleton + error state via `Feedback`.
  - `ReaderActions.tsx`: Read/Unread toggle (uses `useArticle` marked/unread state), Save/Unsave (opens `SaveDialog` on save when not yet saved, direct unsave otherwise), Open (window.open original URL), Share (navigator.share or clipboard). Position: top of article on desktop (in the metadata header row), fixed/sticky bottom action bar on mobile (Tailwind responsive).
  - `ArticleHtml.tsx`: `dangerouslySetInnerHTML` of the backend-rewritten `html`, inside a styled prose container (Tailwind typography-ish manual styles). Links render with `target="_blank"` via a click handler or a `<Base>` override — decide: add a click handler that opens absolute http(s) links in new tabs and shows target URL on hover (spec: "show target URL on hover"). Simplest compliant approach: render as-is (backend already made links absolute); add `title`/hover via a delegated click handler opening external links in new tabs. Images use `/img?u=` (already rewritten). Handle load-error gracefully (hide broken imgs via onError like TimelinePanel does).
  - `SaveDialog.tsx`: a HeroUI Modal with a note textarea + tags input (comma-separated, with existing tag suggestions from `useTags`) + Save/Cancel. On save: `POST /api/articles/:id/save` (create) or `PUT` (edit) with note+tags, invalidate relevant keys.
  - hooks.ts: `useUpdateNoteTags(id)` mutation (PUT /save), maybe `useArticleUnread(id)` toggle. Reuse existing save/unsave/mark-read.

- [ ] **Step 1: Write the failing browser test** (a short CDP script or manual checklist) that will drive the implementation: open an article → title + metadata render; content HTML renders with proxied images; Read toggle flips; Save opens dialog; saving with a note persists; Open/Share present. (Follow the Phase 4 headless-CDp pattern.)
- [ ] **Step 2: Implement `Reader.tsx`, `ReaderActions.tsx`, `ArticleHtml.tsx`, `SaveDialog.tsx`** + hooks. Match the existing component conventions (HeroUI v3 API verified against node_modules types).
- [ ] **Step 3: Wire ReaderPanel → Reader** (the `/feeds/:articleId` route already exists; ensure the desktop right panel and mobile full page both render the real Reader).
- [ ] **Step 4: Verify**: typecheck/lint/build; browser CDP: open article (desktop 3-panel + mobile full page), content renders, Read/Save/Open/Share work, note+tags persist and appear in Saved (Task 4 will confirm grouping).
- [ ] **Step 5: Commit**

```bash
git add frontend/src && git commit -m "Phase 5: add reader view with actions and note/tag editing"
```

---

### Task 2: Overview page — category cards

**Files:**
- Modify: `frontend/src/pages/Overview.tsx` (real), `frontend/src/components/ArticleListItem.tsx` (shared item already used)
- Test: browser CDP

**Interfaces:**
- Consumes: `useOverview()` (exists).
- Produces: Overview renders `{ cards, all }`: a responsive grid of category cards. Each card: header with category name + total count; body: up to 10 items (compact `ArticleListItem` with title, age, source, thumbnail); footer "More" button → navigate to `/feeds?category=<encodeId(id)>` (Feeds page already reads `category` param). An "All" card aggregates `all.total_count`/`all.unread_count` with the latest items across everything — if `all` has no items array in the API shape, show the counts and a "View all" → `/feeds` (check the backend `OverviewResponse` shape in types.ts; if `all` lacks items, that's fine — show counts only). Mobile already gets the search bar via the Feeds page; the Overview page itself needs no search bar on mobile per spec (spec says mobile overview has search bar at top — verify the backend can serve it; if not feasible, note it and defer the mobile search bar to the Feeds page which already has one).

- [ ] **Step 1: Implement Overview.tsx** against `useOverview` (check the actual `OverviewResponse` type — `cards: CategoryCard[]`, `all: { total_count, unread_count }`). Handle empty state ("No feeds yet — add sources").
- [ ] **Step 2: Verify** typecheck/lint/build + browser CDP: cards render with counts + items; "More" navigates to filtered Feeds; empty state shows.
- [ ] **Step 3: Commit**

```bash
git add frontend/src && git commit -m "Phase 5: implement overview page with category cards"
```

---

### Task 3: Sources page/popover — add source (Fetch-title), OPML import/export, per-source menu

**Files:**
- Create: `frontend/src/pages/Sources.tsx`, `frontend/src/components/AddSourceDialog.tsx`, `frontend/src/components/SourceMenu.tsx`, `frontend/src/components/OpmlButtons.tsx`
- Modify: `frontend/src/components/Sidebar.tsx` (a "Manage sources" toggle that opens the popover on desktop), `frontend/src/App.tsx` (mobile `/sources` route exists), `frontend/src/state/hooks.ts` (add `useDiscover`, `useAddSource` wiring if not complete, `useImportOpml`, `useExportOpml`, `useDeleteFeed`, `useRenameFeed`, `useFeedRead` — most exist)
- Test: browser CDP

**Interfaces:**
- Consumes: `useSources` (grouped by category — check the actual shape), `useCategories`, mutations.
- Produces:
  - `AddSourceDialog.tsx`: HeroUI Modal with: URL input; "Fetch title" button → `POST /api/sources/discover` → fills title (+ shows discovered feed_url); category select (from `useCategories`); Add button → `POST /api/sources` → invalidate sources/categories/feeds.
  - `OpmlButtons.tsx`: Import (file input or textarea → `POST /api/sources/import-opml`), Export (download `GET /api/sources/export-opml` text/xml via a Blob link). 
  - `SourceMenu.tsx`: a 3-dot HeroUI dropdown per source row: Open (website), Edit (rename via PATCH), Refresh (`POST /:id/refresh`), Mark all read (`POST /:id/read`), Delete (`DELETE /:id` with confirm). 
  - `Sources.tsx`: top: Add button + OPML buttons; list of sources grouped by category (from the grouped shape); each row: favicon avatar, title, unread count, error indicator (if error_message), SourceMenu. Handles empty state.
  - Desktop: the Sources content renders in a popover/drawer panel toggled from the Sidebar ("Sources" is already a sidebar item — make it toggle a slide-over panel on desktop, and route to the full page on mobile). Decide the cleanest mechanism (a state in Sidebar + a fixed panel overlay, or a route `/sources` that DesktopLayout renders as an overlay). Match the existing dual-mount approach: a route `/sources` rendered as a slide-over on desktop and a page on mobile is cleanest and keeps URL state.

- [ ] **Step 1: Implement the components + hooks** (check the exact `GET /api/sources` grouped shape in types.ts and the backend sources.rs before coding).
- [ ] **Step 2: Verify** typecheck/lint/build + browser CDP: add a source (Fetch-title fills it), import/export OPML, per-source menu actions (rename/refresh/read/delete) work; desktop popover + mobile page both function.
- [ ] **Step 3: Commit**

```bash
git add frontend/src && git commit -m "Phase 5: add sources page with add/import/export and per-source menu"
```

---

### Task 4: Saved page — month grouping + note/tag editing

**Files:**
- Modify: `frontend/src/pages/Saved.tsx` (real), `frontend/src/components/SaveDialog.tsx` (reuse for editing), `frontend/src/state/hooks.ts` (add `useUpdateNoteTags`)
- Test: browser CDP

**Interfaces:**
- Consumes: `useSaved` (exists, month-grouped), `useArticle`, `useUpdateNoteTags`, `useUnsaveArticle`.
- Produces: Saved renders `{ months }` (month label + items, newest first). Each item shows title, source, age, save-time, note snippet, tags (HeroUI Chip). Clicking opens the reader. A per-item edit affordance opens `SaveDialog` pre-filled with the article's note/tags (PUT on save) to edit. Unsave removes it from the list (with the reader's unsave too). Empty state ("Nothing saved yet").

- [ ] **Step 1: Implement Saved.tsx** (check the exact `SavedResponse` shape — months, items with note + tags).
- [ ] **Step 2: Verify** typecheck/lint/build + browser CDP: save an article with note+tags (via Reader), see it in Saved under the right month; edit the note/tags from Saved; unsave removes it.
- [ ] **Step 3: Commit**

```bash
git add frontend/src && git commit -m "Phase 5: implement saved page with month grouping and note/tag editing"
```

---

### Task 5: Settings page + sidebar source/category polish

**Files:**
- Modify: `frontend/src/pages/Settings.tsx` (polish), `frontend/src/components/Sidebar.tsx` (unread badges on categories/feeds; "Manage sources" toggle), `frontend/src/pages/Help.tsx`
- Test: browser CDP

**Interfaces:**
- Consumes: `useSettings`/`useUpdateSettings` (exist), `useCategories`/`useSources` (exist), `useChangePassword`.
- Produces:
  - Settings polish: theme (light/dark/system), sync interval, keep-articles-days, change password (current+new+confirm), stats display (feeds/articles/unread/db-size/last-sync from the settings GET), and the existing logout. Add "About" links: repo URL + issues URL (placeholders — the repo is yet to be published; use a README note). 
  - Sidebar: unread count badges next to categories and feeds (from `useCategories` unread_count + `useSources`/feeds). "Manage sources" opens the Sources popover (desktop) / navigates to /sources (mobile). Help page content: keyboard-ish hints + links.
  - Also fix a parked minor: `formatAge` fallback for invalid dates (return "—" instead of "Invalid Date").

- [ ] **Step 1: Implement Settings polish + Sidebar badges + Help + formatAge fix**.
- [ ] **Step 2: Verify** typecheck/lint/build + browser CDP: settings round-trip, password change, stats render, sidebar badges show unread counts, Manage sources opens the popover.
- [ ] **Step 3: Commit**

```bash
git add frontend/src && git commit -m "Phase 5: polish settings, add sidebar unread badges, and sources toggle"
```

---

## Self-Review Notes

- Spec coverage: Phase 5 completes spec §7 pages — Reader (§7 Reader), Overview, Sources (desktop popover/mobile page), Saved (notes/tags/month grouping), Settings. The only spec item deliberately deferred: mobile overview search bar (the Feeds page already provides search; the backend overview has no search param). Noted in Task 2.
- Placeholder scan: ReaderPanel placeholder is replaced in Task 1; Sources/Help placeholders replaced in Tasks 3/5. No TBDs.
- Type consistency: all hooks/keys/types already exist from Phase 4; this phase adds `useUpdateNoteTags` + any discover/opml wiring. Reader uses the existing `useArticle(id)` and the shared `useSelectedArticleId`. Ids: `encodeId` everywhere in paths (Phase 4 critical fix already in place).
- Design decisions (not defects): `dangerouslySetInnerHTML` on backend-rewritten html (trusted app content, images already proxied) with external-link handling for hover/new-tab; single reader mechanism `/feeds/:articleId` (Phase 4 final fix); Sources as a slide-over route on desktop + page on mobile.
- Phase 4 deferred minors this phase touches: formatAge fallback (Task 5); sidebar unread badges (Task 5); the rest (apiGetText 401, redundant listeners, manifest duplication, h-dvh) remain parked for Phase 6.
