#!/usr/bin/env bash
# Examples workspace: block dqlrs/dql CLI, local web HTTP, and cache folder access.
set -euo pipefail

input=$(cat)

python3 - "$input" <<'PY'
import json
import re
import sys

DENY = {
    "permission": "deny",
    "user_message": (
        "Blocked in the examples workspace: do not run the dqlrs or dql CLI from the shell, "
        "and do not call the local dqlrs web server (127.0.0.1, localhost, or ::1 on any port, "
        "including /api/run, /mcp, and other routes) with curl, wget, or similar. "
        "Write DQL anywhere under this workspace and use MCP tools (e.g. lint_query_file) when you need lint."
    ),
    "agent_message": (
        "Shell execution of dqlrs/dql is not allowed anywhere in the examples workspace. "
        "Direct HTTP to the local dqlrs web server is blocked, including /api/run and any other path or port. "
        "Use MCP tools instead. Write queries only; infer data from the schema and load scripts in this workspace."
    ),
}


def main() -> None:
    raw = sys.argv[1] if len(sys.argv) > 1 else sys.stdin.read()
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        print(json.dumps({"permission": "allow"}))
        return

    command = payload.get("command") or ""
    if not isinstance(command, str) or not command.strip():
        print(json.dumps({"permission": "allow"}))
        return

    cmd = command.strip()

    if re.search(r"(?i)(?:^|[\s;&|(])(?:\./)?(?:dqlrs|dql)(?:\s|$|[;&|)])", cmd):
        print(json.dumps(DENY))
        return

    http_client = re.search(
        r"(?i)\b(curl|wget|fetch|httpie|httpx|nc|netcat|telnet)\b",
        cmd,
    )
    # Any path and any port on the loopback dqlrs web server (8765 or otherwise).
    local_web = re.search(
        r"(?i)(?:https?://)?(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?(?:/[^\s\"']*)?",
        cmd,
    ) or re.search(
        r"(?i)(?:^|[\s\"'=])::1(?:\s+\d+|:\d+)?(?:/[^\s\"']*)?",
        cmd,
    )
    if http_client and local_web:
        print(json.dumps(DENY))
        return

    # Any mention of the cache dirs, including create, modify, delete, append, and redirects.
    if re.search(
        r"(?:^|[\\/\s\"'`=<>|;])\.(?:dqlrs-web-cache|cache)(?=$|[\\/\s\"'`<>|;])",
        cmd,
    ):
        print(
            json.dumps(
                {
                    "permission": "deny",
                    "user_message": (
                        "Shell access to .cache and .dqlrs-web-cache is blocked, "
                        "including reads and writes (create, modify, delete, append, and redirect). "
                        "Those folders hold the table cache and saved web query results."
                    ),
                    "agent_message": (
                        "Do not read or write .cache or .dqlrs-web-cache from the shell anywhere in the examples workspace. "
                        "Infer data from the schema and load scripts checked in under this workspace."
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
