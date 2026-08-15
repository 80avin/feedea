# Phase 4: Frontend Shell + PWA — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the React + HeroUI v3 + Vite + bun PWA with the desktop 3-panel and mobile bottom-nav shells, routing, a typed API client, React Query wiring, the search bar, and the infinite-scroll hook infrastructure — all fetching from the real Phase 3 backend.

**Architecture:** Vite SPA in `frontend/` (bun-managed). React Router v8 defines routes: `/login`, `/` (desktop shell with layout routes), and mobile variants resolved via responsive CSS/Tailwind (same components, different layout). `@tanstack/react-query` for all server state with an infinite-query timeline. A typed fetch client wraps the `/api` endpoints (session cookie flows automatically). The shell renders the desktop 3-panel (sidebar | timeline | reader) on `lg+` and the mobile bottom-nav single-panel on `<lg`. The dev server proxies `/api`, `/img` to `:3000`.

**Tech Stack (verified pins):** react 19.2.8, react-dom 19.2.8, @heroui/react 3.2.4, @heroui/styles 3.2.4, @heroicons/react 2.2.0, @tanstack/react-query 5.101.4, clsx 2.1.1, react-router 8.3.0, tailwindcss 4.3.3, @tailwindcss/vite 4.3.3, @vitejs/plugin-react 6.0.5, typescript 6.0.3, vite 8.2.1, vite-plugin-pwa 1.3.0, workbox-build 7.4.1, workbox-window 7.4.1, typescript-eslint 8.67.0. Managed by bun.

## Global Constraints

- HeroUI v3: NO `HeroUIProvider`, NO framer-motion. CSS order in `src/globals.css`: `@import "tailwindcss";` then `@import "@heroui/styles";`.
- react-router v8 imports come from `react-router` directly (`BrowserRouter`, `Routes`, `Route`, `Outlet`, `useNavigate`, `useLocation`, `Link`, `NavLink`, `useSearchParams`).
- TS 6.0.3 (typescript-eslint caps <6.1). tsconfig `types: ["vite/client", "vite-plugin-pwa/client"]`.
- All API calls go through the typed client in `frontend/src/api/` — no raw fetch in components.
- Session cookie is HttpOnly and automatic (same origin in prod; dev proxy forwards it). The client must treat 401 from `/api/*` (except login/session) as "redirect to /login".
- `/img?u=...` is unauthenticated and used directly in `<img src>` (article reader).
- PWA: `registerType: "autoUpdate"`, `registerSW({ immediate: true })` from `virtual:pwa-register`. SW caches app shell only, NOT `/api` or `/img`.
- No comments in code unless a task explicitly shows them.
- Commit after each task with the exact message given.
- `bun` is the package manager; `bun.lockb` committed. No `npm`.

---

### Task 1: Scaffold frontend (bun + vite + ts + tailwind + heroui + react-query + router + pwa)

**Files:**
- Create: `frontend/package.json`, `frontend/bun.lockb`, `frontend/vite.config.ts`, `frontend/tsconfig.json`, `frontend/index.html`, `frontend/public/manifest.webmanifest`, `frontend/src/globals.css`, `frontend/src/main.tsx`, `frontend/src/App.tsx`, `frontend/src/pages/Login.tsx`, `frontend/src/pages/Home.tsx`, `frontend/.gitignore`
- Modify: `.gitignore` (root) to add `/frontend/node_modules`, `/frontend/dist`

**Interfaces:**
- Consumes: nothing (fresh scaffold).
- Produces: `bun run dev` starts Vite on :5173 proxying `/api` and `/img` to `http://localhost:3000`; `bun run build` produces `dist/` with SW + manifest; `bun run typecheck` (tsc --noEmit) clean; `bun run lint` (eslint) clean. `src/main.tsx` renders the app with QueryClientProvider + BrowserRouter + PWA register. `App.tsx` has routes for `/login` and `/` (Home placeholder).

- [ ] **Step 1: Create `frontend/package.json`** with the exact pins above and scripts:

