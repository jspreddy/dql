"""Build argv and run dql / dqlrs for notebook cells and magics."""

from __future__ import annotations

import html
import json
import os
import shutil
import subprocess
from dataclasses import dataclass
from typing import Any, Mapping, Optional, Sequence

DEFAULT_REGION = "us-west-1"
DEFAULT_PORT = "8000"


class BinaryNotFoundError(FileNotFoundError):
    """The configured DQL binary is missing or not executable."""


def backend_binary_name(backend: str) -> str:
    return "dqlrs" if backend == "rust" else "dql"


def resolve_binary(
    backend: str, env: Optional[Mapping[str, str]] = None
) -> str:
    env = os.environ if env is None else env
    if backend == "rust":
        configured = (env.get("DQLRS_BIN") or "").strip()
        default = "dqlrs"
        hint = "Install dqlrs (see rust-impl/README.md) or set DQLRS_BIN."
    elif backend == "python":
        configured = (env.get("DQL_BIN") or "").strip()
        default = "dql"
        hint = "Install dql (see py-impl/README.md) or set DQL_BIN."
    else:
        raise ValueError(f"Unknown DQL backend {backend!r}")

    candidate = configured or default
    if os.path.sep in candidate or candidate.startswith("."):
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
        raise BinaryNotFoundError(f"{candidate} is not an executable. {hint}")

    found = shutil.which(candidate, path=env.get("PATH"))
    if found:
        return found
    raise BinaryNotFoundError(f"Could not find {candidate} on PATH. {hint}")


def use_json_output(env: Optional[Mapping[str, str]] = None) -> bool:
    env = os.environ if env is None else env
    value = (env.get("DQL_NOTEBOOK_JSON") or "1").strip().lower()
    return value not in {"0", "false", "no", "off"}


def build_argv(
    code: str,
    backend: str,
    *,
    env: Optional[Mapping[str, str]] = None,
) -> list[str]:
    env = os.environ if env is None else env
    argv = [resolve_binary(backend, env)]
    region = (env.get("AWS_REGION") or DEFAULT_REGION).strip() or DEFAULT_REGION
    argv.extend(["-r", region])
    host = (env.get("DQL_HOST") or "").strip()
    if host:
        port = (env.get("DQL_PORT") or DEFAULT_PORT).strip() or DEFAULT_PORT
        argv.extend(["-H", host, "-p", port])
    if use_json_output(env):
        argv.append("--json")
    argv.extend(["-c", code])
    return argv


@dataclass
class RunResult:
    argv: list[str]
    returncode: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.returncode == 0


def run_dql(
    code: str,
    backend: str,
    *,
    env: Optional[Mapping[str, str]] = None,
    timeout: Optional[float] = None,
) -> RunResult:
    merged = os.environ.copy()
    if env:
        merged.update({key: str(value) for key, value in env.items()})
    argv = build_argv(code, backend, env=merged)
    completed = subprocess.run(
        argv,
        capture_output=True,
        text=True,
        env=merged,
        timeout=timeout,
        check=False,
    )
    return RunResult(
        argv=argv,
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def try_parse_json(text: str) -> Any:
    """Parse one JSON value, or several values written back-to-back (DQL --json)."""
    stripped = text.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        pass
    decoder = json.JSONDecoder()
    values: list[Any] = []
    idx = 0
    length = len(stripped)
    while idx < length:
        while idx < length and stripped[idx].isspace():
            idx += 1
        if idx >= length:
            break
        try:
            value, end = decoder.raw_decode(stripped, idx)
        except json.JSONDecodeError:
            return None
        values.append(value)
        idx = end
    if not values:
        return None
    if len(values) == 1:
        return values[0]
    return values


def _cell(value: Any) -> str:
    if isinstance(value, (dict, list)):
        rendered = json.dumps(value, sort_keys=True)
    else:
        rendered = "" if value is None else str(value)
    return html.escape(rendered, quote=True)


def html_table(rows: Sequence[Mapping[str, Any]]) -> str:
    if not rows:
        return "<em>No items</em>"
    keys: list[str] = []
    seen: set[str] = set()
    for row in rows:
        for key in row:
            if key not in seen:
                seen.add(key)
                keys.append(str(key))
    head = "".join(f"<th>{html.escape(key, quote=True)}</th>" for key in keys)
    body_rows = []
    for row in rows:
        cells = "".join(f"<td>{_cell(row.get(key))}</td>" for key in keys)
        body_rows.append(f"<tr>{cells}</tr>")
    body = "".join(body_rows)
    return (
        '<table border="1">'
        f"<thead><tr>{head}</tr></thead>"
        f"<tbody>{body}</tbody>"
        "</table>"
    )


def format_display(stdout: str, stderr: str) -> dict[str, str]:
    """Build a Jupyter mimebundle for CLI output."""
    parts: list[str] = []
    parsed = try_parse_json(stdout)
    if parsed is not None:
        parts.append(json.dumps(parsed, indent=2, sort_keys=True))
    elif stdout:
        parts.append(stdout if stdout.endswith("\n") else stdout + "\n")
    if stderr.strip():
        if parts:
            parts.append("")
        parts.append(stderr if stderr.endswith("\n") else stderr + "\n")
    plain = "\n".join(parts).rstrip() + ("\n" if parts else "")

    bundle = {"text/plain": plain or ""}
    if isinstance(parsed, list) and all(isinstance(item, dict) for item in parsed):
        bundle["text/html"] = html_table(parsed)
    elif isinstance(parsed, dict):
        bundle["text/html"] = (
            f"<pre>{html.escape(json.dumps(parsed, indent=2, sort_keys=True))}</pre>"
        )
    return bundle
