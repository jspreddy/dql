#!/usr/bin/env bash
# Run the test suite under each supported Python version (matches CI matrix).
set -euo pipefail

cd "$(dirname "$0")/../py-impl"

PYTHON_VERSIONS=(3.9 3.10 3.11)
failed=()

for version in "${PYTHON_VERSIONS[@]}"; do
  echo "=== Python ${version} ==="
  if UV_PYTHON="${version}" uv python install "${version}" \
    && UV_PYTHON="${version}" uv sync --dev -q \
    && UV_PYTHON="${version}" uv run pytest tests "$@"; then
    echo "=== Python ${version}: passed ==="
  else
    echo "=== Python ${version}: failed ===" >&2
    failed+=("${version}")
  fi
  echo
done

if ((${#failed[@]} > 0)); then
  echo "Tests failed on Python: ${failed[*]}" >&2
  exit 1
fi

echo "All Python versions passed: ${PYTHON_VERSIONS[*]}"
