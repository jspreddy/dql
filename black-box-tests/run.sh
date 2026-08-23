#!/usr/bin/env bash
# Thin wrapper: map harness flags onto pytest for the pexpect black-box suite.
set -euo pipefail

SUITE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: run.sh [--bin dql|dqlrs|both] [--start-local] [--skip-teardown] [-v|--verbose] [--group FILTER] [filter]

Runs black-box acceptance tests against DynamoDB Local.

  --bin dql|dqlrs|both  Which binary to invoke (default: both, skipping missing)
  --start-local         Start Local only if the port is down (safe if already running)
  --skip-teardown       Do not DROP TABLE after each test
  -v, --verbose         pytest -v (and later: log DQL / CLI output)
  --group FILTER        Numeric group (1xx, 11x, 2xx) or substring of the test name
  filter                Same as --group (positional, matches the old harness)

Environment:
  DQL_BIN / DQLRS_BIN         Explicit binary paths
  DQL_LOCAL_HOST / DQL_LOCAL_PORT   Default localhost:8000
  AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY   Dummy keys if unset
  NO_COLOR                    Set to disable ANSI color
EOF
}

BIN_CHOICE="both"
START_LOCAL=0
SKIP_TEARDOWN=0
VERBOSE=0
GROUP=""
PYTEST_EXTRA=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --bin)
      BIN_CHOICE="${2:-}"
      if [[ -z "$BIN_CHOICE" ]]; then
        echo "error: --bin requires dql, dqlrs, or both" >&2
        exit 2
      fi
      shift 2
      ;;
    --start-local)
      START_LOCAL=1
      shift
      ;;
    --skip-teardown)
      SKIP_TEARDOWN=1
      shift
      ;;
    -v | --verbose)
      VERBOSE=1
      shift
      ;;
    --group)
      GROUP="${2:-}"
      if [[ -z "$GROUP" ]]; then
        echo "error: --group requires a filter" >&2
        exit 2
      fi
      shift 2
      ;;
    --)
      shift
      PYTEST_EXTRA+=("$@")
      break
      ;;
    -*)
      echo "error: unknown flag $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "$GROUP" ]]; then
        echo "error: unexpected extra argument $1" >&2
        exit 2
      fi
      GROUP="$1"
      shift
      ;;
  esac
done

case "$BIN_CHOICE" in
  dql | dqlrs | both) ;;
  *)
    echo "error: --bin must be dql, dqlrs, or both (got $BIN_CHOICE)" >&2
    exit 2
    ;;
esac

pytest_args=(--bin "$BIN_CHOICE")
if [[ "$START_LOCAL" == "1" ]]; then
  pytest_args+=(--start-local)
fi
if [[ "$SKIP_TEARDOWN" == "1" ]]; then
  pytest_args+=(--skip-teardown)
fi
if [[ -n "$GROUP" ]]; then
  pytest_args+=(--group "$GROUP")
fi
if [[ "$VERBOSE" == "1" ]]; then
  pytest_args+=(-v)
fi

cd "$SUITE_ROOT"
exec uv run pytest "${pytest_args[@]}" "${PYTEST_EXTRA[@]}"