```json
{
  "name": "feedea-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit",
    "lint": "eslint ."
  },
  "dependencies": {
    "@heroui/react": "3.2.4",
    "@heroui/styles": "3.2.4",
    "@heroicons/react": "2.2.0",
    "@tanstack/react-query": "5.101.4",
    "clsx": "2.1.1",
    "react": "19.2.8",
    "react-dom": "19.2.8",
    "react-router": "8.3.0"
  },
  "devDependencies": {
    "@tailwindcss/vite": "4.3.3",
    "@types/react": "19.2.18",
    "@types/react-dom": "19.2.4",
    "@vitejs/plugin-react": "6.0.5",
    "tailwindcss": "4.3.3",
    "typescript": "6.0.3",
    "typescript-eslint": "8.67.0",
    "vite": "8.2.1",
    "vite-plugin-pwa": "1.3.0",
    "workbox-build": "7.4.1",
    "workbox-window": "7.4.1"
  }
}
```

- [ ] **Step 2: `frontend/vite.config.ts`**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: "autoUpdate",
      manifest: {
        name: "feedea",
        short_name: "feedea",
        start_url: "/",
        display: "standalone",
        theme_color: "#0a0a0a",
        background_color: "#ffffff",
        icons: [],
      },
      workbox: {
        navigateFallback: "/index.html",
        globPatterns: ["**/*.{js,css,html,svg,png,ico}"],
      },
    }),
  ],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:3000",
      "/img": "http://localhost:3000",
    },
  },
});
```

- [ ] **Step 3: `frontend/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client", "vite-plugin-pwa/client"]
  },
  "include": ["src"]
}
```

- [ ] **Step 4: `frontend/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />
    <meta name="theme-color" content="#0a0a0a" />
    <link rel="manifest" href="/manifest.webmanifest" />
    <title>feedea</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: `frontend/src/globals.css`** — order matters:

```css
@import "tailwindcss";
@import "@heroui/styles";
```

- [ ] **Step 6: `frontend/src/main.tsx`**

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Route, Routes } from "react-router";
import { registerSW } from "virtual:pwa-register";
import App from "./App";
import "./globals.css";

