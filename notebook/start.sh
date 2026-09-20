#!/usr/bin/env bash
# Start JupyterLab with project-local DQL kernels.
# Compatible with bash 3.2+ (macOS /bin/bash).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: start.sh [--local] [--start-local] [--no-browser] [--dry-run] [--help] [-- jupyter-args...]

Start JupyterLab from notebook/ with DQL (Python), DQL (Rust), and Python (dql) kernels.

  --local         Point DQL kernels at DynamoDB Local (localhost:8000) and
                  set dummy AWS keys if they are unset
  --start-local   Start DynamoDB Local if port 8000 is down
  --no-browser    Do not open a browser
  --dry-run       Sync the env, register kernels, list them, and exit
  --help          Show this help

Extra arguments after -- are passed to jupyter lab.

Environment:
  DQL_BIN / DQLRS_BIN     Explicit binary paths (otherwise PATH)
  AWS_REGION              Default us-west-1
  DQL_HOST / DQL_PORT     DynamoDB Local host/port when set
  DQL_NOTEBOOK_JSON       Default 1; set 0 for plain CLI text
  DQL_NOTEBOOK_SERVE      Default 1; set 0 to run rust cells with dqlrs -c
  DQL_PROGRESS_JSON       Set by kernels so Python dql emits progress JSON
  DQL_NOTEBOOK_PORT       Jupyter port (Jupyter picks a free one if busy)
  AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY
EOF
}

NO_BROWSER=0
DRY_RUN=0
USE_LOCAL=0
START_LOCAL=0
JUPYTER_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --no-browser)
      NO_BROWSER=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --local)
      USE_LOCAL=1
      shift
      ;;
    --start-local)
      START_LOCAL=1
      shift
      ;;
    --)
      shift
      JUPYTER_ARGS+=("$@")
      break
      ;;
    *)
      JUPYTER_ARGS+=("$1")
      shift
      ;;
  esac
done

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required. Install: https://docs.astral.sh/uv/" >&2
  exit 1
fi

if [[ "$START_LOCAL" -eq 1 ]]; then
  USE_LOCAL=1
  "$REPO_ROOT/scripts/install_dynamodb_local.sh" background
fi

if [[ "$USE_LOCAL" -eq 1 ]]; then
  export DQL_HOST="${DQL_HOST:-localhost}"
  export DQL_PORT="${DQL_PORT:-8000}"
  export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
  export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
fi

export AWS_REGION="${AWS_REGION:-us-west-1}"
export DQL_NOTEBOOK_JSON="${DQL_NOTEBOOK_JSON:-1}"

cd "$SCRIPT_DIR"
uv python install 3.11
uv sync --python 3.11

VENV_PY="$SCRIPT_DIR/.venv/bin/python"
if [[ ! -x "$VENV_PY" ]]; then
  echo "Expected $VENV_PY after uv sync" >&2
  exit 1
fi

# Keep Jupyter data/config inside notebook/ (do not write ~/.local kernels).
export JUPYTER_DATA_DIR="$SCRIPT_DIR/.jupyter-data"
export JUPYTER_RUNTIME_DIR="$SCRIPT_DIR/.jupyter-runtime"
export JUPYTER_CONFIG_DIR="$SCRIPT_DIR/.jupyter-config"
export JUPYTER_PATH="$SCRIPT_DIR/share/jupyter${JUPYTER_PATH:+:$JUPYTER_PATH}"

mkdir -p "$SCRIPT_DIR/share/jupyter/kernels" \
  "$JUPYTER_DATA_DIR" \
  "$JUPYTER_RUNTIME_DIR" \
  "$JUPYTER_CONFIG_DIR"

uv run --python 3.11 python -m dql_notebook_kernel.install \
  --prefix "$SCRIPT_DIR/share/jupyter" \
  --python "$VENV_PY"

uv run --python 3.11 python -m ipykernel install \
  --prefix "$SCRIPT_DIR" \
  --name python-dql \
  --display-name "Python (dql)" >/dev/null

resolve_on_path() {
  local explicit="$1"
  local name="$2"
  if [[ -n "$explicit" ]]; then
    printf '%s\n' "$explicit"
    return 0
  fi
  command -v "$name" 2>/dev/null || true
}

DQL_RESOLVED="$(resolve_on_path "${DQL_BIN:-}" dql)"
DQLRS_RESOLVED="$(resolve_on_path "${DQLRS_BIN:-}" dqlrs)"

echo "Notebook env: $SCRIPT_DIR/.venv (Python 3.11)"
if [[ -n "$DQL_RESOLVED" ]]; then
  echo "dql:    $DQL_RESOLVED"
else
  echo "dql:    not found (DQL (Python) cells will fail until dql is on PATH or DQL_BIN is set)" >&2
fi
if [[ -n "$DQLRS_RESOLVED" ]]; then
  echo "dqlrs:  $DQLRS_RESOLVED"
else
  echo "dqlrs:  not found (DQL (Rust) cells will fail until dqlrs is on PATH or DQLRS_BIN is set)" >&2
fi
if [[ -n "${DQL_HOST:-}" ]]; then
  echo "DQL endpoint: ${DQL_HOST}:${DQL_PORT:-8000}  region=${AWS_REGION}"
else
  echo "DQL endpoint: live AWS  region=${AWS_REGION}"
fi

echo "Kernels:"
uv run --python 3.11 jupyter kernelspec list

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Dry run complete."
  exit 0
fi

LAB_FLAGS=(
  --config "$SCRIPT_DIR/jupyter_server_config.py"
  --notebook-dir "$SCRIPT_DIR"
)
if [[ "$NO_BROWSER" -eq 1 ]]; then
  LAB_FLAGS+=(--no-browser)
fi
if [[ -n "${DQL_NOTEBOOK_PORT:-}" ]]; then
  LAB_FLAGS+=(--port "$DQL_NOTEBOOK_PORT")
fi

exec uv run --python 3.11 jupyter lab "${LAB_FLAGS[@]}" "${JUPYTER_ARGS[@]}"
