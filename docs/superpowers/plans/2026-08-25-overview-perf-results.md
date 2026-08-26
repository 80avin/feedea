# Overview latency — baseline vs after

Fixture: 24 categorized + 4 uncategorized feeds, 40 items each (~1120 articles).
Command: `cargo test --test overview_perf -- --ignored --nocapture`

## Baseline (main @ 16a3a65)
OVERVIEW_PERF cards=25 min_ms=89.25 median_ms=91.06 mean_ms=91.51 samples=20

## After (worktree branch, final task)
OVERVIEW_PERF cards=25 min_ms=... median_ms=... mean_ms=... samples=20
