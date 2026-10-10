"""Build argv and run dql / dqlrs for notebook cells and magics."""

from __future__ import annotations

import html
import json
import os
import shutil
import subprocess
import threading
from dataclasses import dataclass
from typing import Any, Callable, Mapping, Optional, Sequence

from .progress import parse_progress_line

DEFAULT_REGION = "us-west-1"
DEFAULT_PORT = "8000"
ProgressCallback = Callable[[int, Optional[int], str], None]


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


def use_serve(env: Optional[Mapping[str, str]] = None) -> bool:
    """Rust cells talk to `dqlrs --serve` unless DQL_NOTEBOOK_SERVE is 0."""
    env = os.environ if env is None else env
    value = (env.get("DQL_NOTEBOOK_SERVE") or "1").strip().lower()
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
    envelope: Optional[dict[str, Any]] = None

    @property
    def ok(self) -> bool:
        if self.envelope is not None:
            return bool(self.envelope.get("ok"))
        return self.returncode == 0


def run_dql(
    code: str,
    backend: str,
    *,
    env: Optional[Mapping[str, str]] = None,
    timeout: Optional[float] = None,
    on_progress: Optional[ProgressCallback] = None,
) -> RunResult:
    merged = os.environ.copy()
    if env:
        merged.update({key: str(value) for key, value in env.items()})
    merged.setdefault("DQL_PROGRESS_JSON", "1")
    argv = build_argv(code, backend, env=merged)
    if on_progress is None:
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
    return _run_dql_streaming(argv, merged, timeout, on_progress)


def _run_dql_streaming(
    argv: list[str],
    env: Mapping[str, str],
    timeout: Optional[float],
    on_progress: ProgressCallback,
) -> RunResult:
    proc = subprocess.Popen(
        argv,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=dict(env),
    )
    stderr_parts: list[str] = []

    def consume_stderr() -> None:
        assert proc.stderr is not None
        for line in proc.stderr:
            event = parse_progress_line(line)
            if event is not None:
                done = int(event.get("done") or 0)
                total = event.get("total")
                total_i = int(total) if total is not None else None
                on_progress(done, total_i, str(event.get("phase") or "write"))
            else:
                stderr_parts.append(line)

    reader = threading.Thread(target=consume_stderr, daemon=True)
    reader.start()
    assert proc.stdout is not None
    stdout = proc.stdout.read()
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    reader.join(timeout=2)
    return RunResult(
        argv=argv,
        returncode=proc.returncode if proc.returncode is not None else 1,
        stdout=stdout or "",
        stderr="".join(stderr_parts),
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


def format_envelope(envelope: Mapping[str, Any]) -> dict[str, str]:
    """Render a dqlrs --serve reply the same way as `-c --json` output."""
    kind = envelope.get("kind")
    if kind == "items":
        items = envelope.get("items") or []
        return format_display(json.dumps(items), "")
    if kind == "affected":
        count = envelope.get("affected")
        text = f"{count} affected\n"
        return {"text/plain": text}
    if kind in {"status", "schema", "text"}:
        message = envelope.get("message") or ""
        text = message if str(message).endswith("\n") else f"{message}\n"
        return {"text/plain": text} if message else {}
    if kind == "error":
        error = envelope.get("error") or {}
        message = error.get("message") or json.dumps(envelope)
        return {"text/plain": f"{message}\n"}
    return {}
