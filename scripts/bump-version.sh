#!/usr/bin/env bash
# Run bump2version, then refresh uv.lock and include it in the version bump commit.
set -euo pipefail

cd "$(dirname "$0")/.."

dry_run=false
no_commit=false
for arg in "$@"; do
  case "$arg" in
    --dry-run|-n) dry_run=true ;;
    --no-commit) no_commit=true ;;
  esac
done

run_bump2version() {
  if command -v bump2version >/dev/null 2>&1; then
    command bump2version "$@"
  else
    uv run bump2version "$@"
  fi
}

run_bump2version "$@"

if [[ "$dry_run" == true || "$no_commit" == true ]]; then
  exit 0
fi

uv lock
git add uv.lock
git commit --amend --no-edit
