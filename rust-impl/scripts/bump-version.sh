#!/usr/bin/env bash
# Bump rust-impl workspace version, then refresh Cargo.lock in the bump commit.
# Uses bump2version from py-impl's uv environment (same parse/serialize rules).
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

bump2version_bin=""
if command -v bump2version >/dev/null 2>&1; then
  bump2version_bin="$(command -v bump2version)"
elif [[ -x ../py-impl/.venv/bin/bump2version ]]; then
  bump2version_bin="../py-impl/.venv/bin/bump2version"
else
  echo "error: bump2version not found; run uv sync --dev in py-impl or install bump2version" >&2
  exit 1
fi

sync_lockfile() {
  cargo generate-lockfile
}

sync_lockfile
if ! git diff --quiet Cargo.lock; then
  if git diff --quiet -- . ':!Cargo.lock' && git diff --cached --quiet; then
    git add Cargo.lock
    git commit -m "Sync Cargo.lock with workspace version"
  else
    echo "error: Cargo.lock is out of sync; commit or stash other changes first" >&2
    exit 1
  fi
fi

tags_before=$(git tag -l | LC_ALL=C sort)

"$bump2version_bin" "$@"

if [[ "$dry_run" == true || "$no_commit" == true ]]; then
  exit 0
fi

sync_lockfile
git add Cargo.lock
if git diff --cached --quiet; then
  exit 0
fi

git commit --amend --no-edit

tags_after=$(git tag -l | LC_ALL=C sort)
while IFS= read -r tag; do
  [[ -n "$tag" ]] || continue
  git tag -f "$tag" HEAD
done < <(comm -13 <(printf '%s\n' "$tags_before") <(printf '%s\n' "$tags_after"))
