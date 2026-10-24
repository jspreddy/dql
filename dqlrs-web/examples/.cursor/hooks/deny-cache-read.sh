#!/usr/bin/env bash
# Deny agent reads and writes of .cache and .dqlrs-web-cache.
set -euo pipefail

input=$(cat)

python3 - "$input" <<'PY'
import json
import re
import sys

CACHE = re.compile(
    r"(?:^|[\\/\s\"'`=<>|;])\.(?:dqlrs-web-cache|cache)(?=$|[\\/\s\"'`<>|;])"
)


def is_cache(text: str) -> bool:
    return bool(CACHE.search(text))


def main() -> None:
    raw = sys.argv[1] if len(sys.argv) > 1 else sys.stdin.read()
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        print(json.dumps({"permission": "allow"}))
        return

    paths: list[str] = []
    file_path = payload.get("file_path")
    if isinstance(file_path, str):
        paths.append(file_path)
    for attachment in payload.get("attachments") or []:
        if isinstance(attachment, dict):
            attached = attachment.get("file_path")
            if isinstance(attached, str):
                paths.append(attached)
    tool_input = payload.get("tool_input")
    if isinstance(tool_input, dict):
        for key, value in tool_input.items():
            if not isinstance(value, str):
                continue
            if key == "path" or key.endswith("_path") or key in ("target_directory", "file", "target_notebook"):
                paths.append(value)

    if any(is_cache(path) for path in paths):
        print(
            json.dumps(
                {
                    "permission": "deny",
                    "user_message": (
                        "Reading and writing .cache and .dqlrs-web-cache is blocked "
                        "(including create, edit, and delete). "
                        "Those folders hold the table cache and saved web query results. "
                        "Infer data from cosmos/00-table.dql and cosmos/10-bodies.dql."
                    ),
                }
            )
        )
        return

    print(json.dumps({"permission": "allow"}))


if __name__ == "__main__":
    main()
PY

exit 0