registerSW({ immediate: true });

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 30_000, retry: 1 },
  },
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<App />} />
          <Route path="*" element={<App />} />
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>
);
```

(Simplify: for Task 1 just render `<App />` under `/`; `/login` routing is refined in Task 2. Use a single catch-all for now — refine later.)

- [ ] **Step 7: `frontend/src/App.tsx` (placeholder) + `Login.tsx` + `Home.tsx` (placeholders)**

`App.tsx` renders a minimal shell: a `<main>` with an h1 "feedea" and a `<Button>`-free placeholder. `Login.tsx` and `Home.tsx` are minimal functional components imported but the router wiring happens in Task 2; for Task 1 keep App minimal and compilable.

- [ ] **Step 8: `frontend/.gitignore` + root .gitignore update**

`frontend/.gitignore`: `node_modules/`, `dist/`, `*.local`. Root `.gitignore` add `/frontend/node_modules`, `/frontend/dist`.

- [ ] **Step 9: Install + verify**

Run: `cd frontend && bun install`, then `bun run typecheck`, `bun run build`, `bun run dev` (background, curl :5173, expect 200). All must succeed. `bun run lint` — if ESLint has no config yet, create `frontend/eslint.config.js`:

```js
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "node_modules"] },
  tseslint.configs.recommended,
);
```

and adjust as needed so `bun run lint` passes on the scaffold.

- [ ] **Step 10: Commit**

```bash
git add frontend/ .gitignore && git commit -m "Phase 4: scaffold frontend with vite, heroui, react-query, router, and pwa"
```

---

### Task 2: Typed API client + session handling + login flow + shell layout routes

**Files:**
- Create: `frontend/src/api/client.ts`, `frontend/src/api/types.ts`, `frontend/src/auth/useSession.ts`, `frontend/src/pages/Login.tsx` (real), `frontend/src/components/Shell.tsx`, `frontend/src/pages/Home.tsx` (real placeholder)
- Modify: `frontend/src/App.tsx` (routes: `/login`, `/` → Shell with Home outlet)

**Interfaces:**
- Consumes: backend `/api/session`, `/api/login`, `/api/logout`.
- Produces:
  - `src/api/types.ts`: TypeScript interfaces for the API DTOs we already consume — `SessionInfo { authenticated: boolean; version: string; setup_required: boolean }`, `FeedSummary`, `Headline`, `CategoryCard`, `OverviewResponse`, `Settings`, `ErrorEnvelope { error: { code: string; message: string } }`. Match the backend JSON exactly (camelCase).
  - `src/api/client.ts`: `async function apiFetch<T>(path: string, init?: RequestInit): Promise<T>` — wraps fetch, JSON, throws `ApiError { status, code, message }` on non-2xx, and for 401 (except on `/api/login`/`/api/session`) triggers an auth-failure event. Also `apiGet`, `apiPost`, `apiPatch`, `apiDelete` helpers. `export const api = { get: apiGet, post: apiPost, ... }`.
  - `src/auth/useSession.ts`: a React hook `useSession()` returning `{ session, loading, login, logout, refresh }` — on mount calls `/api/session`; if `!authenticated` and not on `/login`, navigates to `/login`. `login(password)` POSTs `/api/login`, then refreshes. `logout()` POSTs `/api/logout` and navigates to `/login`.
  - `Login.tsx`: HeroUI v3 form — an `<Input type="password">` + `<Button>`; on submit calls `login`; shows error on failure; if `setup_required`, shows a hint (the server printed an initial password to its log — display that message).
  - `Shell.tsx`: renders the route `<Outlet />` with the top-level auth guard (via useSession) and minimal layout chrome (for now a header with the app name + a Logout button; full 3-panel/bottom-nav in Task 3).
  - `App.tsx`: `<Routes><Route path="/login" element={<Login />} /><Route path="/" element={<Shell />}><Route index element={<Home />} /></Route></Routes>`.

- [ ] **Step 1: Write `src/api/types.ts`** (interfaces above; keep them minimal and matching the backend). No test (types are compile-checked).

- [ ] **Step 2: Write `src/api/client.ts`** per the interface. Handle JSON errors; on 401 dispatch a window event `feedea:unauthorized` (useSession listens). No comments.

- [ ] **Step 3: Write `src/auth/useSession.ts`** per the interface. Use `useEffect` + state (or React Query `useQuery` for the session — React Query is fine: `useQuery({ queryKey: ["session"], queryFn: ... })`). On mount, if session not authenticated and path !== "/login", `useNavigate()("/login")`.

- [ ] **Step 4: Write `Login.tsx` and `Shell.tsx`** per interface using HeroUI v3 components (`Input`, `Button`, `Card`). Check HeroUI v3 docs/source for the current component + prop names (the v2→v3 migration renamed Button variants: `variant="primary"`, palette `color="accent"`). Verify against the installed package's types (e.g. `node_modules/@heroui/react/dist/*.d.ts`) if unsure.

- [ ] **Step 5: Update `App.tsx`** with the routes per the interface.

- [ ] **Step 6: Manual verification with the backend**

Start the backend (`cargo run -- --data-dir /tmp/feedea-dev-data` from the repo root; note it prints an initial password to stderr on first run) and the frontend (`bun run dev`). Open `http://localhost:5173` → should redirect to `/login`; log in with the printed password → lands on Home. Verify in browser. (The initial password is random; you may also `PATCH /api/settings/password` after logging in. Alternatively seed via env — but the printed password flow is fine for a manual check.)

- [ ] **Step 7: Commit**

```bash
git add frontend/src && git commit -m "Phase 4: add typed api client, session auth, login, and shell routing"
```

---

### Task 3: Desktop 3-panel + mobile bottom-nav shell

**Files:**
- Create: `frontend/src/components/Sidebar.tsx`, `frontend/src/components/TimelinePanel.tsx` (placeholder), `frontend/src/components/ReaderPanel.tsx` (placeholder), `frontend/src/components/BottomNav.tsx`, `frontend/src/layout/DesktopLayout.tsx`, `frontend/src/layout/MobileLayout.tsx`
- Modify: `frontend/src/components/Shell.tsx` (responds to breakpoint), `frontend/src/App.tsx` (routes for overview/feeds/saved/settings + reader outlet), `frontend/src/pages/Overview.tsx`/`Feeds.tsx`/`Saved.tsx`/`Settings.tsx` (placeholder pages)

**Interfaces:**
- Consumes: React Router, HeroUI v3 (`Listbox`, `Button`, `Separator`, icons via `@heroicons/react`).
- Produces:
  - `DesktopLayout.tsx`: 3 columns — left `Sidebar` (fixed nav + scrollable sections), center timeline outlet, right reader outlet (reader hidden when not selected, or when Overview is active → 2 columns). Grid via Tailwind: `lg:grid lg:grid-cols-[240px_minmax(0,1fr)_minmax(0,1fr)]` etc.
  - `MobileLayout.tsx`: single-panel page + bottom `BottomNav` (Feed, Sources, Overview, Saved, Settings).
  - `Sidebar.tsx`: Section 1 (Overview, Feeds, Saved) top; Section 2 (Categories, scrollable, only when Feeds active); Section 3 (Sources tree, scrollable, only when Feeds active); spacer; Section 4 (Help, Settings) bottom. Uses `NavLink` for navigation.
  - `BottomNav.tsx`: 5 items with icons, `NavLink` active styling.
  - `Shell.tsx`: chooses DesktopLayout vs MobileLayout by breakpoint. Simplest robust approach: render BOTH and use Tailwind responsive classes to show/hide (`hidden lg:flex` / `lg:hidden`). That avoids a JS matchMedia hook. The reader on mobile is a separate route (full page), on desktop the right panel.
  - Pages: `Overview.tsx`, `Feeds.tsx`, `Saved.tsx`, `Settings.tsx` as minimal placeholders (an h2 + a note "coming in Phase 5") so routing works. Reader route: `/feeds/:articleId` (mobile full page) and desktop reader panel fed by selected article id (state in a small context or via URL search param — decide: use URL `?article=<id>` on the Feeds route so it's shareable; the DesktopLayout reads it for the right panel and the MobileLayout renders it as a full page).

- [ ] **Step 1: Create layout + nav components** per interfaces. Verify HeroUI v3 component prop names against installed types.
- [ ] **Step 2: Create placeholder pages** and wire routes in App.tsx.
- [ ] **Step 3: Verify**: `bun run typecheck`, `bun run build`, `bun run dev` (manual: log in, navigate between nav items, confirm responsive switch by resizing).
- [ ] **Step 4: Commit**

```bash
git add frontend/src && git commit -m "Phase 4: add desktop 3-panel and mobile bottom-nav shell"
```

---

### Task 4: React Query data layer — feeds, overview, timeline infinite query, saved, settings

**Files:**
- Create: `frontend/src/state/hooks.ts` (all data hooks), `frontend/src/state/keys.ts` (query key factory)
- Modify: `frontend/src/pages/Overview.tsx` (real), `frontend/src/pages/Feeds.tsx` (real timeline), `frontend/src/pages/Saved.tsx` (real), `frontend/src/pages/Settings.tsx` (real)
- Modify: `frontend/src/components/TimelinePanel.tsx`, `frontend/src/components/Sidebar.tsx` (categories + sources from hooks)

**Interfaces:**
- Consumes: `api` client, backend endpoints.
- Produces in `frontend/src/state/`:
  - `keys.ts`: `export const queryKeys = { overview: ["overview"] as const, feeds: ["feeds"] as const, articles: (params) => ["articles", params] as const, saved: (params) => ["saved", params] as const, settings: ["settings"] as const, categories: ["categories"] as const, tags: ["tags"] as const };`
  - `hooks.ts`:
    - `useOverview()` → `useQuery({ queryKey: queryKeys.overview, queryFn: () => api.get<OverviewResponse>("/api/overview") })`
    - `useFeeds()` → feeds
    - `useCategories()` → categories tree
    - `useTags()` → tags
    - `useSettings()` / `useUpdateSettings()` → get/patch
    - `useArticles(params)` → `useInfiniteQuery` on `/api/articles?...&offset=&limit=30`, `getNextPageParam` from returned length (if page returned < limit, no next page)
    - `useSaved(params)` → saved months
    - `useArticle(id)` → article detail
    - Mutations: `useSaveArticle()`, `useUnsaveArticle()`, `useMarkRead()`, `useMarkAllRead()`, `useRefreshFeed()`, `useAddSource()`, `useDeleteFeed()`, `useAddCategory()`, `useDeleteCategory()`, `useRenameCategory()`, `useMarkCategoryRead()`, `useChangePassword()`, `useImportOpml()`, `useExportOpml()`, `useDiscover()`
- Pages: Overview renders cards from `useOverview`; Feeds renders the timeline via `useArticles` with `useInfiniteScroll` (Task 5) and a search bar (Task 5) — for this task, render the list with a "Load more" button fallback until the infinite-scroll hook lands; Saved renders month groups; Settings renders a form (theme select, sync interval, keep-articles, change password) wired to mutations.
- Add a shared `Skeleton`/loading + `ErrorState` component in `frontend/src/components/Feedback.tsx`.

- [ ] **Step 1: Create `keys.ts` + `hooks.ts`** with all hooks, matching backend response shapes.
- [ ] **Step 2: Implement `Feedback.tsx`** (loading skeleton + error message).
- [ ] **Step 3: Implement Overview, Feeds (with Load-more), Saved, Settings** pages against the hooks.
- [ ] **Step 4: Verify**: typecheck, build, dev against the backend — overview shows cards, feeds list loads, saved shows saved items (save one via the API or a quick curl), settings reads/writes.
- [ ] **Step 5: Commit**

```bash
git add frontend/src && git commit -m "Phase 4: add react-query data layer and wire overview, feeds, saved, settings pages"
```

---

### Task 5: Search bar (debounced suggestions) + infinite scroll hook

**Files:**
- Create: `frontend/src/hooks/useDebounce.ts`, `frontend/src/hooks/useInfiniteScroll.ts`, `frontend/src/components/SearchBar.tsx`, `frontend/src/components/SearchSuggestions.tsx`
- Modify: `frontend/src/pages/Feeds.tsx`, `frontend/src/state/hooks.ts` (suggestions query), `frontend/src/components/TimelinePanel.tsx`

**Interfaces:**
- Consumes: backend `/api/search/suggestions?q=`, `/api/articles?search=...&offset=&limit=`.
- Produces:
  - `useDebounce.ts`: `export function useDebounce<T>(value: T, delayMs: number): T`.
  - `useInfiniteScroll.ts`: `export function useInfiniteScroll(onBottom: () => void, options?: { rootMargin?: string }): (node: Element | null) => void` — a sentinel ref callback using `IntersectionObserver`; when the sentinel intersects (rootMargin ~"250px"), calls `onBottom` once per transition.
  - `SearchBar.tsx`: an `<Input>` with a dropdown of suggestions. On typing: debounce 300ms → `useSuggestions(q)` → top 5-8; shows a list; clicking a suggestion navigates to `/feeds?article=<id>` (opens reader). On **Enter**: applies the query as a filter to the timeline (`useSearchParams` `?q=`), which drives `useArticles`'s `search` param.
  - Feeds page: wires `SearchBar`, `useInfiniteQuery` + `useInfiniteScroll` (prefetch next page when within ~250px of the bottom, per the roadmap), renders TimelinePanel items.
  - `TimelinePanel.tsx`: renders a headline item (title, age, source, thumbnail left, content-thumb right per spec) with the infinite-scroll sentinel at the bottom.
- Age formatting: a small `formatAge(iso: string): string` helper ("2h", "3d", "5m").

- [ ] **Step 1: Implement `useDebounce` + `useInfiniteScroll`** (pure hooks; can unit-test `useDebounce` with a tiny timer-based test if a test runner is configured — if not, rely on typecheck).
- [ ] **Step 2: Implement `SearchBar` + `SearchSuggestions`**.
- [ ] **Step 3: Wire Feeds page**: search params `q`, infinite query with `useInfiniteScroll` sentinel, timeline items with age/source/thumbnails.
- [ ] **Step 4: Verify** against backend: type `prog` in the search bar → suggestions dropdown appears (debounced); Enter → timeline filtered; scrolling to near-bottom prefetches the next page.
- [ ] **Step 5: Commit**

```bash
git add frontend/src && git commit -m "Phase 4: add debounced search suggestions, search bar, and infinite scroll"
```

---

## Self-Review Notes

- Spec coverage: Phase 4 delivers the shell, auth, routing, data layer, search bar, and infinite-scroll infrastructure per spec §7 (Layout, PWA, Search behavior, Infinite scroll). Phase 5 (next) fills in the full pages (overview cards detail, reader view with actions, sources page/popover, saved with note/tag editing, settings polish).
- Placeholder scan: pages Overview/Feeds/Saved/Settings are real in Task 4; only the reader panel is a placeholder until Phase 5. No TBDs.
- Type consistency: API DTO types match the Phase 3 backend JSON exactly (camelCase: `category_id`, `total_count`, `unread_count`, `sync_interval_minutes`, `keep_articles_days`, etc.). The client is the single source of truth for the frontend.
- Known Phase 4 constraints carried from Phase 3 parking: 3xx image redirects → 502 (reader may show broken images for redirecting hosts — Phase 5 note); `/api/thumbnail/:id` may 404 when no thumbnail exists (UI must handle gracefully).
- HeroUI v3 API drift: components are not the v2 API. Implementers MUST verify prop names against `node_modules/@heroui/react` types when unsure (palette `accent`, Button `variant="primary"`, etc.).
