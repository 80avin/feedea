# Offline cache (settings + SW + warmer) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** With offline toggles enabled, overview + article bodies + N most recent articles open fully with no network.

**Architecture:** Workbox `runtimeCaching` (NetworkFirst for API JSON, StaleWhileRevalidate for images) plus a frontend warmer that fetches overview item details + recent-N details to populate the caches. Settings stored in the existing `settings` kv table. Logout clears runtime API caches.

**Tech Stack:** `vite-plugin-pwa` Workbox, TanStack Query, Rust axum + rusqlite `AppDb`.

**Spec:** `docs/superpowers/specs/2026-10-10-reader-overview-offline-design.md` (section 4)

## Global Constraints

- Defaults: `offline_cache_overview=true`, `offline_cache_recent=false`, `offline_recent_count=50`, clamp 1–200 on write.
- Warmer never blocks rendering; silent `allSettled` failures; cap total detail fetches at 250.
- `PATCH /api/settings` rejects `offline_recent_count=0` or negative with 400.
- Only useful tests: settings default/clamp roundtrip test; warmer verified by typecheck + manual offline test.

## Review Focus

- Offline reload of `/` with cache enabled: overview list renders without network.
- Opening a warmed article offline: full body renders (not just headline).
- `offline_recent_count=500` saved: stored as 200 (clamped), UI shows 200.
- Recent toggle off: N input disabled, no `/api/articles?limit=` prefetch fires.
- Logout on shared device: previously cached `/api/*` entries are gone.

---

### Task 1: Backend offline settings

**Files:**
- Modify: `src/app_db/mod.rs:64-84` (accessors next to theme/sync-interval)
- Modify: `src/dto.rs:87-94` (`Settings` struct)
- Modify: `src/api/settings.rs:12-77` (request struct + get/update)
- Modify: `frontend/src/api/types.ts:65-71` (`Settings` interface)
- Test: `src/app_db/mod.rs` tests module (new roundtrip test)

