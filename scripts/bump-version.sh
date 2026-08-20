#!/usr/bin/env bash
# Bump python or rust implementation version, then refresh that tree's lockfile
# in the bump commit.
set -euo pipefail

usage() {
  echo "usage: $0 <python|rust> [bump2version args...]" >&2
  echo "  python | py   bump py-impl (uv.lock)" >&2
  echo "  rust          bump rust-impl (Cargo.lock)" >&2
  exit 1
}

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ $# -lt 1 ]]; then
  usage
fi

language="$1"
shift

case "$language" in
  python|py) ;;
  rust) ;;
  --help|-h) usage ;;
  *)
    echo "error: unknown language '$language'" >&2
    usage
    ;;
esac

dry_run=false
no_commit=false
for arg in "$@"; do
  case "$arg" in
    --dry-run|-n) dry_run=true ;;
    --no-commit) no_commit=true ;;
  esac
done

retarget_new_tags() {
  local tags_before="$1"
  local tags_after
  tags_after=$(git tag -l | LC_ALL=C sort)
  while IFS= read -r tag; do
    [[ -n "$tag" ]] || continue
    git tag -f "$tag" HEAD
  done < <(comm -13 <(printf '%s\n' "$tags_before") <(printf '%s\n' "$tags_after"))
}

bump_python() {
  cd "$REPO_ROOT/py-impl"

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

  local tags_before
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
  retarget_new_tags "$tags_before"
}

bump_rust() {
  cd "$REPO_ROOT/rust-impl"

  local bump2version_bin=""
  if command -v bump2version >/dev/null 2>&1; then
    bump2version_bin="$(command -v bump2version)"
  elif [[ -x "$REPO_ROOT/py-impl/.venv/bin/bump2version" ]]; then
    bump2version_bin="$REPO_ROOT/py-impl/.venv/bin/bump2version"
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

  local tags_before
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
  retarget_new_tags "$tags_before"
}

case "$language" in
  python|py) bump_python "$@" ;;
  rust) bump_rust "$@" ;;
esac
