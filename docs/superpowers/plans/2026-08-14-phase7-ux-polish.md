# Phase 7: Frontend UX Polish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the user-reported frontend UX inconsistencies (backlog `docs/superpowers/backlog/2026-08-14-frontend-ux-polish.md`) AND audit the frontend for more issues of the same class, then fix them together. Not urgent; correctness of the existing features must be preserved.

**Architecture:** A single subagent-driven audit pass first (a fresh-eyes frontend review walking each page/component on both breakpoints), producing a consolidated issue list. Then fixes are implemented together (one branch, one review loop), grouped into tasks below. The audit may add items beyond the backlog.

**Tech Stack:** (unchanged) React 19.2.8 + HeroUI v3 + Tailwind v4 + React Query + bun + Vite PWA.

## Global Constraints

- No comments in code unless a task explicitly shows them.
- Preserve all existing functionality — the Phase 6 smoke test (`scripts/smoke.sh`) and the full test suites must stay green.
- All fixes go through the existing `frontend/src/api` client + `frontend/src/state` hooks. Ids are URLs → `encodeId`.
- Theme/accent support must respect the existing Settings theme field and the app's dark-first design; light/system must actually work.
- Browser verification (headless Chrome CDP) for responsive + interaction fixes; the audit subagent must walk both desktop (≥lg) and mobile (<lg) viewports.
- Commit per task with the exact message given.

---

### Task 1: Frontend UX audit (fresh-eyes review across pages + breakpoints)

**Deliverable:** `docs/superpowers/backlog/2026-08-14-frontend-ux-polish.md` updated with: (a) confirmation/status of each of the 15 user-reported items, (b) any NEW issues found of the same class (theme, affordance, layout, responsiveness, accessibility), each with file:line and a suggested fix.

**Process:** A subagent (fresh context, no assumptions) runs the app (dev or the smoke-built release) and inspects the code, walking:
- Login, Overview (desktop + mobile), Feeds/timeline (search bar, infinite scroll both breakpoints), Reader (desktop 3-panel + mobile full page, actions, note display), Sources (desktop slide-over + mobile page, action buttons overflow, dialogs), Saved (month grouping, note display), Settings (theme behavior, accent, stats), dialogs (login, save, add-source, 3-dot menu, category), Sidebar (categories/sources spacing, icon buttons), FeedAvatar, Overview items clickability.
- For each backlog item: still-reproducible? fixed-already? partially? Add file:line evidence.
- For new findings: severity (Critical/Important/Minor), file:line, suggested fix.

---

### Task 2: Theme + accent support that actually works

**Backlog items:** #9 (theme broken — light shows dark/system), #13 (dialogs white-themed), #11 (reader action buttons don't fit dark theme + too big).

**Scope:**
- Make the Settings theme (light/dark/system) genuinely apply app-wide (a theme provider/toggle on the root element; HeroUI v3 + Tailwind v4 dark-mode via a class on `<html>` or the `dark` variant — verify the correct v4 mechanism and HeroUI v3 support).
- Add accent color support (a small set of accent colors; the user mentioned "theme and accent colours too"). Store in Settings; apply via CSS variables.
- Ensure ALL surfaces inherit the theme: login, dialogs (Save, Add-source, Rename, 3-dot menus), reader action bar, bottom nav, etc. Audit-verified (Task 1 output) list of white-themed surfaces must all follow the theme.
- Reader action buttons: restyle to fit the theme (outline/ghost on dark, not blue/white rounded blocks) and reduce their size.

---

### Task 3: Search bar + FeedAvatar + Overview + Sources layout

**Backlog items:** #1 (search bar full width / centered), #2 (FeedAvatar 2 letters + per-source color), #8 (overview items clickable), #3 (sources in main layout, not a popup/slide-over on desktop), #15 (sidebar icon buttons for add-source/add-category/import).

**Scope:**
- Search bar spans its container (or centered if capped).
- FeedAvatar shows 2 letters with a color derived deterministically per feed id (configurable per source in the future — for now a stable hash→color; note the "configurable per source" as a follow-up or add it if cheap via the sources edit dialog).
- Overview card items open the reader on click (like timeline/saved).
- Sources: decide with the audit whether the desktop slide-over should become part of the main layout (the user suggested it can be in the main layout). If moved, the sidebar "Sources" item navigates to a `/sources` main-layout view on desktop (not a slide-over), mobile unchanged. Verify the background-location pattern is removed if no longer needed.
- Sidebar Categories/Sources sections: add icon-only action buttons (add source, add category, import) per the user's request.

---

### Task 4: Mobile responsiveness + interaction fixes

**Backlog items:** #4 (sources action buttons overflow mobile + theme + "Exported" text widening), #5 (right-click menu + touch long-press on feeds), #7 (sidebar Categories empty space when one category), #10 (mobile timeline doesn't infinite-scroll), #14 (no category creation UI).

**Scope:**
- Sources action buttons: wrap/stack on mobile, themed, and the "Exported" state must not widen the container.
- Feeds context menu: right-click (mouse) + long-press (touch ONLY — not mouse long-press) opening the same per-feed menu (reuse SourceMenu actions). Long-press-on-touch requires pointer-events detection (pointerType) — implement carefully so mouse long-press doesn't trigger it.
- Sidebar Categories: when few categories, fit content and let Sources start early (flexbox `shrink`/`basis` fix or a max-height → auto when small).
- Mobile infinite scroll: diagnose why the IntersectionObserver sentinel doesn't fire on mobile (likely the observer `root: null` vs an inner scroll container — fix by observing the correct scroll container or using a sentinel inside the scrollable element).
- Category creation UI: add a "New category" affordance (sidebar button and/or categories page/section) using the existing `useAddCategory` mutation.

---

### Task 5: Reader — mark-read-on-open + notes below content

**Backlog items:** #6 (saved notes shown below content in reader, visually distinct), #12 (default mark-read on open).

**Scope:**
- Opening a feed item marks it read by default (fire `useMarkRead(id, true)` on reader mount when currently unread; don't auto-mark when opened via Saved/search if that's undesirable — decide and document; the user said "default action on opening a feed should be to mark it read").
- Saved notes: when an opened article has a saved note, render it BELOW the article content in the reader, visually distinct (a bordered/card container with a "Note" label + different background), not as article content. Verify it doesn't get confused with the content.

---

## Self-Review Notes

- This phase is entirely frontend UX polish; the backend is frozen unless a Task-1-audit item requires a tiny API addition (then flag it explicitly for approval).
- Every task ends with browser CDP verification on BOTH breakpoints.
- The audit (Task 1) may surface new items; new Important/Critical items get folded into the relevant task's scope, new Minor items are recorded in the backlog (deferred) unless trivially fixed in passing.
- `scripts/smoke.sh` + full test suites must remain green at every commit.
