# Reader bottom-padding fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Article content on mobile is never hidden behind the fixed reader action bar.

**Architecture:** CSS-only fix in `Reader.tsx`: enlarge bottom padding of the scroll content to clear the fixed bar height plus iOS safe-area, and pad the bar itself for the home indicator. No layout restructuring, no backend change.

**Tech Stack:** React + Tailwind v4, existing `MobileLayout` scroll container.

**Spec:** `docs/superpowers/specs/2026-10-10-reader-overview-offline-design.md` (section 2)

## Global Constraints

- Touch only `frontend/src/components/Reader.tsx` for product code.
- Keep desktop (`lg:`) behavior unchanged.
- Verify with `bun run typecheck` from `frontend/`.

## Review Focus

- Very long article scrolled to end: last line fully visible above the bar.
- Article with note box at end: note box fully visible above the bar.
- Desktop width (>=1024px): no extra bottom gap, no floating bar.
- iOS safe-area device: bar sits above home indicator, content clears it.
- Empty/loading states: no visual regression.

---

### Task 1: Reader bottom padding

**Files:**
- Modify: `frontend/src/components/Reader.tsx:64,105,115-120`
- Test: manual viewport check + typecheck (no unit test infra for CSS; a snapshot would be brittle and is deliberately omitted per user test policy)

**Interfaces:**
- Consumes: existing `MobileLayout` scroll container (`main.min-h-0.flex-1.overflow-y-auto`).
- Produces: content bottom clearance >= fixed bar height + safe-area.

- [ ] **Step 1: Apply the padding fix**

In `frontend/src/components/Reader.tsx`, make these exact changes:

1. Root wrapper (line 64), change:
```tsx
<div className="flex h-full flex-col pb-20 lg:pb-0">
```
to:
```tsx
<div className="flex h-full flex-col pb-[calc(5.5rem+env(safe-area-inset-bottom))] lg:pb-0">
```

2. Content container (line 105), change:
```tsx
<div className="mt-4 min-h-0 flex-1 px-4 pb-4">
```
to:
```tsx
<div className="mt-4 min-h-0 flex-1 scroll-pb-28 px-4 pb-8">
```

3. Fixed action bar (lines 115-118), change:
```tsx
className="fixed inset-x-0 bottom-0 z-20 border-t border-app-border bg-app-bg/95 px-4 py-1.5 backdrop-blur lg:hidden"
```
to:
```tsx
className="fixed inset-x-0 bottom-0 z-20 border-t border-app-border bg-app-bg/95 px-4 pt-1.5 pb-[calc(0.375rem+env(safe-area-inset-bottom))] backdrop-blur lg:hidden"
```

- [ ] **Step 2: Typecheck**

Run: `bun run typecheck` (workdir `frontend/`)
Expected: PASS with no errors.

- [ ] **Step 3: Manual viewport check**

Run: `bun run dev` (workdir `frontend/`, backend via `make backend-dev` or existing), open a long article at 390px width, scroll to end.
Expected: last paragraph and note box fully visible above the bar; bar does not overlap text.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/Reader.tsx
git commit -m "fix(reader): clear mobile action bar with safe-area padding"
```
