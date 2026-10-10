#!/usr/bin/env bash
# Install Playwright and run the DQL notebook UI tests.
# Compatible with bash 3.2+ (macOS /bin/bash). Empty arrays must not be
# expanded under `set -u`; that is an unbound-variable error on bash 3.2.
set -euo pipefail

SUITE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SUITE_ROOT/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: run.sh [--start-local] [--headed] [--slowmo MS] [--debug] [--list] [--skip-install] [pytest-args...]

Playwright tests for the DQL JupyterLab notebook. They open Lab, check the
DQL kernels on the launcher, and run a DQL (Rust) notebook against DynamoDB Local.

  --start-local    Start DynamoDB Local if port 8000 is down
  --headed         Show the browser window
  --slowmo MS      Pause MS milliseconds between Playwright actions (implies --headed)
  --debug          Open the Playwright inspector and pause (implies --headed)
  --list           Print test node ids and exit
  --skip-install   Do not sync envs or download the Playwright browser
  --               Remaining arguments are passed to pytest

Pass a pytest node id or -k expression to run one test:

  ./notebook-tests/run.sh --headed --skip-install -- tests/test_dql_notebook.py::test_launcher_lists_dql_kernels
  ./notebook-tests/run.sh --headed --skip-install -- tests/test_dql_notebook.py::test_rust_kernel_runs_select

Environment:
  DQL_BIN / DQLRS_BIN              Explicit binaries (debug/release used otherwise)
  DQL_LOCAL_HOST / DQL_LOCAL_PORT  DynamoDB Local, default localhost:8000
  AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY   Dummy keys if unset
EOF
}

START_LOCAL=0
HEADED=0
SKIP_INSTALL=0
LIST_ONLY=0
SLOWMO=""
PYTEST_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --start-local)
      START_LOCAL=1
      shift
      ;;
    --headed)
      HEADED=1
      shift
      ;;
    --slowmo)
      SLOWMO="${2:-}"
      if [[ -z "$SLOWMO" ]]; then
        echo "error: --slowmo requires milliseconds" >&2
        exit 2
      fi
      HEADED=1
      shift 2
      ;;
    --debug)
      HEADED=1
      export PWDEBUG=1
      shift
      ;;
    --list)
      LIST_ONLY=1
      shift
      ;;
    --skip-install)
      SKIP_INSTALL=1
      shift
      ;;
    --)
      shift
      PYTEST_ARGS+=("$@")
      break
      ;;
    *)
      PYTEST_ARGS+=("$1")
      shift
      ;;
  esac
done

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required. Install: https://docs.astral.sh/uv/" >&2
  exit 1
fi

chrome_installed() {
  if command -v google-chrome >/dev/null 2>&1 || command -v google-chrome-stable >/dev/null 2>&1; then
    return 0
  fi
  [[ -d "/Applications/Google Chrome.app" ]]
}

if [[ "$LIST_ONLY" -eq 1 ]]; then
  cd "$SUITE_ROOT"
  if [[ "$SKIP_INSTALL" -eq 0 ]]; then
    uv sync
  fi
  exec uv run pytest --collect-only -q
fi

if [[ "$START_LOCAL" -eq 1 ]]; then
  "$REPO_ROOT/scripts/install_dynamodb_local.sh" background
fi

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
export AWS_REGION="${AWS_REGION:-us-west-1}"
export AWS_EC2_METADATA_DISABLED="${AWS_EC2_METADATA_DISABLED:-true}"

if [[ -z "${DQLRS_BIN:-}" ]]; then
  if [[ -x "$REPO_ROOT/rust-impl/target/debug/dqlrs" ]]; then
    export DQLRS_BIN="$REPO_ROOT/rust-impl/target/debug/dqlrs"
  elif [[ -x "$REPO_ROOT/rust-impl/target/release/dqlrs" ]]; then
    export DQLRS_BIN="$REPO_ROOT/rust-impl/target/release/dqlrs"
  fi
fi
if [[ -z "${DQL_BIN:-}" && -x "$REPO_ROOT/py-impl/.venv/bin/dql" ]]; then
  export DQL_BIN="$REPO_ROOT/py-impl/.venv/bin/dql"
fi

if [[ -n "${DQLRS_BIN:-}" ]] && ! "$DQLRS_BIN" --help 2>&1 | grep -q -- '--serve'; then
  echo "error: $DQLRS_BIN does not support --serve." >&2
  echo "Rebuild from rust-impl: cargo build -p dql-cli" >&2
  exit 1
fi

# Prefer installed Google Chrome. Playwright's CDN is blocked in some environments.
BROWSER_ARGS=()
if chrome_installed; then
  BROWSER_ARGS=(--browser-channel chrome)
fi

if [[ "$SKIP_INSTALL" -eq 0 ]]; then
  "$REPO_ROOT/notebook/start.sh" --dry-run --local
  cd "$SUITE_ROOT"
  uv sync
  if [[ ${#BROWSER_ARGS[@]} -eq 0 ]]; then
    uv run playwright install chromium
  else
    echo "Using installed Google Chrome (Playwright --browser-channel chrome)."
  fi
elif [[ ${#BROWSER_ARGS[@]} -eq 0 ]]; then
  echo "Google Chrome was not found. Install it, or run without --skip-install so Playwright can download Chromium." >&2
  exit 1
fi

cd "$SUITE_ROOT"
if [[ "$HEADED" -eq 1 ]]; then
  PYTEST_ARGS+=(--headed)
fi
if [[ -n "$SLOWMO" ]]; then
  PYTEST_ARGS+=(--slowmo "$SLOWMO")
fi

if [[ -n "${DQLRS_BIN:-}" ]]; then
  echo "dqlrs: $DQLRS_BIN"
fi
if [[ "$HEADED" -eq 1 ]]; then
  echo "Playwright will show the browser window."
fi

# bash 3.2 + set -u treats "${empty[@]}" as an unbound variable.
CMD=(uv run pytest)
if [[ ${#BROWSER_ARGS[@]} -gt 0 ]]; then
  CMD=("${CMD[@]}" "${BROWSER_ARGS[@]}")
fi
if [[ ${#PYTEST_ARGS[@]} -gt 0 ]]; then
  CMD=("${CMD[@]}" "${PYTEST_ARGS[@]}")
fi
exec "${CMD[@]}"
