#!/usr/bin/env python3
"""Speak MCP to `dqlrs --mcp` and check search plus query-file tools.

Uses the in-memory backend, so DynamoDB does not need to be running.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> None:
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "dqlrs")
    if not binary.is_file():
        raise SystemExit(f"dqlrs binary not found: {binary}")

    with tempfile.TemporaryDirectory(prefix="dqlrs-mcp-") as workspace:
        env = os.environ.copy()
        env["DQL_BACKEND"] = "memory"
        env.pop("AWS_ACCESS_KEY_ID", None)
        proc = subprocess.Popen(
            [str(binary), "--mcp", "--dir", workspace],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            text=True,
        )
        requests = [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "mcp-smoke", "version": "0"},
                },
            },
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "create_query_file",
                    "arguments": {
                        "path": "queries/smoke.dql",
                        "text": "SELECT * FROM mcp_smoke WHERE id = 'a';\n",
                    },
                },
            },
            {
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "read_query_file",
                    "arguments": {"path": "queries/smoke.dql"},
                },
            },
            {
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {
                    "name": "update_query_file",
                    "arguments": {
                        "path": "queries/smoke.dql",
                        "text": "SELECT status FROM mcp_smoke WHERE id = 'a';\n",
                    },
                },
            },
            {
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": {
                    "name": "global_search",
                    "arguments": {"query": "mcp_smoke"},
                },
            },
            {
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": {"name": "list_query_files", "arguments": {}},
            },
            {
                "jsonrpc": "2.0",
                "id": 8,
                "method": "tools/call",
                "params": {"name": "list_tables", "arguments": {}},
            },
        ]
        payload = "".join(json.dumps(request) + "\n" for request in requests)
        stdout, stderr = proc.communicate(payload, timeout=30)
        if proc.returncode not in (0, None):
            raise SystemExit(f"dqlrs --mcp exited {proc.returncode}\n{stderr}")
        lines = [line for line in stdout.splitlines() if line.strip()]
        responses = [json.loads(line) for line in lines]
        by_id = {response["id"]: response for response in responses if "id" in response}
        missing = [request["id"] for request in requests if "id" in request and request["id"] not in by_id]
        if missing:
            raise SystemExit(f"missing responses {missing}\n{stdout}\n{stderr}")

        names = [tool["name"] for tool in by_id[2]["result"]["tools"]]
        expect = [
            "global_search",
            "lint_query",
            "list_tables",
            "describe_tables",
            "list_query_files",
            "create_query_file",
            "read_query_file",
            "update_query_file",
        ]
        if names != expect:
            raise SystemExit(f"tools {names}")

        created = tool_json(by_id[3])
        if created.get("path") != "queries/smoke.dql":
            raise SystemExit(f"create {created}")
        read = tool_json(by_id[4])
        if "mcp_smoke" not in read.get("text", ""):
            raise SystemExit(f"read {read}")
        updated = tool_json(by_id[5])
        if "status" not in updated.get("text", ""):
            raise SystemExit(f"update {updated}")
        found = tool_json(by_id[6])
        queries = [item for item in found.get("items", []) if item.get("kind") == "query"]
        if len(queries) != 1 or queries[0].get("path") != "queries/smoke.dql":
            raise SystemExit(f"search {found}")
        files = tool_json(by_id[7])
        if files.get("files") != [{"path": "queries/smoke.dql", "name": "smoke.dql"}]:
            raise SystemExit(f"files {files}")
        listed = tool_json(by_id[8])
        if "tables" not in listed:
            raise SystemExit(f"tables {listed}")
        saved = Path(workspace, "queries", "smoke.dql").read_text(encoding="utf-8")
        if "status" not in saved:
            raise SystemExit("file on disk was not updated")
    print("mcp smoke ok")


def tool_json(response: dict) -> dict:
    if "error" in response:
        raise SystemExit(f"rpc error {response['error']}")
    result = response["result"]
    if result.get("isError"):
        raise SystemExit(f"tool error {result}")
    return json.loads(result["content"][0]["text"])


if __name__ == "__main__":
    try:
        main()
    except subprocess.TimeoutExpired:
        raise SystemExit("dqlrs --mcp timed out") from None
