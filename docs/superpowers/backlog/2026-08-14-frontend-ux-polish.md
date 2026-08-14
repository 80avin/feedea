# rssea — Frontend UX Polish Backlog

Date: 2026-08-14
Status: Collected from user feedback after Phase 5/6. All 15 items FIXED in Phase 7 (branch `feat/phase7-ux-polish`, commits `0f1f76a..fe1c689`). New audit findings recorded below (Importants fixed; notable minors deferred). This file is the committed record of what was done and what remains.

## User-reported issues (all FIXED in Phase 7)

1. **Search bar width** — FIXED (Task 3, `b00a535`): `w-full` on the wrapper + Hero `fullWidth`; spans its container on desktop and mobile (`SearchBar.tsx`).
2. **FeedAvatar** — FIXED (Task 3, `b00a535`): always a 2-letter colored circle; deterministic hash of `feed_id` → color with a per-source override picker in the Edit-source dialog (`utils/avatarColor.ts`, `FeedAvatar.tsx`).
3. **Sources placement (desktop)** — FIXED (Task 3, `b00a535`): `/sources` is a main-layout page; the `SourcesSlideOver` + `backgroundLocation` plumbing were removed (`App.tsx`, `Sidebar.tsx`).
4. **Sources action buttons overflow (mobile)** — FIXED (Task 4, `8dac3ba`): header/action rows wrap; Export/Import status is a non-layout absolute tooltip that can't widen the container; buttons themed (`Sources.tsx`, `OpmlButtons.tsx`).
5. **Feeds context menu** — FIXED (Task 4, `8dac3ba` + `cb2fd76`): right-click + touch long-press (pointerType-gated, mouse long-press excluded) on timeline and Overview items; same actions as the 3-dot menu; Edit/Delete dialogs wired (`useContextMenu.ts`, `FeedContextMenu.tsx`).
6. **Saved notes in reader** — FIXED (Task 5, `fe1c689`): note card rendered below the article content with a "Note" label, accent border/background, visually distinct from content (`Reader.tsx`).
7. **Sidebar Categories spacing (desktop)** — FIXED (Task 4, `8dac3ba`): Categories shrinks to fit its content; Sources takes the remaining space (`Sidebar.tsx`).
8. **Overview items clickable** — FIXED (Task 3, `b00a535`): `ArticleListItem` root is now a `Link` to `/feeds/{id}`.
9. **Theme broken** — FIXED (Task 2, `547b751`): theme (dark/light/system) genuinely applies app-wide via a `.dark` variant + semantic tokens; accent colors (blue/zinc/emerald/violet) added to Settings.
10. **Infinite scroll on mobile** — FIXED (Task 4, `8dac3ba`): the IntersectionObserver now roots on the timeline's scroll container instead of `root: null` (`useInfiniteScroll.ts` `rootRef`, `Feeds.tsx`).
11. **Reader action buttons theme (desktop)** — FIXED (Task 2, `547b751`): compact themed buttons (28px, `rounded-md`) replace the blue/white pills.
12. **Default mark-read on open** — FIXED (Task 5, `fe1c689`): see "Decision" note below.
13. **Dialogs wrong theme** — FIXED (Task 2, `547b751`): all Hero surfaces (dialogs, inputs, dropdowns, chips, login card) now follow the theme.
14. **Category creation** — FIXED (Tasks 3/4, `b00a535` + `8dac3ba`): "New category" button on the Feeds page (mobile) + sidebar "+" (desktop), both wired to `useAddCategory` (`AddCategoryDialog.tsx`).
15. **Sidebar section buttons (desktop)** — FIXED (Task 3, `b00a535`): icon-only buttons in the Categories/Sources headers (add category, add source, OPML import) (`Sidebar.tsx`).

## New findings from the Phase 7 audit

Found by the Task 1 audit (SDD `task-1-audit.md`); severity per that report.

### Importants (all FIXED in Phase 7)

- **N1. Theme system non-functional; light surfaces leaking everywhere** — FIXED (Task 2, `547b751`): theme context toggles `.dark` + accent on `<html>`; all Hero surfaces and hardcoded `zinc-*` colors converted to semantic tokens.
- **N2. Sources action row clipped on mobile and in the desktop slide-over** — FIXED (Task 4, `8dac3ba`): wrap + non-layout status tooltip.
- **N3. Mobile timeline pagination never fired at the natural bottom** — FIXED (Task 4, `8dac3ba`): observer rooted to the scroll container.
- **N4. FeedAvatar shape flip (square favicon vs round fallback)** — FIXED (Task 3, `b00a535`): avatar is always a colored 2-letter circle; the favicon `<img>` path was removed entirely.

### Notable minors — deferred / future work

- Context-menu keyboard path: the custom `FeedContextMenu` is a `role="menu"` with mouse/touch input only — no arrow-key navigation/focus management yet.
- The "Exported"/"Imported" status tooltip does not auto-dismiss.
- Touch long-press was verified only via headless-Chrome CDP; the iOS Safari long-press gesture is untested on real hardware.
- `rssea.avatar-colors` localStorage entries go stale when a feed is deleted.
- White text on the 2 lightest avatar palette swatches (contrast).
- Dead placeholder repo links in Help/Settings (`github.com/yourname/rssea`).
- PWA manifest `theme_color`/`background_color` hardcoded dark, so window chrome stays dark in light theme (the runtime `theme-color` meta does follow the theme).
- Per-row mounted `RenameDialog`/`DeleteDialog` subtrees on timeline/Overview rows (perf).
- `autoMarkedReadIds` Set never clears (session-scoped, unbounded growth).
- Backend accepts arbitrary `accent`/`theme` strings (unknown values silently fall back to defaults).
- Auto-mark-read is not retried if the request fails.
- One-frame unread→read flip in the reader while the mutation settles.

## Decision — auto-mark-read on open (Task 5)

All Reader opens auto-mark the article read when currently unread (uniform mechanism per user's stated default "default action on opening a feed should be to mark it read"). A session-scoped Set dedupes so an explicitly-unread-then-reopened article isn't re-marked (explicit intent wins). Known consequence: opening a long-saved article from Saved flips it read — accepted for now.

## Process instruction from the user

"There could be more inconsistencies. Best if you ask a subagent to find more such issues and fix them together. Not urgent."

Plan: after Phase 6 (single-binary shipping) is merged, dispatch a subagent to (a) audit the frontend for more inconsistencies of this class (theme/affordance/layout/responsiveness), (b) compile the full list, (c) fix them together, and (d) get review. Track as a follow-on phase/plan.

**Executed as Phase 7** (`0f1f76a..fe1c689`): audit done (Task 1), fixes landed per task above, backlog updated to record status, audit findings, and the mark-read decision (this file).
