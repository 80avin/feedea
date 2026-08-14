# Smoke test

`scripts/smoke.sh` exercises the release binary end to end against a fresh,
temporary data directory, without needing the public internet.

## Run

```sh
make build          # or ensure target/release/rssea exists
bash scripts/smoke.sh
```

Exit code is `0` only if every assertion passes; otherwise it exits non-zero
after printing the first `FAIL: ...` line. Every step prints a `PASS: ...`
line as it is verified.

## What it verifies

1. **Startup + health** — starts `target/release/rssea` with an empty temp
   data dir, waits for `/api/health` (200, `{"status":"ok"}`).
2. **First-run password** — parses `rssea initial password: <token>` from the
   server's stderr log.
3. **Session + login** — `/api/session` reports not authenticated before login;
   `POST /api/login` with the printed password returns a `rssea_session` cookie
   and `/api/session` reports authenticated afterwards.
4. **Add a feed** — serves a local RSS fixture over `python3 -m http.server`
   on 127.0.0.1, adds it via `POST /api/sources`, then triggers
   `POST /api/sources/refresh-all`.
5. **Overview** — `/api/overview` is non-empty (≥ 2 articles).
6. **Saved round-trip** — saves the newest article via
   `POST /api/articles/{id}/save`, then `/api/saved` reports `total ≥ 1`.
7. **Settings round-trip** — `PATCH /api/settings` (theme + sync interval)
   and a follow-up `GET` confirms both persisted.
8. **Static + SPA fallback + PWA** — `GET /` and a deep link like
   `/feeds/<anything>` both return 200 with the `id="root"` app shell;
   `/manifest.webmanifest` returns the PWA manifest with icons.
9. **Shutdown** — kills the server and cleans up the temp dir via a trap.

The script needs `python3` and `curl`; if the release binary is missing it
runs `make build` first (which needs `bun`).
