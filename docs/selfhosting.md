# Self-hosting feedea

## Single-binary deployment

feedea ships as one static-ish binary: the React frontend is embedded at
compile time via `rust-embed`, so there is no separate static file server.

```sh
make build
./target/release/feedea --data-dir /var/lib/feedea
```

You can cross-compile or copy `target/release/feedea` to the host — no runtime
dependencies other than the data directory.

## systemd unit

Save as `/etc/systemd/system/feedea.service`:

```ini
[Unit]
Description=feedea feed aggregator
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=feedea
Group=feedea
WorkingDirectory=/var/lib/feedea
ExecStart=/usr/local/bin/feedea --data-dir /var/lib/feedea
Environment=FEEDEA_HOST=0.0.0.0
Environment=FEEDEA_PORT=3000
# Restrict proxying to public networks only (recommended for a public instance):
Environment=FEEDEA_ALLOW_PRIVATE_PROXY=false
Restart=on-failure
RestartSec=5

# hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ProtectHome=true
ReadWritePaths=/var/lib/feedea

[Install]
WantedBy=multi-user.target
```

Set up:

```sh
sudo useradd --system --home /var/lib/feedea --shell /usr/sbin/nologin feedea
sudo mkdir -p /var/lib/feedea
sudo chown feedea:feedea /var/lib/feedea
sudo install -m 755 target/release/feedea /usr/local/bin/feedea
sudo systemctl daemon-reload
sudo systemctl enable --now feedea
```

### Behind a reverse proxy

feedea serves both the API and the SPA itself, so a proxy like Caddy or
nginx only needs to forward `/`:

```text
# Caddyfile
example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Because the SPA uses client-side routing, the proxy must not rewrite
unknown paths to 404 — feedea handles the SPA fallback internally, so plain
`reverse_proxy` to the root is sufficient.

## First run

On first start with an empty data dir, feedea:

1. creates the data directory and both databases;
2. generates a random password, hashes it, stores it, and prints it to
   stderr (log output), e.g.:

   ```
   feedea initial password: <generated>
   log in at /api/login (use the web UI) and change it in Settings
   ```

3. starts the HTTP server.

Log in with that password at `http://host:3000/` and change it in
Settings. If you lose it, delete the `password_hash` row from
`feedea.sqlite` (`settings` table) and restart — a new one will be printed.

## Environment variables

See the README table. All flags map to env vars:

| env                          | default            |
| ---------------------------- | ------------------ |
| `FEEDEA_DATA_DIR`             | `~/.local/share/feedea` |
| `FEEDEA_HOST`                 | `0.0.0.0`          |
| `FEEDEA_PORT`                 | `3000`             |
| `FEEDEA_ALLOW_PRIVATE_PROXY`  | `false`            |

## Data directory layout

```
<data_dir>/
  feedea.sqlite              # feedea settings, sessions, password hash
  feedea.sqlite-wal          # SQLite WAL (safe to delete when stopped)
  feedea.sqlite-shm          # SQLite shared memory (safe to delete when stopped)
  engine/
    config/                 # news-flash engine config
    data/
      database.sqlite       # news-flash feed/article database
```

To migrate or back up, copy the entire `<data_dir>` while the service is
stopped (or use `sqlite3 .backup` on the running DBs).

## Updating

```sh
git pull
make build                # rebuilds frontend + backend
sudo systemctl restart feedea
```

The schema is additive; no data migration step is required.

## Troubleshooting

- **`frontend/dist is missing` at compile time**: run `make build`, which
  builds the frontend before `cargo build`. (`frontend/dist` is git-ignored
  on purpose.) `make test` builds the frontend itself first, so it works from
  a fresh checkout.
- **No initial password printed**: make sure the data dir is empty (or the
  `password_hash` setting is absent). Check `journalctl -u feedea` for the
  line `feedea initial password: ...`.
- **Port already in use**: change `FEEDEA_PORT` or pass `--port`.
- **Images don't load from a private/LAN network**: feedea blocks proxying to
  private IP ranges by default (SSRF protection). Set
  `FEEDEA_ALLOW_PRIVATE_PROXY=true` only if you trust your feeds.
- **Proxy 502 in dev**: Vite proxies `/api` and `/img` to `127.0.0.1:3000` —
  make sure the backend (`make backend-dev`) is running.
- **bun: command not found**: `export PATH="$HOME/.bun/bin:$PATH"` or install
  bun per its installer.

## License note

feedea links against [news-flash](https://crates.io/crates/news-flash), which
is GPL-3.0-or-later. If you distribute the compiled binary, the corresponding
source (including the GPL-3.0 license text) must be made available to
recipients.
