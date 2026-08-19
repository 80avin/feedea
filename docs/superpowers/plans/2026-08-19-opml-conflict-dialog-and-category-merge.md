# OPML Conflict Dialog UX + Sibling Category Merge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the OPML conflict dialog UX (wider modal, old-vs-new comparison columns with change highlighting, bulk select-all actions) and stop duplicate sibling categories from being created on OPML import.

**Architecture:** Two independent workstreams. Backend: add a category-normalization step to `build_cleaned_opml` in `src/engine/opml_import.rs` that merges same-title sibling category outlines before news-flash imports. Frontend: redesign `OpmlConflictDialog.tsx` layout with a two-column old/new grid, diff highlighting, and bulk action buttons. Both are testable independently.

**Tech Stack:** Rust (axum, sqlite via news-flash, `opml` crate), React 19 + TypeScript + Tailwind v4 + HeroUI 3.2.

## Global Constraints

- OPML category "title" resolution is `title` attribute if non-empty else `text` attribute (matches `collect` in `opml_import.rs:53-55` and news-flash `parse_outlines` at `mod.rs:210-218`).
- Only merge **sibling** category outlines under the same parent; never merge categories under different parents; never merge feed outlines (those with `xml_url`).
- Frontend: `bun run typecheck` and `bun run lint` in `frontend/` must pass.
- Rust: `cargo test` must pass.
- No comments in code unless asked (repo convention).
- Commit after each task with a message in the repo's style (`feat:`, `fix:`, `docs:`, `chore:`, `test:`).

---

### Task 1: Backend — merge duplicate sibling categories in `build_cleaned_opml`

**Files:**
- Modify: `src/engine/opml_import.rs` (add helper functions; call them inside `build_cleaned_opml` before the final `.to_string()`)
- Test: `src/engine/opml_import.rs` (unit tests in existing `mod tests`)

