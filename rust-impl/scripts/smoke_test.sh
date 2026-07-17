#!/usr/bin/env bash
# Smoke-test a built `dqlrs` binary (release by default).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DQL_BIN="${DQL_BIN:-$ROOT/target/release/dqlrs}"
REQUIRE_LOCAL="${DQL_REQUIRE_LOCAL:-0}"

if [[ ! -x "$DQL_BIN" ]]; then
  echo "error: dqlrs binary not found or not executable: $DQL_BIN" >&2
  exit 1
fi

run() {
  echo "+ $*"
  "$@"
}

local_available() {
  local host="${DQL_LOCAL_HOST:-localhost}"
  local port="${DQL_LOCAL_PORT:-8000}"
  if command -v nc >/dev/null 2>&1; then
    nc -z "$host" "$port" >/dev/null 2>&1
    return
  fi
  python3 - "$host" "$port" <<'PY'
import socket, sys
host, port = sys.argv[1], int(sys.argv[2])
s = socket.socket()
s.settimeout(1)
try:
    s.connect((host, port))
except OSError:
    sys.exit(1)
finally:
    s.close()
PY
}

unique_table() {
  printf 'pkg_smoke_%s_%s' "$1" "$$"
}

echo "Smoke testing: $DQL_BIN"
run "$DQL_BIN" --version

# Offline one-shot paths use the in-memory backend (no AWS credentials required).
export DQL_BACKEND=memory

TABLE="$(unique_table mem)"
run "$DQL_BIN" -c "CREATE TABLE ${TABLE} (id STRING HASH KEY); INSERT INTO ${TABLE} (id) VALUES ('a'); SCAN * FROM ${TABLE}; DROP TABLE ${TABLE};"

TABLE_JSON="$(unique_table mem_json)"
JSON_OUT="$(
  run "$DQL_BIN" --json -c "CREATE TABLE ${TABLE_JSON} (id STRING HASH KEY); INSERT INTO ${TABLE_JSON} (id) VALUES ('a'); SCAN * FROM ${TABLE_JSON}"
)"
if [[ "$JSON_OUT" != *'"id"'* ]]; then
  echo "error: json output missing expected field" >&2
  exit 1
fi

# DynamoDB Local uses -H and does not need DQL_BACKEND=memory.
unset DQL_BACKEND

if local_available; then
  HOST="${DQL_LOCAL_HOST:-localhost}"
  PORT="${DQL_LOCAL_PORT:-8000}"
  TABLE_LOCAL="$(unique_table local)"
  run "$DQL_BIN" -H "$HOST" -p "$PORT" -c "CREATE TABLE ${TABLE_LOCAL} (id STRING HASH KEY, score NUMBER); INSERT INTO ${TABLE_LOCAL} (id, score) VALUES ('a', 1); SELECT * FROM ${TABLE_LOCAL} WHERE id = 'a'; DROP TABLE ${TABLE_LOCAL};"
elif [[ "$REQUIRE_LOCAL" == "1" || "$REQUIRE_LOCAL" == "true" ]]; then
  echo "error: DQL_REQUIRE_LOCAL is set but DynamoDB Local is not reachable" >&2
  exit 1
else
  echo "skipping: DynamoDB Local not available"
fi

echo "smoke test passed"
