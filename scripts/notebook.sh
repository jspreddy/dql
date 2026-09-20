#!/usr/bin/env bash
# Thin wrapper so maintainers can start the notebook UI from scripts/.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/../notebook/start.sh" "$@"
