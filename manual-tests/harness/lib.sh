# shellcheck shell=bash
# Shared helpers for the black-box acceptance harness. Source from run.sh.

manual_tests_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

local_host() {
  printf '%s' "${DQL_LOCAL_HOST:-localhost}"
}

local_port() {
  printf '%s' "${DQL_LOCAL_PORT:-8000}"
}

local_available() {
  local host port
  host="$(local_host)"
  port="$(local_port)"
  if command -v nc >/dev/null 2>&1; then
    nc -z "$host" "$port" >/dev/null 2>&1
    return
  fi
  python3 - "$host" "$port" <<'PY'
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
try:
    with socket.create_connection((host, port), timeout=1):
        pass
except OSError:
    sys.exit(1)
PY
}

wait_for_local() {
  local i
  for i in $(seq 1 30); do
    if local_available; then
      return 0
    fi
    sleep 1
  done
  return 1
}

start_local() {
  local root
  root="$(repo_root)"
  if local_available; then
    return 0
  fi
  if [[ ! -x "$root/scripts/install_dynamodb_local.sh" ]]; then
    echo "error: missing $root/scripts/install_dynamodb_local.sh" >&2
    return 1
  fi
  "$root/scripts/install_dynamodb_local.sh" background
  wait_for_local
}

require_local() {
  if local_available; then
    return 0
  fi
  echo "error: DynamoDB Local is not reachable at $(local_host):$(local_port)" >&2
  echo "Start it from the repository root:" >&2
  echo "  ./scripts/install_dynamodb_local.sh" >&2
  echo "Or re-run with --start-local." >&2
  return 1
}

# Print a resolved executable path or empty.
find_named_bin() {
  local name="$1"
  local env_var="$2"
  local from_env=""
  if [[ -n "${!env_var:-}" ]]; then
    from_env="${!env_var}"
    if [[ -x "$from_env" ]]; then
      printf '%s' "$from_env"
      return 0
    fi
    echo "error: $env_var is set but not executable: $from_env" >&2
    return 1
  fi
  if command -v "$name" >/dev/null 2>&1; then
    command -v "$name"
    return 0
  fi
  return 0
}

sanitize_slug() {
  local slug="$1"
  slug="${slug//\//_}"
  slug="${slug//-/_}"
  printf '%s' "$slug" | tr -cd 'A-Za-z0-9_'
}

table_name() {
  local slug="$1"
  local index="$2"
  printf 'at_%s_%s_%s' "$(sanitize_slug "$slug")" "$$" "$index"
}

# Substitute {{TABLE}} {{TABLE2}} ... in stdin.
apply_tables() {
  local slug="$1"
  python3 - "$slug" "$$" <<'PY'
import sys

slug = sys.argv[1]
pid = sys.argv[2]
text = sys.stdin.read()
safe = "".join(ch if ch.isalnum() or ch == "_" else "_" for ch in slug.replace("/", "_").replace("-", "_"))
# Longest placeholder first so TABLE10 is not eaten by TABLE1.
for n in range(20, 0, -1):
    token = "{{TABLE}}" if n == 1 else "{{TABLE%d}}" % n
    name = "at_%s_%s_%s" % (safe, pid, n)
    text = text.replace(token, name)
sys.stdout.write(text)
PY
}

used_table_indexes() {
  python3 - <<'PY'
import re
import sys

text = sys.stdin.read()
indexes = set()
for match in re.finditer(r"\{\{TABLE(\d*)\}\}", text):
    raw = match.group(1)
    indexes.add(int(raw) if raw else 1)
for n in sorted(indexes):
    print(n)
PY
}

list_case_dirs() {
  local cases_root="$1"
  local filter="${2:-}"
  local dir rel
  while IFS= read -r -d "" dir; do
    if [[ ! -f "$dir/input.dql" ]]; then
      continue
    fi
    rel="${dir#"$cases_root"/}"
    if [[ -n "$filter" && "$rel" != *"$filter"* ]]; then
      continue
    fi
    printf '%s\n' "$rel"
  done < <(find "$cases_root" -mindepth 2 -maxdepth 2 -type d -print0 | sort -z)
}
