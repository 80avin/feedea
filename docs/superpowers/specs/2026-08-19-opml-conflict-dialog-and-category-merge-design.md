# feedea: OPML conflict dialog UX + category merge — design

Date: 2026-08-19
Status: approved

## Overview

Three frontend improvements to the OPML conflict-resolution dialog plus one backend bug fix:

1. **Wide dialog on desktop** — the conflict dialog is too thin at the default HeroUI `md` size.
2. **Old-vs-new comparison layout** — show each conflict as two columns (existing vs imported) with the URL shown once in the header and the differing fields (title, category) highlighted.
3. **Bulk actions** — "Select all new" and "Select all existing" buttons.
4. **Backend: merge same-name sibling categories on import** — when an OPML defines the same category title twice under the same parent, news-flash creates two category rows with different UUIDs. We must merge them into one.

## Item 1 — Wide dialog

- In `frontend/src/components/OpmlConflictDialog.tsx`, give `Modal.Container` a larger size: `size="lg"` maps to `modal__dialog--lg` (`max-w-lg` = 32rem). For a two-column layout this is still narrow; use a custom `max-w-2xl` (42rem) or `max-w-3xl` (48rem) class instead. Decision: `max-w-2xl` via a className on `Modal.Container`/`Modal.Dialog`, which `composeSlotClassName` merges with the slot classes.
- Verify in the HeroUI `Modal.Container` implementation that a `className` prop is composed with the slot class (`composeSlotClassName(slots?.dialog, className)`), so a Tailwind `max-w-2xl` overrides the default `md` width cleanly.

## Item 2 — Old-vs-new comparison layout

Redesign each conflict row in `OpmlConflictDialog.tsx`:

- **Header line**: conflict `kind` label (existing `kindLabel` map) plus the shared URL.
  - The URL is usually identical between old and new (a changed URL is classified as a separate feed, so it only differs for `url-variant`). Show it once in the header.
  - If the URL differs (url-variant conflict), show it per-column with the change highlighted instead.
- **Two-column grid** (`grid grid-cols-2 gap-3`):
  - **Old** column: the existing match(es). For each match show title, category, and URL (URL only when it differs from the OPML entry). A radio for each existing match ("keep existing: <title>"), defaulting to the first non-`__file__` match (existing `defaultResolution` behavior).
  - **New** column: the OPML entry — title, category, URL. A radio "keep new".
  - Highlighting: any field (title, category, url) that differs between the OPML entry and the currently-selected existing match gets a visual "changed" treatment (e.g. amber/`warning` text or a subtle background + a `Changed` chip). Fields that match get neutral styling.
- Keep the existing "Skip" radio.
- Intra-file conflicts: the "old" match is a synthetic `__file__:N` entry (first occurrence in the file); treat it as the Old column. No migration applies (existing behavior).
- **Bulk actions** (Item 3): buttons near the top of the modal body: "Select all new" → set `action: "keep-new"` for every conflict; "Select all existing" → set `action: "keep-existing"` with the first non-`__file__` match's id for every conflict (same choice as the current default). Reusing the existing `choices` state, these just batch-set the same per-conflict resolution.
- No backend contract change: the dialog still submits `{ opml, resolutions }` with the existing `OpmlResolution` shape.

## Item 3 — Bulk actions

Covered in Item 2. Two buttons, no new API fields.

## Item 4 — Backend: merge same-name sibling categories

### Root cause (verified against news-flash 3.2.0 source)

`news_flash::opml::parse_outlines` (`~/.cargo/registry/src/index.crates.io-*/news-flash-3.2.0/src/util/opml/mod.rs:262`) resolves a category by label **only against the DB snapshot** passed in (`existing_categories`), never against categories created during the same import run (`category_vec`). So an OPML that defines the same category title twice under the same parent produces two category rows with different UUIDs. `feedea`'s `Engine::import_opml` passes the (cleaned) OPML straight to `nf.import_opml`, so the duplicates reach the DB.

### Fix

Add a normalization step in `src/engine/opml_import.rs` that runs inside `build_cleaned_opml` (the single funnel for both import paths) **after** `filter_outlines`:

- Walk the outline tree recursively. For each parent, group child outlines that are *categories* (no `xml_url`) by their resolved title (title attribute if non-empty, else text).
- For any group of size > 1, keep the first outline and append the other outlines' `children` into it; drop the duplicates. Recurse into every category's children afterward.
- Feeds (outlines with `xml_url`) are never merged — two sibling feeds with the same title but different URLs are distinct feeds (and same-URL siblings are already handled as intra-file conflicts elsewhere).
- This guarantees news-flash sees at most one category outline per title per parent, so it creates a single category row.

Implementation detail: `Outline` (`opml` crate) has `title: Option<String>` and `text: String`. The resolved title used by news-flash's `parse_outlines` and by our `collect` is `title.filter(non-empty).unwrap_or(text)`. Use the same resolution for grouping.

### Tests

- Unit test in `src/engine/opml_import.rs`:
  - OPML with two sibling category outlines sharing a title (each with distinct feed children) → after `build_cleaned_opml`, re-parsing yields exactly one category outline with all feed children.
  - Nested duplicate (duplicate title at a deeper level) also merges.
  - Same-name categories under different parents are NOT merged (only siblings).
  - Two sibling feeds with the same title but different URLs are not merged.
- Integration test in `tests/sources.rs` (reusing `spawn_app` + `/api/categories` tree helper):
  - Import an OPML with two sibling categories of the same name, each containing a distinct feed URL; assert the category tree contains exactly one category with that label and both feeds.

## Files touched

- `frontend/src/components/OpmlConflictDialog.tsx` (layout, highlighting, bulk actions, width)
- `src/engine/opml_import.rs` (normalization + unit tests)
- `tests/sources.rs` (integration test for category merge)
- Possibly `frontend/src/api/types.ts` only if a helper type is added (not expected)

## Verification

- Rust: `cargo test` (unit + integration).
- Frontend: `bun run typecheck` and `bun run lint` in `frontend/`.
- Manual desktop pass of the dialog (width, two columns, highlighting, bulk actions) and an OPML import that defines a duplicate sibling category.