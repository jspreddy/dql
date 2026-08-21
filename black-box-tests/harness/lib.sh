# shellcheck shell=bash
# Shared helpers for the black-box acceptance harness. Source from run.sh.

HARNESS_VERBOSE="${HARNESS_VERBOSE:-0}"
HARNESS_INDENT="${HARNESS_INDENT:-0}"

harness_color_enabled() {
  [[ -z "${NO_COLOR:-}" ]]
}

harness_pad() {
  printf '%*s' "${HARNESS_INDENT:-0}" ''
}

# Paint text with an SGR code (e.g. 1, 2, 31, 1;32). No trailing newline.
harness_paint() {
  local code="$1"
  shift
  if harness_color_enabled; then
    printf '\033[%sm%s\033[0m' "$code" "$*"
  else
    printf '%s' "$*"
  fi
}

# Bold yellow heading, three blank lines above, dashed line above and below.
harness_heading() {
  local text="$1"
  local n=${#text}
  local line
  if [[ "$n" -lt 3 ]]; then
    n=3
  fi
  line="$(printf '%*s' "$n" '' | tr ' ' '-')"
  printf '\n\n\n%s\n%s\n%s\n' "$(harness_paint 33 "$line")" "$(harness_paint '1;33' "$text")" "$(harness_paint 33 "$line")"
}

# Bold white subheading, blank line above.
harness_subheading() {
  printf '\n%s%s\n' "$(harness_pad)" "$(harness_paint '1;37' "$1")"
}

# Bold magenta subheading for a binary section (dql / dqlrs).
harness_bin_heading() {
  printf '\n%s\n' "$(harness_paint '1;35' "$1")"
}

harness_verbose_subheading() {
  [[ "${HARNESS_VERBOSE:-0}" == "1" ]] || return 0
  harness_subheading "$1"
}

harness_filename() {
  harness_paint 34 "$1"
}

harness_info() {
  printf '  %s\n' "$*"
}

harness_kv() {
  local key="$1"
  local value="$2"
  printf '  %s  %s\n' "$(harness_paint 2 "$(printf '%-8s' "$key")")" "$(harness_paint 1 "$value")"
}

harness_error() {
  printf '%s %s\n' "$(harness_paint '1;31' "error:")" "$*" >&2
}

harness_warn() {
  printf '%s %s\n' "$(harness_paint '1;33' "warning:")" "$*" >&2
}

harness_verbose() {
  [[ "${HARNESS_VERBOSE:-0}" == "1" ]] || return 0
  printf '  %s %s\n' "$(harness_paint 2 "·")" "$(harness_paint 2 "$*")"
}

# Dump a file under a blue filename label. Missing or empty files are noted.
harness_verbose_file() {
  local title="$1"
  local file="${2:-}"
  [[ "${HARNESS_VERBOSE:-0}" == "1" ]] || return 0
  printf '\n%s%s\n' "$(harness_pad)" "$(harness_filename "$title")"
  if [[ -z "$file" || ! -e "$file" ]]; then
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "(none)")"
    return 0
  fi
  if [[ ! -s "$file" ]]; then
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "(empty)")"
    return 0
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "$line")"
  done <"$file"
}

# Dump file contents in verbose mode with no title (caller prints the heading).
harness_verbose_dump() {
  local file="${1:-}"
  [[ "${HARNESS_VERBOSE:-0}" == "1" ]] || return 0
  if [[ -z "$file" || ! -e "$file" ]]; then
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "(none)")"
    return 0
  fi
  if [[ ! -s "$file" ]]; then
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "(empty)")"
    return 0
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s      %s\n' "$(harness_pad)" "$(harness_paint 2 "$line")"
  done <"$file"
}

# In -v, omit empty stderr when exit is 0. Non-zero exit always shows the section.
harness_verbose_stderr() {
  local title="$1"
  local file="$2"
  local exit_code="${3:-0}"
  [[ "${HARNESS_VERBOSE:-0}" == "1" ]] || return 0
  if [[ "$exit_code" == "0" && ( -z "$file" || ! -s "$file" ) ]]; then
    return 0
  fi
  harness_subheading "$title"
  harness_verbose_dump "$file"
}

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
  python3 -c '
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
tried = []
for candidate in (host, "127.0.0.1", "localhost"):
    if candidate in tried:
        continue
    tried.append(candidate)
    try:
        with socket.create_connection((candidate, port), timeout=1):
            sys.exit(0)
    except OSError:
        continue
sys.exit(1)
' "$(local_host)" "$(local_port)"
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
    harness_info "DynamoDB Local already running at $(local_host):$(local_port)"
    return 0
  fi
  if [[ ! -x "$root/scripts/install_dynamodb_local.sh" ]]; then
    harness_error "missing $root/scripts/install_dynamodb_local.sh"
    return 1
  fi
  harness_info "Starting DynamoDB Local via $root/scripts/install_dynamodb_local.sh"
  "$root/scripts/install_dynamodb_local.sh" background
  if wait_for_local; then
    return 0
  fi
  harness_error "DynamoDB Local did not become reachable at $(local_host):$(local_port)"
  return 1
}

require_local() {
  if local_available; then
    return 0
  fi
  harness_error "DynamoDB Local is not reachable at $(local_host):$(local_port)"
  printf '  Start it from the repository root:\n' >&2
  printf '    ./scripts/install_dynamodb_local.sh\n' >&2
  printf '  Or re-run with --start-local.\n' >&2
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
    harness_error "$env_var is set but not executable: $from_env"
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

# Substitute {{TABLE}} {{TABLE2}} ... on stdin; write to stdout.
apply_tables() {
  local slug="$1"
  python3 -c '
import sys
slug = sys.argv[1]
pid = sys.argv[2]
text = sys.stdin.read()
safe = "".join(ch if ch.isalnum() or ch == "_" else "_" for ch in slug.replace("/", "_").replace("-", "_"))
for n in range(20, 0, -1):
    token = "{{TABLE}}" if n == 1 else "{{TABLE%d}}" % n
    name = "at_%s_%s_%s" % (safe, pid, n)
    text = text.replace(token, name)
sys.stdout.write(text)
' "$slug" "$$"
}

used_table_indexes() {
  python3 -c '
import re
import sys
text = sys.stdin.read()
indexes = set()
for match in re.finditer(r"\{\{TABLE(\d*)\}\}", text):
    raw = match.group(1)
    indexes.add(int(raw) if raw else 1)
for n in sorted(indexes):
    print(n)
'
}

# List case directory names under cases_root, one per line, sorted.
# Filter: a pattern of digits and x/X (e.g. 1xx, 11x, 2xx) matches the
# leading number; any other string is a substring of the folder name.
list_case_dirs() {
  local cases_root="$1"
  local filter="${2:-}"
  python3 -c '
import os
import re
import sys

root, filt = sys.argv[1], sys.argv[2]
names = []
for name in os.listdir(root):
    path = os.path.join(root, name)
    if not os.path.isdir(path):
        continue
    if not os.path.isfile(os.path.join(path, "input.dql")):
        continue
    names.append(name)
names.sort()
if filt:
    if re.fullmatch(r"[0-9xX]+", filt):
        pat = "".join("[0-9]" if c in "xX" else re.escape(c) for c in filt)
        rx = re.compile(pat)
        names = [n for n in names if rx.match(n)]
    else:
        names = [n for n in names if filt in n]
for name in names:
    print(name)
' "$cases_root" "$filter"
}
