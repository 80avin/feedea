# rssea — Frontend UX Polish Backlog

Date: 2026-08-14
Status: Collected from user feedback after Phase 5/6. To be triaged + fixed together by a subagent audit pass (post-Phase 6 shipping).

## User-reported issues

1. **Search bar width** — doesn't span full width of its container. Should be full width, or centered if capped by a max-width.
2. **FeedAvatar** — should show 2 letters and have color; the color should be configurable per source.
3. **Sources placement (desktop)** — the sources list need not be a popup/right slide-over panel on desktop; it can be part of the main layout.
4. **Sources action buttons overflow (mobile)** — "Import", "Export", "Add source" take too much horizontal space and overflow the screen width; their style doesn't fit the dark theme; clicking "Export" shows "Exported" text and widens the container further.
5. **Feeds context menu** — right-click menu on feeds; long-press on touch screens only (NOT mouse long-press) to open it.
6. **Saved notes in reader** — when opening a saved item, the saved note should appear BELOW the content in the reader and be visually distinct (not look like article content).
7. **Sidebar Categories spacing (desktop)** — with only one category, the Categories section takes too much empty space; it should fit content and let the Sources section start earlier.
8. **Overview items clickable** — overview card items should open the reader on click (like the timeline/saved lists).
9. **Theme broken** — theme selection in Settings appears not to work (light theme still shows dark/system). Low priority but should support theme + accent colors.
10. **Infinite scroll on mobile** — the mobile timeline doesn't do infinite scroll at the list bottom (works on desktop).
11. **Reader action buttons theme (desktop)** — Read/Save/Open/Share don't fit the dark theme (blue/white rounded buttons); also too big.
12. **Default mark-read on open** — opening a feed item should mark it read by default; currently requires manual "Read" click.
13. **Dialogs wrong theme** — Save dialog, login, Add source, 3-dot sources menu, etc. are white-themed even when the app is dark.
14. **Category creation** — no way to create categories in the UI.
15. **Sidebar section buttons (desktop)** — Categories & Sources sections should have icon-only buttons (import, add source, add category, etc.) in desktop mode.

## Process instruction from the user

"There could be more inconsistencies. Best if you ask a subagent to find more such issues and fix them together. Not urgent."

Plan: after Phase 6 (single-binary shipping) is merged, dispatch a subagent to (a) audit the frontend for more inconsistencies of this class (theme/affordance/layout/responsiveness), (b) compile the full list, (c) fix them together, and (d) get review. Track as a follow-on phase/plan.
