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


_HELP_KEYWORDS = {
    "SELECT", "SCAN", "INSERT", "UPDATE", "CREATE", "ALTER", "DUMP", "LOAD", "EXPLAIN",
    "ANALYZE", "SHOW", "LIKE", "DESCRIBE", "FROM", "WHERE", "INTO", "VALUES", "SET", "ADD",
    "REMOVE", "USING", "LIMIT", "ORDER", "BY", "ASC", "DESC", "AS", "KEYS", "IN", "CONSISTENT",
    "THROTTLE", "RETURNS", "SAVE", "PARENTS", "TABLE", "INDEX", "GLOBAL", "HASH", "RANGE",
    "KEY", "THROUGHPUT", "TP", "IF", "NOT", "EXISTS", "INCLUDE", "ALL", "SCHEMA", "AND", "OR",
    "BETWEEN", "NONE", "UPDATED", "OLD", "NEW", "LS", "OPT", "OPTIONS", "HELP", "USE", "WATCH",
}
_HELP_DELETED = {"DROP", "DELETE"}
_HELP_TYPES = {"STRING", "NUMBER", "BINARY", "BOOL", "BOOLEAN"}
_HELP_FUNCS = {
    "COUNT", "SIZE", "BEGINS_WITH", "CONTAINS", "ATTRIBUTE_EXISTS", "ATTRIBUTE_NOT_EXISTS",
    "ATTRIBUTE_TYPE", "IF_NOT_EXISTS", "LIST_APPEND", "TIMESTAMP", "TS", "NOW", "UTCNOW",
}
_HELP_TONE = {
    "keyword": "color:#0f766e;font-weight:650",
    "deleted": "color:#dc2626;font-weight:700",
    "type": "color:#1d4ed8",
    "func": "color:#7c3aed",
    "string": "color:#be123c",
    "number": "color:#b45309",
    "comment": "color:#6b7280;font-style:italic",
    "punct": "color:#64748b",
}


def _help_tone(word: str) -> str | None:
    key = word.upper()
    if key in _HELP_DELETED:
        return "deleted"
    if key in _HELP_KEYWORDS:
        return "keyword"
    if key in _HELP_TYPES:
        return "type"
    if key in _HELP_FUNCS:
        return "func"
    if key in {"TRUE", "FALSE", "NULL"}:
        return "keyword"
    return None


def highlight_dql_html(text: str) -> str:
    """Paint one DQL snippet with inline colors for a notebook output."""
    out: list[str] = []
    i = 0
    raw = text or ""
    while i < len(raw):
        ch = raw[i]
        if ch == "-" and raw[i : i + 2] == "--":
            out.append(f'<span style="{_HELP_TONE["comment"]}">{html.escape(raw[i:])}</span>')
            break
        if ch in {"'", '"'}:
            quote = ch
            j = i + 1
            while j < len(raw):
                if raw[j] == "\\" and j + 1 < len(raw):
                    j += 2
                    continue
                if raw[j] == quote:
                    j += 1
                    break
                j += 1
            out.append(f'<span style="{_HELP_TONE["string"]}">{html.escape(raw[i:j])}</span>')
            i = j
            continue
        if ch.isdigit():
            j = i + 1
            while j < len(raw) and (raw[j].isdigit() or raw[j] == "."):
                j += 1
            out.append(f'<span style="{_HELP_TONE["number"]}">{html.escape(raw[i:j])}</span>')
            i = j
            continue
        if ch.isalpha() or ch == "_":
            j = i + 1
            while j < len(raw) and (raw[j].isalnum() or raw[j] == "_"):
                j += 1
            word = raw[i:j]
            tone = _help_tone(word)
            escaped = html.escape(word)
            if tone:
                out.append(f'<span style="{_HELP_TONE[tone]}">{escaped}</span>')
            else:
                out.append(escaped)
            i = j
            continue
        if ch in "=<>!,()*+-/%;.|[]":
            out.append(f'<span style="{_HELP_TONE["punct"]}">{html.escape(ch)}</span>')
            i += 1
            continue
        out.append(html.escape(ch))
        i += 1
    return "".join(out)


def help_html(page: Mapping[str, Any]) -> str:
    """Notebook card for a HELP query. Colors are inline so Jupyter can show them."""
    title = html.escape(str(page.get("title") or "HELP"))
    parts = [
        '<div style="font-family:system-ui,sans-serif;line-height:1.45;color:#1f2937">',
        f'<div style="font-size:18px;font-weight:700;color:#0f766e;margin:0 0 10px">{title}</div>',
    ]
    for block in page.get("blocks") or []:
        kind = block.get("kind")
        if kind == "heading":
            text = html.escape(str(block.get("text") or ""))
            parts.append(
                f'<div style="margin:14px 0 6px;font-size:12px;letter-spacing:0.04em;'
                f'text-transform:uppercase;color:#64748b">{text}</div>'
            )
        elif kind == "code":
            code = highlight_dql_html(str(block.get("text") or ""))
            parts.append(
                '<pre style="margin:0 0 10px;padding:10px 12px;background:#f8fafc;'
                'border:1px solid #e2e8f0;border-radius:8px;font-family:ui-monospace,monospace;'
                f'font-size:13px;line-height:1.5;white-space:pre-wrap">{code}</pre>'
            )
        elif kind == "entry":
            name = html.escape(str(block.get("name") or ""))
            summary = html.escape(str(block.get("summary") or ""))
            parts.append(
                '<div style="display:grid;grid-template-columns:7.5rem 1fr;gap:8px;padding:2px 0">'
                f'<span style="font-family:ui-monospace,monospace;font-weight:650;color:#0f766e">{name}</span>'
                f"<span>{summary}</span></div>"
            )
        else:
            text = html.escape(str(block.get("text") or ""))
            parts.append(f'<p style="margin:0 0 8px;white-space:pre-wrap">{text}</p>')
    parts.append("</div>")
    return "".join(parts)


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
    if kind == "help":
        message = envelope.get("message") or ""
        text = message if str(message).endswith("\n") else f"{message}\n"
        return {
            "text/plain": text,
            "text/html": help_html(envelope.get("help") or {}),
        }
    if kind in {"status", "schema", "text"}:
        message = envelope.get("message") or ""
        text = message if str(message).endswith("\n") else f"{message}\n"
        return {"text/plain": text} if message else {}
    if kind == "error":
        error = envelope.get("error") or {}
        message = error.get("message") or json.dumps(envelope)
        return {"text/plain": f"{message}\n"}
    return {}
