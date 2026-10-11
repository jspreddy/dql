#!/usr/bin/env bash
# Start the bundled DQLRS web UI against the folder you pass, or this directory.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"

if [[ -z "${DQLRS_BIN:-}" ]]; then
  if [[ -x "$REPO/rust-impl/target/debug/dqlrs" ]]; then
    DQLRS_BIN="$REPO/rust-impl/target/debug/dqlrs"
  elif [[ -x "$REPO/rust-impl/target/release/dqlrs" ]]; then
    DQLRS_BIN="$REPO/rust-impl/target/release/dqlrs"
  elif command -v dqlrs >/dev/null 2>&1; then
    DQLRS_BIN="$(command -v dqlrs)"
  fi
fi

if [[ -z "${DQLRS_BIN:-}" || ! -x "$DQLRS_BIN" ]]; then
  echo "dqlrs was not found. Build rust-impl or set DQLRS_BIN." >&2
  exit 1
fi

dir="$PWD"
host="127.0.0.1"
port="8765"
aws=0
listen=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir)
      dir="${2:?--dir needs a folder}"
      shift 2
      ;;
    --host)
      host="${2:?--host needs an address}"
      shift 2
      ;;
    --port)
      port="${2:?--port needs a port}"
      shift 2
      ;;
    --listen)
      listen="${2:?--listen needs HOST:PORT}"
      shift 2
      ;;
    --aws)
      aws=1
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
export AWS_REGION="${AWS_REGION:-us-west-1}"
export AWS_EC2_METADATA_DISABLED="${AWS_EC2_METADATA_DISABLED:-true}"

args=(--web --listen "${listen:-$host:$port}" --dir "$dir" -r "$AWS_REGION")
if [[ "$aws" -eq 0 ]]; then
  args+=(-H "${DQL_HOST:-localhost}" -p "${DQL_PORT:-8000}")
fi

exec "$DQLRS_BIN" "${args[@]}"
