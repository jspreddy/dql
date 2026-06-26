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

# `uv run task bump` rebuilds the local package and may leave uv.lock modified.
uv lock
if ! git diff --quiet uv.lock; then
  if git diff --quiet -- . ':!uv.lock' && git diff --cached --quiet; then
    git add uv.lock
    git commit -m "Sync uv.lock with pyproject.toml"
  else
    echo "error: uv.lock is out of sync; commit or stash other changes first" >&2
    exit 1
  fi
fi

uv run bump2version "$@"

if [[ "$dry_run" == true || "$no_commit" == true ]]; then
  exit 0
fi

uv lock
git add uv.lock
git commit --amend --no-edit