**Interfaces:**
- Consumes: `AppDb::get_setting/set_setting`.
- Produces: `offline_cache_overview()`, `set_offline_cache_overview(bool)`, `offline_cache_recent()`, `set_offline_cache_recent(bool)`, `offline_recent_count()`, `set_offline_recent_count(i64)`; `Settings{offline_cache_overview: bool, offline_cache_recent: bool, offline_recent_count: i64}` JSON.

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` in `src/app_db/mod.rs`:

```rust
#[test]
fn offline_cache_settings_roundtrip_with_defaults() {
    let dir = tmp_dir();
    let mut db = open(&dir).unwrap();
    assert_eq!(db.offline_cache_overview().unwrap(), true);
    assert_eq!(db.offline_cache_recent().unwrap(), false);
    assert_eq!(db.offline_recent_count().unwrap(), 50);
    db.set_offline_cache_overview(false).unwrap();
    db.set_offline_cache_recent(true).unwrap();
    db.set_offline_recent_count(200).unwrap();
    assert_eq!(db.offline_cache_overview().unwrap(), false);
    assert_eq!(db.offline_cache_recent().unwrap(), true);
    assert_eq!(db.offline_recent_count().unwrap(), 200);
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test offline_cache_settings_roundtrip_with_defaults`
Expected: FAIL (compile error: methods not found).

- [ ] **Step 3: Write minimal implementation**

1. In `src/app_db/mod.rs` after `set_sync_interval_minutes`, add:
```rust
pub fn offline_cache_overview(&self) -> anyhow::Result<bool> {
    Ok(self
        .get_setting("offline_cache_overview")?
        .as_deref()
        .map(|s| s != "0" && s.to_lowercase() != "false")
        .unwrap_or(true))
}
pub fn set_offline_cache_overview(&mut self, v: bool) -> anyhow::Result<()> {
    self.set_setting("offline_cache_overview", if v { "1" } else { "0" })
}
pub fn offline_cache_recent(&self) -> anyhow::Result<bool> {
    Ok(self
        .get_setting("offline_cache_recent")?
        .as_deref()
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(false))
}
pub fn set_offline_cache_recent(&mut self, v: bool) -> anyhow::Result<()> {
    self.set_setting("offline_cache_recent", if v { "1" } else { "0" })
}
pub fn offline_recent_count(&self) -> anyhow::Result<i64> {
    Ok(self
        .get_setting("offline_recent_count")?
        .and_then(|s| s.parse().ok())
        .map(|n: i64| n.clamp(1, 200))
        .unwrap_or(50))
}
pub fn set_offline_recent_count(&mut self, n: i64) -> anyhow::Result<()> {
    self.set_setting("offline_recent_count", &n.clamp(1, 200).to_string())
}
```

2. In `src/dto.rs`, extend `Settings`:
```rust
pub struct Settings {
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub sync_interval_minutes: i64,
    pub keep_articles_days: Option<i64>,
    pub offline_cache_overview: bool,
    pub offline_cache_recent: bool,
    pub offline_recent_count: i64,
    pub stats: SettingsStats,
}
```

3. In `src/api/settings.rs`, extend `UpdateSettings`:
```rust
pub offline_cache_overview: Option<bool>,
pub offline_cache_recent: Option<bool>,
pub offline_recent_count: Option<i64>,
```

In `get()`, after `keep_articles_days`, read the three values from `app_db` (clone to locals before `drop(app_db)` like the existing pattern) and include them in the `Settings` literal.

In `update()`, after the keep-days block, add:
```rust
if let Some(v) = req.offline_cache_overview {
    state.app_db.lock().await.set_offline_cache_overview(v)?;
}
if let Some(v) = req.offline_cache_recent {
    state.app_db.lock().await.set_offline_cache_recent(v)?;
}
if let Some(n) = req.offline_recent_count {
    if !(1..=200).contains(&n) {
        return Err(ApiError::bad_request("offline_recent_count must be 1-200"));
    }
    state.app_db.lock().await.set_offline_recent_count(n)?;
}
```

4. In `frontend/src/api/types.ts`, extend `Settings`:
```ts
offline_cache_overview: boolean;
offline_cache_recent: boolean;
offline_recent_count: number;
```

- [ ] **Step 4: Run tests**

Run: `cargo test offline_cache_settings_roundtrip_with_defaults` → PASS; then `cargo test` → all green.

- [ ] **Step 5: Commit**

```bash
git add src/app_db/mod.rs src/dto.rs src/api/settings.rs frontend/src/api/types.ts
git commit -m "feat(settings): offline cache preferences with defaults and clamp"
```

### Task 2: Service-worker runtime caching + logout clear

**Files:**
- Modify: `frontend/vite.config.ts:10-29`
- Modify: `frontend/src/auth/useSession.ts` (find logout function; add cache clear before redirect)
- Test: `bun run build` + inspect `dist/sw.js` for runtime rules (no unit test; SW behavior verified manually)

**Interfaces:**
- Consumes: Task 1 settings (not directly; SW caches regardless, warmer decides what to fetch).
- Produces: cached `/api/*` JSON + image responses; logout-time eviction.

- [ ] **Step 1: Add runtimeCaching**

In `frontend/vite.config.ts`, inside `VitePWA({ ... })` add after `workbox:` block entries:

```ts
workbox: {
  navigateFallback: "/index.html",
  globPatterns: ["**/*.{js,css,html,svg,png,ico}"],
  runtimeCaching: [
    {
      urlPattern: ({ url }) =>
        url.origin === self.location.origin &&
        (url.pathname === "/api/overview" || url.pathname.startsWith("/api/articles")),
      handler: "NetworkFirst",
      options: {
        cacheName: "feedea-api",
        networkTimeoutSeconds: 3,
        cacheableResponse: { statuses: [200] },
        expiration: { maxEntries: 300, maxAgeSeconds: 7 * 24 * 3600 },
      },
    },
    {
      urlPattern: ({ url }) =>
        url.origin === self.location.origin &&
        (url.pathname.startsWith("/img/") ||
          url.pathname.startsWith("/api/thumbnail") ||
          url.pathname.startsWith("/api/favicon")),
      handler: "StaleWhileRevalidate",
      options: {
        cacheName: "feedea-images",
        cacheableResponse: { statuses: [200] },
        expiration: { maxEntries: 300, maxAgeSeconds: 30 * 24 * 3600 },
      },
    },
  ],
},
```

Note: `urlPattern` as a function requires Workbox v7 (already via `workbox-build@7.4.1`). If TS complains about `self`, use `({ url }: { url: URL }) => url.origin === location.origin && ...`.

- [ ] **Step 2: Clear API cache on logout**

In `frontend/src/auth/useSession.ts:51-62`, inside the `logout` callback's `finally` block before `navigate("/login", ...)` (line 60), insert:

```ts
try {
  const names = await caches.keys();
  await Promise.all(names.filter((n) => n === "feedea-api").map((n) => caches.delete(n)));
} catch {
  // Cache API unavailable (private mode) — logout still proceeds.
}
```

Resulting block:
```ts
} finally {
  try {
    const names = await caches.keys();
    await Promise.all(names.filter((n) => n === "feedea-api").map((n) => caches.delete(n)));
  } catch {
    // Cache API unavailable (private mode) — logout still proceeds.
  }
  queryClient.setQueryData(SESSION_QUERY_KEY, {
```

- [ ] **Step 3: Verify build**

Run (workdir `frontend/`): `bun run build`
Expected: PASS; `grep -o "feedea-api" dist/sw.js` finds the cache name.

- [ ] **Step 4: Commit**

```bash
git add frontend/vite.config.ts frontend/src/auth/useSession.ts
git commit -m "feat(pwa): runtime-cache api and images, clear api cache on logout"
```

### Task 3: Warmer hook + Settings UI

**Files:**
- Create: `frontend/src/pwa/useOfflineWarmer.ts`
- Modify: `frontend/src/pages/Overview.tsx` (invoke warmer; no visual change)
- Modify: `frontend/src/state/hooks.ts:332-345` (`useUpdateSettings` patch type)
- Modify: `frontend/src/pages/Settings.tsx:40-110,142-222` (Offline section)
- Test: typecheck/lint + manual offline test

**Interfaces:**
- Consumes: `useSettings()` (Task 1 fields), `useOverview()` data, `api.get`, `encodeId`.
- Produces: `useOfflineWarmer(ids: string[])` — fire-and-forget warming, returns void.

- [ ] **Step 1: Create the warmer**

Create `frontend/src/pwa/useOfflineWarmer.ts`:

```ts
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, encodeId } from "../api/client";
import type { ArticleDetail, Headline } from "../api/types";
import { queryKeys } from "../state/keys";
import { useSettings } from "../state/hooks";

const MAX_WARM = 250;

async function warmDetails(queryClient: ReturnType<typeof useQueryClient>, ids: string[]) {
  const limited = [...new Set(ids)].slice(0, MAX_WARM);
  await Promise.allSettled(
    limited.map((id) =>
      queryClient.fetchQuery({
        queryKey: queryKeys.article(id),
        queryFn: () => api.get<ArticleDetail>(`/api/articles/${encodeId(id)}`),
        staleTime: 5 * 60_000,
      }),
    ),
  );
}

export function useOfflineWarmer(overviewIds: string[]) {
  const queryClient = useQueryClient();
  const { data: settings } = useSettings();
  useEffect(() => {
    if (!settings) return;
    (async () => {
      try {
        if (settings.offline_cache_overview && overviewIds.length > 0) {
          await warmDetails(queryClient, overviewIds);
        }
        if (settings.offline_cache_recent) {
          const n = Math.min(Math.max(settings.offline_recent_count || 50, 1), 200);
          const recent = await api.get<Headline[]>(`/api/articles?limit=${n}`);
          const fresh = recent.map((h) => h.id).filter((id) => !overviewIds.includes(id));
          if (fresh.length > 0) await warmDetails(queryClient, fresh);
        }
      } catch {
        // Offline or warming failure — never surface to UI.
      }
    })();
  }, [queryClient, settings, overviewIds.join("|")]); // eslint-disable-line react-hooks/exhaustive-deps
}
```

`overviewIds` is derived in `Overview.tsx` via `useMemo(() => cards.flatMap((c) => c.items.map((i) => i.id)), [cards])` and passed to the hook. The `join("|")` dependency avoids refetch loops from array identity.

- [ ] **Step 2: Wire warmer into Overview**

In `frontend/src/pages/Overview.tsx`, add:
```tsx
import { useOfflineWarmer } from "../pwa/useOfflineWarmer";
```
and inside `Overview()` after `cards` is computed:
```tsx
const warmIds = useMemo(() => cards.flatMap((c) => c.items.map((i) => i.id)), [cards]);
useOfflineWarmer(warmIds);
```
Add `useMemo` to the React import if missing.

- [ ] **Step 3: Extend `useUpdateSettings` + Settings UI**

In `frontend/src/state/hooks.ts`, extend the patch type:
```ts
mutationFn: (patch: {
  theme?: string;
  accent?: string;
  sync_interval_minutes?: number;
  keep_articles_days?: number | null;
  offline_cache_overview?: boolean;
  offline_cache_recent?: boolean;
  offline_recent_count?: number;
}) => api.patch("/api/settings", patch),
```

In `frontend/src/pages/Settings.tsx`, add state:
```tsx
const [cacheOverview, setCacheOverview] = useState(true);
const [cacheRecent, setCacheRecent] = useState(false);
const [recentCount, setRecentCount] = useState("50");
```
hydrate in the existing `useEffect` (alongside theme/accent):
```tsx
setCacheOverview(data.offline_cache_overview);
setCacheRecent(data.offline_cache_recent);
setRecentCount(String(data.offline_recent_count));
```
(guard with the same `!dirty` condition), and send in `savePreferences`:
```tsx
if (cacheOverview !== data.offline_cache_overview) patch.offline_cache_overview = cacheOverview;
if (cacheRecent !== data.offline_cache_recent) patch.offline_cache_recent = cacheRecent;
const parsedN = Number(recentCount);
if (parsedN !== data.offline_recent_count) {
  if (!Number.isInteger(parsedN) || parsedN < 1 || parsedN > 200) {
    setSavedBanner("Recent count must be 1–200.");
    return;
  }
  patch.offline_recent_count = parsedN;
}
```

Add an "Offline" section after Preferences (before Change password):
```tsx
<section className="flex flex-col gap-4 rounded-lg border border-app-border p-4">
  <h3 className="text-sm font-semibold">Offline</h3>
  <label className="flex cursor-pointer items-center justify-between gap-3 text-sm text-app-text-2">
    Cache overview articles
    <input type="checkbox" checked={cacheOverview} onChange={(e) => { setCacheOverview(e.target.checked); markDirty(); }} className="h-4 w-4 accent-[var(--accent)]" />
  </label>
  <label className="flex cursor-pointer items-center justify-between gap-3 text-sm text-app-text-2">
    Cache recent articles
    <input type="checkbox" checked={cacheRecent} onChange={(e) => { setCacheRecent(e.target.checked); markDirty(); }} className="h-4 w-4 accent-[var(--accent)]" />
  </label>
  <Field label="Recent articles to keep offline" hint="1–200. Applies when recent caching is on.">
    <input type="number" min={1} max={200} disabled={!cacheRecent} value={recentCount}
      onChange={(e) => { setRecentCount(e.target.value); markDirty(); }} className={inputClass} />
  </Field>
</section>
```

- [ ] **Step 4: Verify**

Run: `bun run typecheck` then `bun run lint` (workdir `frontend/`), `cargo test` (repo root).
Expected: all PASS.

Manual: enable both toggles (N=20), load overview online, DevTools → Offline → reload `/` → overview renders; open an article → full body renders.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/pwa/useOfflineWarmer.ts frontend/src/pages/Overview.tsx frontend/src/state/hooks.ts frontend/src/pages/Settings.tsx
git commit -m "feat(offline): warmer hook and settings UI for article cache"
```
