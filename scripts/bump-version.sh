#!/usr/bin/env bash
# Bump the Python or Rust implementation version, refreshing that tree's lockfile
# in the bump commit. Python uses bump2version (py-impl/.bumpversion.cfg); Rust uses
# cargo-release (rust-impl/release.toml).
set -euo pipefail

usage() {
  echo "usage: $0 python [bump2version args...]" >&2
  echo "       $0 rust <level|version> [--dry-run] [--no-commit] [--tag]" >&2
  echo "  python | py   bump py-impl with bump2version (uv.lock)" >&2
  echo "  rust          bump rust-impl with cargo-release (Cargo.lock)" >&2
  echo "                levels: major minor patch release rc beta alpha" >&2
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

  if ! command -v cargo-release >/dev/null 2>&1; then
    echo "error: cargo-release not found; run: cargo install cargo-release --locked" >&2
    exit 1
  fi

  # cargo-release writes the version (rust-impl/release.toml); this script commits and tags.
  local level="" tag=false
  for arg in "$@"; do
    case "$arg" in
      --dry-run|-n|--no-commit) ;;
      --tag) tag=true ;;
      -*)
        echo "error: unknown rust option '$arg'" >&2
        usage
        ;;
      *)
        if [[ -n "$level" ]]; then
          echo "error: more than one level given ('$level', '$arg')" >&2
          usage
        fi
        level="$arg"
        ;;
    esac
  done
  if [[ -z "$level" ]]; then
    echo "error: give a level (major, minor, patch, release, rc, beta, alpha) or a version" >&2
    usage
  fi

  if [[ "$dry_run" == true ]]; then
    # Without --execute, cargo-release only prints what it would change.
    cargo release version "$level"
    exit 0
  fi

  local old new
  old=$(workspace_version)
  # Writes Cargo.toml's workspace version and refreshes the workspace entries in Cargo.lock.
  cargo release version "$level" --execute --no-confirm
  new=$(workspace_version)
  if [[ "$no_commit" == true ]]; then
    exit 0
  fi
  # cargo-release cannot fill {{prev_version}} in a one-commit workspace bump, so commit
  # here. scripts/changelog.py starts a section at this subject.
  git commit -m "Bump version: $old → $new" -- Cargo.toml Cargo.lock
  # Point the README install commands at the new tag (crates/dql-cli/release.toml). Runs after
  # the commit so cargo-release sees a clean tree; pre-releases change nothing.
  cargo release replace -p dql-cli --execute --no-confirm
  git commit -q --amend --no-edit -- "$REPO_ROOT/README.md" "$REPO_ROOT/rust-docs/README.md"
  if [[ "$tag" == true ]]; then
    git tag -a "dqlrs-$new" -m "dqlrs $new"
  fi
}

# The version dql-cli inherits from [workspace.package], e.g. 0.7.10 or 0.8.0-alpha.1.
workspace_version() {
  local id
  id=$(cargo pkgid --offline -p dql-cli)
  id=${id##*#}
  echo "${id##*@}"
}

case "$language" in
  python|py) bump_python "$@" ;;
  rust) bump_rust "$@" ;;
esac
