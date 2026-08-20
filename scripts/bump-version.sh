#!/usr/bin/env bash
# Run bump2version, then refresh uv.lock in the bump commit.
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

sync_lockfile() {
  uv lock
}

# `uv run task bump` rebuilds the local package and may leave uv.lock modified.
sync_lockfile
if ! git diff --quiet uv.lock; then
  if git diff --quiet -- . ':!uv.lock' && git diff --cached --quiet; then
    git add uv.lock
    git commit -m "Sync lockfile with project version"
  else
    echo "error: uv.lock is out of sync; commit or stash other changes first" >&2
    exit 1
  fi
fi

tags_before=$(git tag -l | LC_ALL=C sort)

uv run bump2version "$@"

if [[ "$dry_run" == true || "$no_commit" == true ]]; then
  exit 0
fi

sync_lockfile
git add uv.lock
if git diff --cached --quiet; then
  exit 0
fi

git commit --amend --no-edit

# bump2version tags the pre-amend commit; move any new tags to the amended HEAD.
tags_after=$(git tag -l | LC_ALL=C sort)
while IFS= read -r tag; do
  [[ -n "$tag" ]] || continue
  git tag -f "$tag" HEAD
done < <(comm -13 <(printf '%s\n' "$tags_before") <(printf '%s\n' "$tags_after"))
