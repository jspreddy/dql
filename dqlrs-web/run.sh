#!/usr/bin/env bash
# Start DQLRS Web against the folder you pass, or this directory.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"

if [[ -z "${DQLRS_BIN:-}" ]]; then
  if [[ -x "$REPO/rust-impl/target/debug/dqlrs" ]]; then
    export DQLRS_BIN="$REPO/rust-impl/target/debug/dqlrs"
  elif [[ -x "$REPO/rust-impl/target/release/dqlrs" ]]; then
    export DQLRS_BIN="$REPO/rust-impl/target/release/dqlrs"
  fi
fi

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
export AWS_REGION="${AWS_REGION:-us-west-1}"
export AWS_EC2_METADATA_DISABLED="${AWS_EC2_METADATA_DISABLED:-true}"
export DQL_HOST="${DQL_HOST:-localhost}"
export DQL_PORT="${DQL_PORT:-8000}"

cd "$ROOT"
exec python3 server.py "$@"
