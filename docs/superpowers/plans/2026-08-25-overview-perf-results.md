# Overview latency — baseline vs after

Fixture: 24 categorized + 4 uncategorized feeds, 40 items each (~1120 articles).
Command: `cargo test --test overview_perf -- --ignored --nocapture`

## Baseline (main @ 16a3a65)
OVERVIEW_PERF cards=25 min_ms=89.25 median_ms=91.06 mean_ms=91.51 samples=20

## After (worktree branch @ 4f4820f — per-card parallel previews)
OVERVIEW_PERF cards=25 min_ms=19.16 median_ms=20.36 mean_ms=20.31 samples=20

## Summary
- `/api/overview` dropped from ~6N+12 sequential SQLite statements (per-card
  headline fetch + per-card mapping re-reads) to a per-request CategoryIndex
  snapshot + 1 direct-totals query + per-card parallel `get_fat_articles`
  (one query per card, run concurrently, throttled by news-flash's r2d2
  pool) + 1 global count.
- Median latency change: 91.06ms -> 20.36ms on the fixture above. ~4.5x
  faster, with NO regression vs the (rejected) single-batched-window approach.
- NOTE (2026-08-26): the original single-batched-window implementation was
  REJECTED after end-to-end verification on a real database: a global
  newest-N window starves categories whose articles fall outside it (13 of
  22 cards showed 0 items on a real 2899-article DB). The final per-card
  parallel implementation guarantees each card its own category's 6 newest
  articles and measures the same latency.