#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.bun/bin:$PATH"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/rssea"

if [[ ! -x "$BIN" ]]; then
  echo "release binary not found; running make build"
  (cd "$ROOT" && make build)
fi

TMP="$(mktemp -d)"
APP_PID=""
FIXTURE_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]]; then kill "$APP_PID" 2>/dev/null || true; fi
  if [[ -n "$FIXTURE_PID" ]]; then kill "$FIXTURE_PID" 2>/dev/null || true; fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

find_free_port() {
  python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'
}

APP_PORT="$(find_free_port)"
FIXTURE_PORT="$(find_free_port)"
BASE="http://127.0.0.1:$APP_PORT"
COOKIE=""

FIXTURE_DIR="$TMP/fixture"
mkdir -p "$FIXTURE_DIR"

cat > "$FIXTURE_DIR/feed.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Smoke Fixture Feed</title>
    <link>http://example.com/</link>
    <description>local fixture for the smoke test</description>
    <item>
      <title>Smoke Article One</title>
      <link>http://example.com/one</link>
      <guid isPermaLink="false">smoke-1</guid>
      <pubDate>Mon, 10 Aug 2026 09:00:00 GMT</pubDate>
      <description>First fixture article.</description>
    </item>
    <item>
      <title>Smoke Article Two</title>
      <link>http://example.com/two</link>
      <guid isPermaLink="false">smoke-2</guid>
      <pubDate>Tue, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Second fixture article.</description>
    </item>
    <item>
      <title>Smoke Article Three</title>
      <link>http://example.com/three</link>
      <guid isPermaLink="false">smoke-3</guid>
      <pubDate>Wed, 12 Aug 2026 11:00:00 GMT</pubDate>
      <description>Third fixture article.</description>
    </item>
  </channel>
</rss>
EOF

python3 -m http.server "$FIXTURE_PORT" --bind 127.0.0.1 --directory "$FIXTURE_DIR" >/dev/null 2>&1 &
FIXTURE_PID=$!

mkdir -p "$TMP/data"
"$BIN" --data-dir "$TMP/data" --host 127.0.0.1 --port "$APP_PORT" 2>"$TMP/server.log" &
APP_PID=$!

echo "waiting for $BASE/api/health"
for _ in $(seq 1 60); do
  if curl -sf -o /dev/null "$BASE/api/health"; then break; fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "FAIL: rssea exited during startup"
    cat "$TMP/server.log" 2>/dev/null || true
    exit 1
  fi
  sleep 1
done

if ! curl -sf -o /dev/null "$BASE/api/health"; then
  echo "FAIL: timed out waiting for $BASE/api/health after 60s"
  cat "$TMP/server.log" 2>/dev/null || true
  exit 1
fi

check() {
  local desc="$1" expected="$2" method="$3" path="$4"
  shift 4
  local args=(-s -o "$TMP/last_body" -D "$TMP/last_headers" -w '%{http_code}')
  if [[ -n "$COOKIE" ]]; then args+=(-H "Cookie: $COOKIE"); fi
  args+=(-X "$method")
  if [[ $# -gt 0 ]]; then args+=("$@"); fi
  local code
  code="$(curl "${args[@]}" "$BASE$path")" || { echo "FAIL: $desc (curl failed)"; exit 1; }
  if [[ "$code" != "$expected" ]]; then
    echo "FAIL: $desc (expected $expected, got $code)"
    cat "$TMP/last_body" 2>/dev/null || true
    exit 1
  fi
  echo "PASS: $desc ($code)"
}

assert_json() {
  local file="$1" expr="$2"
  if ! python3 -c "import json,sys; d=json.load(open(sys.argv[1])); assert ($expr), d" "$file"; then
    echo "FAIL: $expr"
    exit 1
  fi
  echo "PASS: $expr"
}

echo "== health =="
check "health endpoint returns 200" 200 GET /api/health
assert_json "$TMP/last_body" 'd["status"] == "ok"'

echo "== first-run password =="
PASSWORD="$(grep -o 'rssea initial password: [0-9a-f]*' "$TMP/server.log" | awk '{print $NF}' | tail -1)"
if [[ -z "$PASSWORD" ]]; then
  echo "FAIL: no initial password in server log"
  cat "$TMP/server.log" 2>/dev/null || true
  exit 1
fi
echo "PASS: captured initial password from server log"

echo "== session before login =="
check "session endpoint before login" 200 GET /api/session
assert_json "$TMP/last_body" 'd["authenticated"] is False and d["setup_required"] is False'

echo "== login =="
check "login with initial password" 200 POST /api/login \
  -H 'Content-Type: application/json' --data "{\"password\":\"$PASSWORD\"}"
COOKIE="$(grep -i '^set-cookie:' "$TMP/last_headers" | head -1 | sed 's/^[Ss]et-[Cc]ookie: //' | cut -d';' -f1)"
if [[ -z "$COOKIE" || "$COOKIE" != rssea_session=* ]]; then
  echo "FAIL: no session cookie from login"
  exit 1
fi
echo "PASS: login set a session cookie"

check "session endpoint after login" 200 GET /api/session
assert_json "$TMP/last_body" 'd["authenticated"] is True'

echo "== add feed + refresh =="
FEED_URL="http://127.0.0.1:$FIXTURE_PORT/feed.xml"
check "add local fixture feed" 200 POST /api/sources \
  -H 'Content-Type: application/json' --data "{\"url\":\"$FEED_URL\",\"title\":\"Smoke Feed\"}"
FEED_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$TMP/last_body")"
echo "PASS: added feed $FEED_ID"

check "refresh all sources" 200 POST /api/sources/refresh-all

echo "== overview non-empty =="
check "overview after sync" 200 GET /api/overview
assert_json "$TMP/last_body" 'd["all"]["total_count"] >= 2'

echo "== save an article =="
check "list articles" 200 GET /api/articles
ARTICLE_ID="$(python3 -c 'import json,sys; a=json.load(open(sys.argv[1])); assert len(a) > 0, a; print(a[0]["id"])' "$TMP/last_body")"
echo "PASS: fetched article $ARTICLE_ID"

check "save the article" 200 POST "/api/articles/$ARTICLE_ID/save" \
  -H 'Content-Type: application/json' --data '{}'

echo "== saved page =="
check "saved list" 200 GET /api/saved
assert_json "$TMP/last_body" 'd["total"] >= 1'

echo "== settings round-trip =="
check "settings get" 200 GET /api/settings
check "settings update" 200 PATCH /api/settings \
  -H 'Content-Type: application/json' --data '{"theme":"dark","sync_interval_minutes":60}'
check "settings get after update" 200 GET /api/settings
assert_json "$TMP/last_body" 'd["theme"] == "dark" and d["sync_interval_minutes"] == 60'

echo "== static + SPA fallback + PWA =="
check "root serves index.html" 200 GET /
grep -q 'id="root"' "$TMP/last_body" || { echo "FAIL: / does not contain root div"; exit 1; }
echo "PASS: / contains the app root div"

check "SPA fallback for deep link" 200 GET /feeds/smoke-deep-link
grep -q 'id="root"' "$TMP/last_body" || { echo "FAIL: deep link does not serve index.html"; exit 1; }
echo "PASS: /feeds/smoke-deep-link serves the SPA shell"

check "PWA manifest served" 200 GET /manifest.webmanifest
grep -q 'icons' "$TMP/last_body" || { echo "FAIL: manifest missing icons"; exit 1; }
echo "PASS: PWA manifest served with icons"

echo "== final health =="
check "health after all flows" 200 GET /api/health

echo ""
echo "smoke test passed: release binary serves the full app standalone"
