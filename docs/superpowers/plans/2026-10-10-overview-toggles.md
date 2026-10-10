# Overview sort + hide-read toggles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Overview cards can be sorted alphabetically or by recency, and read articles can be hidden via a server-side unread-only filter.

**Architecture:** Backend adds `?unread_only=1` to `GET /api/overview` by setting `ArticleFilter.unread = Some(Read::Unread)` on per-card item queries (totals unchanged). Frontend adds a subtle toolbar with sort segmented control + hide-read switch, persisted in localStorage, with client-side recent-sort.

**Tech Stack:** Rust axum + news-flash `ArticleFilter`/`Read`; React + TanStack Query.

**Spec:** `docs/superpowers/specs/2026-10-10-reader-overview-offline-design.md` (section 3)

## Global Constraints

- Totals (`total_count`, `unread_count`, `all`) never change under `unread_only`; only card `items` are filtered.
- Sort default `alpha` (current server order); hide-read default off.
- localStorage keys: `feedea:overview-sort` (`alpha`|`recent`), `feedea:overview-hide-read` (`0`|`1`).
- Only useful tests: backend filter-mapping test; frontend verified by typecheck/lint (no test runner configured).

## Review Focus

- `unread_only=1` with all-read category: card returns empty `items` but keeps its counts.
- `unread_only` with invalid value (`?unread_only=yes`): treated as off, no 400.
- Card with no items under recent sort: sorts last, does not crash on empty dates.
- Refresh with toggles set: toggles restore from localStorage.
- Marking an article read then refetching overview with hide-read on: article disappears from card items.

---

### Task 1: Backend `unread_only` param

**Files:**
- Modify: `src/api/overview.rs:1-107`
- Test: `src/api/overview.rs` (new `#[cfg(test)]` unit test for filter helper)

**Interfaces:**
- Consumes: `ArticleFilter`, `Read`, `FeedID` from `news_flash::models`.
- Produces: `fn card_article_filter(feed_ids: Vec<FeedID>, unread_only: bool) -> ArticleFilter` used by the `overview` handler.

- [ ] **Step 1: Write the failing test**

Append to `src/api/overview.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_filter_sets_unread_only_when_requested() {
        let feeds = vec![FeedID::new("f1")];
        let off = card_article_filter(feeds.clone(), false);
        assert_eq!(off.unread, None);
        assert_eq!(off.limit, Some(super::MAX_OVERVIEW_ITEMS));
        let on = card_article_filter(feeds, true);
        assert_eq!(on.unread, Some(Read::Unread));
    }
}
```

And add the helper plus `Read` import it needs (this is the red step — test fails to compile until Step 3).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test card_filter_sets_unread_only_when_requested`
Expected: FAIL (compile error: `card_article_filter` not found).

- [ ] **Step 3: Write minimal implementation**

In `src/api/overview.rs`:

1. Change the import line to include `Read`:
```rust
use news_flash::models::{ArticleFilter, ArticleOrder, FeedID, OrderBy, Read};
```

2. Add above the `overview` handler:
```rust
pub fn card_article_filter(feed_ids: Vec<FeedID>, unread_only: bool) -> ArticleFilter {
    ArticleFilter {
        order: Some(ArticleOrder::NewestFirst),
        order_by: Some(OrderBy::Published),
        limit: Some(MAX_OVERVIEW_ITEMS),
        feeds: Some(feed_ids),
        unread: unread_only.then_some(Read::Unread),
        ..ArticleFilter::default()
    }
}
```

3. Add a query struct and use it in the handler:
```rust
#[derive(serde::Deserialize, Default)]
pub struct OverviewParams {
    #[serde(default)]
    pub unread_only: Option<String>,
}

