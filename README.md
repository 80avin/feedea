# rssea

A self-hosted RSS feed aggregator with a built-in web UI.

Single Rust binary, embedded React frontend, SQLite storage. Built on the
[news-flash](https://crates.io/crates/news-flash) engine.

## Features

- Feed management: add, discover, import/export OPML, per-feed refresh
- Articles: list, search, save with notes and tags, mark read/unread
- Categories and unread badges
- Privacy-friendly image proxy (private-network proxying off by default)
- PWA: installable, works offline after first load

## Requirements

- Rust toolchain (stable) + `cargo`
- [bun](https://bun.sh) for the frontend (used for both dev and build)

If bun is not on your PATH (e.g. installed via the bun installer into
`~/.bun/bin`), add it:

```sh
export PATH="$HOME/.bun/bin:$PATH"
```

## Quick start (production binary)

```sh
make build        # builds frontend then cargo build --release
./target/release/rssea --data-dir /path/to/data
```

On first run rssea prints a generated password to stderr. Log in at the web
UI and change it in Settings.

Default data dir (when `--data-dir` is omitted):
`$HOME/.local/share/rssea` (or `RSSEA_DATA_DIR`).

## Development

Run backend and frontend together:

```sh
make dev
```

- backend: `cargo run` on port 3000
- frontend: `bun run dev` (Vite) on port 5173, proxying `/api` and `/img` to
  the backend

Or run them in two terminals:

```sh
# terminal 1 — backend
make backend-dev

# terminal 2 — frontend
make frontend-dev
```

Then open http://localhost:5173.

### Tests

```sh
make test     # cargo test + frontend typecheck/lint
```

### Other targets

| target           | purpose                                            |
| ---------------- | -------------------------------------------------- |
| `make build`     | `bun run build` then `cargo build --release`       |
| `make run`       | `cargo run --release` (single binary)              |
| `make dev`       | backend + Vite concurrently                        |
| `make test`      | cargo tests + frontend typecheck/lint              |
| `make clean`     | remove `target/`, `frontend/dist`, `frontend/node_modules` |

## Configuration

The binary accepts these flags; each can also be set as an environment
variable:

| flag                      | env                         | default                     | description                        |
| ------------------------- | --------------------------- | --------------------------- | ---------------------------------- |
| `--data-dir`              | `RSSEA_DATA_DIR`            | `~/.local/share/rssea`      | data directory                     |
| `--host`                  | `RSSEA_HOST`                | `0.0.0.0`                   | listen address                     |
| `--port`                  | `RSSEA_PORT`                | `3000`                      | listen port                        |
| `--allow-private-proxy`   | `RSSEA_ALLOW_PRIVATE_PROXY` | `false`                     | allow proxying images from private networks |

Example:

```sh
RSSEA_PORT=8080 RSSEA_ALLOW_PRIVATE_PROXY=1 ./target/release/rssea
```

### Data directory layout

```
<data_dir>/
  rssea.sqlite              # rssea settings, sessions
  rssea.sqlite-wal, -shm    # SQLite WAL files
  engine/
    config/                 # news-flash config
    data/
      database.sqlite       # news-flash feed/article database
```

Back up the whole `<data_dir>` for a full snapshot.

## Self-hosting

See [docs/selfhosting.md](docs/selfhosting.md) for the systemd unit,
deployment notes, and troubleshooting.

## License

rssea links against [news-flash](https://crates.io/crates/news-flash), which
is licensed GPL-3.0-or-later. The combined binary is therefore distributed
under GPL-3.0-or-later — provide the corresponding source if you distribute
the binary. See `Cargo.lock` for the full dependency tree and licenses.