**Interfaces:**
- Consumes: `opml::Outline` struct (`text: String`, `title: Option<String>`, `xml_url: Option<String>`, `outlines: Vec<Outline>`), existing `filter_outlines` + `build_cleaned_opml` (`src/engine/opml_import.rs:232-318`).
- Produces: private helper `fn merge_sibling_categories(outlines: &mut Vec<Outline>)` and `fn outline_title(o: &Outline) -> String` (used internally; callers outside this task only observe `build_cleaned_opml`'s unchanged signature `(String, usize)`).

- [ ] **Step 1: Add `outline_title` helper**

Add near the top of `src/engine/opml_import.rs` (after `normalize_url`, before `classify`):

```rust
fn outline_title(o: &Outline) -> String {
    o.title
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| o.text.clone())
}
```

- [ ] **Step 2: Add `merge_sibling_categories` helper**

```rust
fn merge_sibling_categories(outlines: &mut Vec<Outline>) {
    struct Holder {
        is_category: bool,
        title: String,
        outline: Outline,
    }
    let mut groups: Vec<Holder> = Vec::with_capacity(outlines.len());
    for mut outline in std::mem::take(outlines) {
        let is_category = outline.xml_url.is_none();
        let title = outline_title(&outline);
        if is_category {
            if let Some(existing) = groups
                .iter_mut()
                .find(|g| g.is_category && g.title == title)
            {
                existing.outline.outlines.append(&mut outline.outlines);
                continue;
            }
        }
        groups.push(Holder {
            is_category,
            title,
            outline,
        });
    }
    // Recurse AFTER all appends so a merged parent's children are
    // normalized as one level (e.g. "Sub" siblings that each came from a
    // different duplicate "Top" also merge).
    *outlines = groups
        .into_iter()
        .map(|mut holder| {
            merge_sibling_categories(&mut holder.outline.outlines);
            holder.outline
        })
        .collect();
}
```

Notes:
- `outline_title` resolves title the same way news-flash and `collect` do (`title` attr if non-empty else `text`).
- `is_category` guards the group key: a category and a feed sharing an empty resolved title never collide (feed outlines carry `is_category: false`).
- The recursion runs after appends so cross-duplicate children merge too (e.g. two "Sub" siblings arriving from two merged "Top" outlines collapse into one "Sub").

- [ ] **Step 3: Call the helper in `build_cleaned_opml`**

In `build_cleaned_opml`, after `filter_outlines(&mut doc.body.outlines, &mut index, &keep);` (line 313) and before `let cleaned = doc.to_string()...` (line 314), insert:

```rust
merge_sibling_categories(&mut doc.body.outlines);
```

- [ ] **Step 4: Write failing unit tests**

Append to the `mod tests` block in `src/engine/opml_import.rs`:

```rust
#[test]
fn build_cleaned_opml_merges_sibling_categories_with_same_title() {
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/a"/>
    </outline>
    <outline text="Tech" title="Tech">
      <outline text="Feed B" title="Feed B" type="rss" xmlUrl="https://example.com/b"/>
    </outline>
  </body>
</opml>"#;
    let entries = parse_entries(opml).unwrap();
    let classification = classify(&entries, &[]);
    let resolutions = vec![];
    let (cleaned, added) =
        build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
    assert_eq!(added, 2);
    let cleaned_entries = parse_entries(&cleaned).unwrap();
    let categories: Vec<&str> = cleaned_entries.iter().map(|e| e.category.as_str()).collect();
    assert_eq!(categories, vec!["Tech", "Tech"]);
    let doc = opml::OPML::from_str(&cleaned).unwrap();
    let cats: Vec<&str> = doc
        .body
        .outlines
        .iter()
        .map(|o| outline_title(o).as_str())
        .collect();
    assert_eq!(cats, vec!["Tech"], "two sibling 'Tech' outlines must merge into one");
    assert_eq!(doc.body.outlines[0].outlines.len(), 2, "both feeds kept under the merged category");
}

#[test]
fn build_cleaned_opml_merges_nested_sibling_duplicates_but_not_across_parents() {
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="A" title="A" type="rss" xmlUrl="https://example.com/a"/>
      </outline>
      <outline text="Sub" title="Sub">
        <outline text="B" title="B" type="rss" xmlUrl="https://example.com/b"/>
      </outline>
    </outline>
    <outline text="Other" title="Other">
      <outline text="Sub" title="Sub">
        <outline text="C" title="C" type="rss" xmlUrl="https://example.com/c"/>
      </outline>
    </outline>
  </body>
</opml>"#;
    let entries = parse_entries(opml).unwrap();
    let classification = classify(&entries, &[]);
    let (cleaned, added) =
        build_cleaned_opml(opml, &entries, &classification, &[]).unwrap();
    assert_eq!(added, 3);
    let doc = opml::OPML::from_str(&cleaned).unwrap();
    assert_eq!(doc.body.outlines.len(), 2, "sibling 'Top'/'Other' outlines stay separate");
    let top = &doc.body.outlines[0];
    let other = &doc.body.outlines[1];
    assert_eq!(outline_title(top), "Top");
    assert_eq!(outline_title(other), "Other");
    assert_eq!(top.outlines.len(), 1, "nested sibling 'Sub' outlines merge into one");
    assert_eq!(outline_title(&top.outlines[0]), "Sub");
    assert_eq!(top.outlines[0].outlines.len(), 2, "A and B kept under the merged 'Sub'");
    assert_eq!(other.outlines.len(), 1, "'Sub' under 'Other' is a distinct category");
    assert_eq!(other.outlines[0].outlines.len(), 1, "C kept under 'Other'/'Sub'");
}

#[test]
fn build_cleaned_opml_does_not_merge_feeds_with_same_title() {
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/x"/>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/y"/>
  </body>
</opml>"#;
    let entries = parse_entries(opml).unwrap();
    let classification = classify(&entries, &[]);
    let (cleaned, added) =
        build_cleaned_opml(opml, &entries, &classification, &[]).unwrap();
    assert_eq!(added, 2);
    let cleaned_entries = parse_entries(&cleaned).unwrap();
    assert_eq!(cleaned_entries.len(), 2, "same-title feeds with different URLs stay distinct");
}
```

- [ ] **Step 5: Run tests to verify they fail first (before Step 3 is in place)**

Run: `cargo test --lib engine::opml_import::tests`
Expected: `build_cleaned_opml_merges_sibling_categories_with_same_title` FAILS (assertion `cats == vec!["Tech"]` fails; two outlines present). Other two tests fail for the same reason.

- [ ] **Step 6: Implement Steps 1–3, re-run tests**

Run: `cargo test --lib engine::opml_import::tests`
Expected: all three new tests PASS. Existing tests in the module still PASS (regression check).

- [ ] **Step 7: Commit**

```bash
git add src/engine/opml_import.rs
git commit -m "fix(backend): merge duplicate sibling categories on opml import"
```

---

### Task 2: Backend — integration test for category merge

**Files:**
- Modify: `tests/sources.rs` (add one integration test near the existing OPML import tests)
- Test: the new test itself

**Interfaces:**
- Consumes: `spawn_app()`, `login_cookie(&app)`, `cookie_pair`, `get_groups` (all in `tests/sources.rs:40-117`). `/api/sources/import-opml` endpoint. `/api/categories` returns `{ categories: [...] }` tree.
- Produces: no new exports.

- [ ] **Step 1: Write the failing integration test**

Append to `tests/sources.rs` (import `serde_json::Value` is already used; no new imports needed for this test):

```rust
#[tokio::test]
async fn opml_import_merges_duplicate_sibling_categories() {
    let (_feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example-a.invalid/feed.xml"/>
    </outline>
    <outline text="Tech" title="Tech">
      <outline text="Feed B" title="Feed B" type="rss" xmlUrl="https://example-b.invalid/feed.xml"/>
    </outline>
  </body>
</opml>"#;
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "imported");
    assert_eq!(val["added"], 2);

    let tree_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/categories")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tree_resp.status(), StatusCode::OK);
    let tree: serde_json::Value =
        serde_json::from_slice(&tree_resp.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let tech: Vec<&serde_json::Value> = tree["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["name"] == "Tech")
        .collect();
    assert_eq!(tech.len(), 1, "exactly one 'Tech' category must exist");

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 2, "both feeds imported");
}
```

- [ ] **Step 2: Run the test to verify it fails on the current code**

Run: `cargo test --test sources opml_import_merges_duplicate_sibling_categories`
Expected: FAIL — `tech.len()` is 2 (two "Tech" categories with different IDs) because Task 1 is not yet merged into this branch. If Task 1 is already in place, this test should PASS; in that case skip the "verify fail" and continue.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: all pass, including the new test and all existing OPML/category tests.

- [ ] **Step 4: Commit**

```bash
git add tests/sources.rs
git commit -m "test(backend): assert sibling category merge on opml import"
```

---

### Task 3: Frontend — widen the conflict dialog

**Files:**
- Modify: `frontend/src/components/OpmlConflictDialog.tsx`

**Interfaces:**
- Consumes: `Modal.Container` accepts a `className` prop (verified: `composeSlotClassName(slots?.dialog, className)` merges it with the slot class in `@heroui/react/dist/components/modal/modal.js:161`).
- Produces: no exports; only the dialog's presentation changes.

- [ ] **Step 1: Add a wider max-width class**

In `OpmlConflictDialog.tsx`, change `<Modal.Container>` to:

```tsx
<Modal.Container className="max-w-2xl">
```

The default slot class `modal__dialog--md` applies `max-w-md`; Tailwind v4 compiles `max-w-2xl` (42rem) into the dialog's class list. Because the dialog already has `flex w-full flex-col`, the wider max-width simply lets the two-column grid breathe on desktop while remaining full-width on small screens (container is `w-full` under `sm:`).

- [ ] **Step 2: Verify typecheck + lint**

Run (in `frontend/`): `bun run typecheck && bun run lint`
Expected: both pass (className on Modal.Container is a valid prop).

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/OpmlConflictDialog.tsx
git commit -m "fix(frontend): widen opml conflict dialog on desktop"
```

---

### Task 4: Frontend — old-vs-new comparison layout with highlighting

**Files:**
- Modify: `frontend/src/components/OpmlConflictDialog.tsx` (the `conflicts.map(...)` body and helper functions)

**Interfaces:**
- Consumes: `OpmlConflict`, `OpmlExistingFeed`, `OpmlResolution`, `OpmlEntry` types from `frontend/src/api/types.ts:151-177`. Existing `choices`/`initialChoices`/`choiceFor` state and `submit()`.
- Produces: helper `function fieldChanged(a: string, b: string): boolean` and `function EntryField({ label, value, changed }: { label: string; value: string; changed: boolean })` used only within this file.

- [ ] **Step 1: Add diff-highlight helpers**

After `defaultResolution` (line 30) add:

```tsx
function fieldChanged(a: string, b: string): boolean {
  const na = (a ?? "").trim().toLowerCase();
  const nb = (b ?? "").trim().toLowerCase();
  return na !== nb && na !== "" && nb !== "";
}

function changedClass(changed: boolean): string {
  return changed
    ? "rounded bg-amber-500/10 px-1 text-amber-700 dark:text-amber-400"
    : "";
}
```

- [ ] **Step 2: Compute the selected old match per conflict**

Update the type import on line 4 to include `OpmlExistingFeed`:

```tsx
import type { ImportOpmlResponse, OpmlConflict, OpmlExistingFeed, OpmlResolution } from "../api/types";
```

Then, inside the component after `choiceFor`, add a helper to find the currently selected existing match:

```tsx
const selectedMatch = (conflict: OpmlConflict): OpmlExistingFeed | undefined => {
  const choice = choiceFor(conflict);
  if (choice.action !== "keep-existing") return undefined;
  return conflict.matches.find((m) => m.id === choice.keep_existing_feed_id);
};
```

- [ ] **Step 3: Replace the conflict card body**

Replace the `<div key={conflict.key} className="rounded-lg border ...">` block (lines 74–128) with:

```tsx
<div key={conflict.key} className="rounded-lg border border-app-border p-3">
  <div className="flex items-center justify-between gap-2">
    <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">{kindLabel[conflict.kind]}</p>
    <span className="flex shrink-0 items-center gap-1">
      <BulkChoiceButton
        active={choice.action === "keep-new"}
        onClick={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))}
      >
        Keep new
      </BulkChoiceButton>
      {conflict.matches.map((match) => (
        <BulkChoiceButton
          key={match.id}
          active={choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id}
          onClick={() =>
            setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
          }
        >
          {match.id.startsWith("__file__:") ? "Keep first" : "Keep existing"}
        </BulkChoiceButton>
      ))}
      <BulkChoiceButton
        active={choice.action === "skip"}
        onClick={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "skip" } }))}
      >
        Skip
      </BulkChoiceButton>
    </span>
  </div>

  <p className="mt-2 truncate text-xs text-app-text-muted">{conflict.opml.url}</p>

  <div className="mt-3 grid grid-cols-2 gap-3">
    <div className="min-w-0 rounded-md border border-app-border bg-app-surface/60 p-3">
      <p className="text-xs font-semibold uppercase tracking-wider text-app-text-faint">Old</p>
      {conflict.matches
        .filter((m) => !m.id.startsWith("__file__:") || conflict.matches.every((x) => x.id.startsWith("__file__:")))
        .map((match) => {
          const isSelected = choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id;
          return (
            <label key={match.id} className={`mt-2 flex cursor-pointer items-start gap-2 rounded p-1 text-sm ${isSelected ? "bg-app-selected/60" : ""}`}>
              <input
                type="radio"
                name={`conflict-${conflict.key}`}
                checked={isSelected}
                onChange={() =>
                  setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
                }
                className="mt-0.5"
              />
              <span className="min-w-0 flex-1">
                <span className={`block ${changedClass(fieldChanged(match.title, conflict.opml.title))}`}>{match.title}</span>
                <span className={`block text-xs ${changedClass(fieldChanged(match.category, conflict.opml.category))}`}>{match.category || "Uncategorized"}</span>
                {fieldChanged(match.url ?? "", conflict.opml.url) && (
                  <span className="block truncate text-xs text-app-text-faint">{match.url}</span>
                )}
              </span>
            </label>
          );
        })}
    </div>
    <div className="min-w-0 rounded-md border border-app-border bg-app-surface/60 p-3">
      <p className="text-xs font-semibold uppercase tracking-wider text-app-text-faint">New</p>
      <label className="mt-2 flex cursor-pointer items-start gap-2 rounded p-1 text-sm">
        <input
          type="radio"
          name={`conflict-${conflict.key}`}
          checked={choice.action === "keep-new"}
          onChange={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))}
          className="mt-0.5"
        />
        <span className="min-w-0 flex-1">
          <span className={`block ${changedClass(fieldChanged(selectedMatch(conflict)?.title ?? "", conflict.opml.title))}`}>{conflict.opml.title}</span>
          <span className={`block text-xs ${changedClass(fieldChanged(selectedMatch(conflict)?.category ?? "", conflict.opml.category))}`}>{conflict.opml.category || "Uncategorized"}</span>
          {fieldChanged(selectedMatch(conflict)?.url ?? "", conflict.opml.url) && (
            <span className="block truncate text-xs text-app-text-faint">{conflict.opml.url}</span>
          )}
        </span>
      </label>
    </div>
  </div>