fn is_truthy(v: Option<&String>) -> bool {
    matches!(v.map(String::as_str), Some("1") | Some("true"))
}
```

Change the handler signature to:
```rust
pub async fn overview(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<OverviewParams>,
) -> ApiResult<Json<Value>> {
```

and inside the per-card `tasks.spawn` closure replace the inline `ArticleFilter { ... }` with:
```rust
let unread_only = is_truthy(params.unread_only.as_ref());
let filter = card_article_filter(feed_ids, unread_only);
```

Note: `params` must be read before the loop (`let unread_only = is_truthy(...)` once outside) and moved/copied (`bool` is `Copy`) into each spawned task.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test card_filter_sets_unread_only_when_requested`
Expected: PASS. Then run `cargo test` (full suite) — all green.

- [ ] **Step 5: Commit**

```bash
git add src/api/overview.rs
git commit -m "feat(overview): support unread_only filter for card items"
```

### Task 2: Frontend toolbar + sort + hide-read

**Files:**
- Modify: `frontend/src/state/hooks.ts:38-40` (`useOverview`)
- Modify: `frontend/src/state/keys.ts:4` (query key)
- Modify: `frontend/src/pages/Overview.tsx:87-127` (toolbar + sorting + placeholder)

**Interfaces:**
- Consumes: `GET /api/overview?unread_only=1` from Task 1; `CategoryCard`, `OverviewResponse` types (unchanged).
- Produces: toolbar UI persisting to the localStorage keys above.

- [ ] **Step 1: Update query key and hook**

In `frontend/src/state/keys.ts`, change:
```ts
overview: ["overview"] as const,
```
to:
```ts
overview: ["overview"] as const,
overviewWith: (unreadOnly: boolean) => ["overview", { unreadOnly }] as const,
```

In `frontend/src/state/hooks.ts`, change `useOverview` to:
```ts
export function useOverview(unreadOnly = false) {
  const query = unreadOnly ? "?unread_only=1" : "";
  return useQuery({
    queryKey: queryKeys.overviewWith(unreadOnly),
    queryFn: () => api.get<OverviewResponse>(`/api/overview${query}`),
  });
}
```

Keep the old `queryKeys.overview` entry so existing invalidations (`["overview"]` prefix matches `["overview", ...]`) still invalidate both variants.

- [ ] **Step 2: Add toolbar, persistence, sorting**

In `frontend/src/pages/Overview.tsx`:

1. Add helpers at top (after imports):
```tsx
type SortMode = "alpha" | "recent";
const SORT_KEY = "feedea:overview-sort";
const HIDE_KEY = "feedea:overview-hide-read";
function loadSort(): SortMode {
  return window.localStorage.getItem(SORT_KEY) === "recent" ? "recent" : "alpha";
}
function loadHide(): boolean {
  return window.localStorage.getItem(HIDE_KEY) === "1";
}
function cardRecency(c: { items: { date: string }[] }): number {
  return c.items.reduce((m, i) => Math.max(m, Date.parse(i.date) || 0), 0);
}
```

2. In `Overview()`, add state:
```tsx
const [sort, setSort] = useState<SortMode>(loadSort);
const [hideRead, setHideRead] = useState(loadHide);
const { data, isLoading, isError, error, refetch } = useOverview(hideRead);
const cards = useMemo(() => {
  const list = data?.cards ?? [];
  if (sort === "recent") {
    return [...list].sort((a, b) => cardRecency(b) - cardRecency(a) || a.name.localeCompare(b.name));
  }
  return list;
}, [data, sort]);
```

Persist on change:
```tsx
const changeSort = (m: SortMode) => { setSort(m); window.localStorage.setItem(SORT_KEY, m); };
const toggleHide = () => { setHideRead((v) => { window.localStorage.setItem(HIDE_KEY, v ? "0" : "1"); return !v; }); };
```

3. Render a subtle toolbar row between the header and the grid:
```tsx
<div className="mt-3 flex items-center gap-2 text-xs">
  <div className="flex overflow-hidden rounded-md border border-app-border" role="tablist" aria-label="Sort categories">
    {(["alpha", "recent"] as const).map((m) => (
      <button key={m} type="button" role="tab" aria-selected={sort === m}
        onClick={() => changeSort(m)}
        className={sort === m ? "bg-app-surface-2 px-2.5 py-1 font-semibold text-app-text" : "px-2.5 py-1 text-app-text-muted hover:text-app-text"}>
        {m === "alpha" ? "A–Z" : "Recent"}
      </button>
    ))}
  </div>
  <label className="ml-auto flex cursor-pointer items-center gap-1.5 text-app-text-muted">
    <input type="checkbox" checked={hideRead} onChange={toggleHide} className="accent-[var(--accent)]" />
    Hide read
  </label>
</div>
```

4. Render `cards` instead of `data.cards`, and in `Card`'s list add the caught-up placeholder: after `{items.length === 0 && ...}` keep existing "No articles yet." only when `totalCount === 0`; when `items.length === 0 && totalCount > 0` (hide-read filtered everything) show `<li className="py-4 text-sm text-app-text-faint">All caught up.</li>`. Pass `totalCount` into that branch (already a prop).

- [ ] **Step 3: Verify**

Run (workdir `frontend/`): `bun run typecheck` then `bun run lint`
Expected: both PASS.

Manual: toggle Recent → cards reorder by newest item; toggle Hide read → URL becomes `/api/overview?unread_only=1`, read items vanish, all-read cards show "All caught up"; reload → toggles persist.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/state/hooks.ts frontend/src/state/keys.ts frontend/src/pages/Overview.tsx
git commit -m "feat(overview): sort toggle and hide-read filter"
```
