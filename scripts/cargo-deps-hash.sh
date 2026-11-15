#!/usr/bin/env bash
# Print a sha256 of rust-impl/Cargo.lock without the workspace's own crates, for CI cache keys.
#
# Every version bump rewrites the workspace crates' entries (dql-cli 0.7.11 → 0.7.12), so a key on
# the whole lockfile would change on each release. Only packages that come from a registry or git
# carry a `source =` line; keeping just those blocks hashes the dependencies alone.
set -euo pipefail
cd "$(dirname "$0")/../rust-impl"
deps=$(awk '
  /^\[\[package\]\]$/ { if (block != "" && keep) printf "%s", block; block = ""; keep = 0 }
  /^source = /        { keep = 1 }
  { block = block $0 "\n" }
  END { if (block != "" && keep) printf "%s", block }
' Cargo.lock)
# Nothing kept means the lockfile format changed under this script. Fail rather than print the
# same hash every run, which would quietly freeze the caches.
count=$(grep -c '^\[\[package\]\]$' <<<"$deps" || true)
if [[ "$count" -eq 0 ]]; then
  echo "error: no dependencies with a source in Cargo.lock; update scripts/cargo-deps-hash.sh" >&2
  exit 1
fi
printf '%s\n' "$deps" | sha256sum | cut -d' ' -f1