</div>
```

- [ ] **Step 4: Add the `BulkChoiceButton` component**

Add at the bottom of the file (or just above `export default`). Update the import on line 1 to also bring in `ReactNode`:

```tsx
import { useMemo, useState } from "react";
import type { ReactNode } from "react";
```

Then:

```tsx
function BulkChoiceButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-2 py-0.5 text-xs font-medium transition-colors ${
        active
          ? "border-accent bg-accent-soft text-accent-soft-foreground"
          : "border-app-border text-app-text-muted hover:bg-app-hover/60 hover:text-app-text"
      }`}
    >
      {children}
    </button>
  );
}
```

Note: Task 5 adds the header-level bulk actions; the per-conflict `BulkChoiceButton` pills here give an always-visible choice for each conflict and reuse the same component.

- [ ] **Step 5: Typecheck + lint**

Run (in `frontend/`): `bun run typecheck && bun run lint`
Expected: pass. Fix any unused-import warnings (e.g. ensure all imported types are used).

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/OpmlConflictDialog.tsx
git commit -m "feat(frontend): old-vs-new columns with change highlighting in conflict dialog"
```

---

### Task 5: Frontend — bulk "select all new / select all existing" actions

**Files:**
- Modify: `frontend/src/components/OpmlConflictDialog.tsx` (add header buttons + handlers)

**Interfaces:**
- Consumes: `conflicts`, `setChoices`, `defaultResolution` (from earlier tasks / existing code).
- Produces: no new exports.

- [ ] **Step 1: Add bulk handlers**

Inside the component, after `choiceFor` add:

```tsx
const selectAllNew = () => {
  setChoices(Object.fromEntries(conflicts.map((c) => [c.key, { key: c.key, action: "keep-new" }])));
};

const selectAllExisting = () => {
  setChoices(Object.fromEntries(conflicts.map((c) => [c.key, defaultResolution(c)])));
};
```

- [ ] **Step 2: Render the bulk action bar**

Immediately after the intro `<p>` (line 68) and before the conflict list `<div className="mt-4 ...">` (line 70), insert:

```tsx
<div className="mt-3 flex flex-wrap items-center gap-2">
  <Button size="sm" variant="secondary" onPress={selectAllNew} isDisabled={importOpml.isPending}>
    Select all new
  </Button>
  <Button size="sm" variant="secondary" onPress={selectAllExisting} isDisabled={importOpml.isPending}>
    Select all existing
  </Button>
</div>
```

`Button` is already imported from `@heroui/react` (line 2).

- [ ] **Step 3: Typecheck + lint**

Run (in `frontend/`): `bun run typecheck && bun run lint`
Expected: pass.

- [ ] **Step 4: Manual sanity check (if a dev server is available)**

Run `bun run dev` in `frontend/` and confirm: two bulk buttons appear above the conflict list; clicking "Select all new" flips every conflict to the New column selection; "Select all existing" restores each to its default existing match. If no server is available, note this as a manual follow-up.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/OpmlConflictDialog.tsx
git commit -m "feat(frontend): bulk select-all-new and select-all-existing in conflict dialog"
```

---

## Self-Review Notes

- **Spec coverage:** Item 1 (wide dialog) → Task 3. Item 2 (old-vs-new layout, URL in header, highlight diffs) → Task 4. Item 3 (bulk actions) → Task 5. Item 4 (merge sibling categories) → Tasks 1 + 2. All spec requirements covered.
- **Type consistency:** `defaultResolution` (existing, line 24) returns `OpmlResolution`; reused in Task 5. `fieldChanged`/`changedClass`/`BulkChoiceButton`/`selectedMatch`/`EntryField` are consistently named within Task 4. `outline_title`/`merge_sibling_categories` used consistently in Task 1.
- **Placeholder scan:** no TBD/TODO; every code step contains full code.
- **Note on `EntryField`:** the spec mentioned a possible `EntryField` helper; the plan inlines the field markup instead (smaller diff, no unused abstraction). This is a deliberate simplification consistent with the spec's intent.
- **Caveat:** `tests/sources.rs` integration test uses `.invalid` TLDs so news-flash's `parse_all_feeds=true` download attempt fails fast and falls back to OPML data (same pattern the app uses for its own imported entries).