"""Talk to a long-lived `dqlrs --serve` process."""

from __future__ import annotations

import json
import os
import re
import shlex
import shutil
import subprocess
import threading
from dataclasses import dataclass
from pathlib import Path

from statements import split_statements

REPO_ROOT = Path(__file__).resolve().parents[1]
PAGE_SIZE = 50


class EngineError(RuntimeError):
    """dqlrs could not run the request."""


class Engine:
    def __init__(self, binary: str, host: str, port: str, region: str) -> None:
        self.binary = binary
        self.host = host
        self.port = port
        self.region = region
        self._proc: subprocess.Popen[str] | None = None
        self._lock = threading.Lock()
        self._next_id = 1
        self._shapes: dict[str, TableShape] = {}

    @property
    def endpoint_label(self) -> str:
        if self.host:
            return f"{self.host}:{self.port}"
        return self.region

    def close(self) -> None:
        with self._lock:
            proc = self._proc
            self._proc = None
        if proc is None or proc.poll() is not None:
            return
        try:
            if proc.stdin:
                proc.stdin.write(json.dumps({"op": "shutdown"}) + "\n")
                proc.stdin.flush()
        except OSError:
            pass
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()

    def run_script(self, dql: str) -> list[dict]:
        results = []
        for event in self.iter_script(dql):
            if event.get("event") == "result":
                results.append(event["result"])
            if event.get("event") == "result" and not event["result"].get("ok"):
                break
        return results

    def iter_script(self, dql: str):
        """Yield statement, progress, and result events for one script.

        Progress events arrive while a statement is still running. The caller
        has to consume the generator for the engine lock to be released.
        """
        statements = split_statements(dql)
        if not statements:
            raise EngineError("nothing to run")
        with self._lock:
            proc = self._ensure_locked()
            for index, statement in enumerate(statements):
                yield {"event": "statement", "index": index}
                envelope = None
                for value in self._exec_values(proc, statement):
                    interpreted = interpret_serve_value(value)
                    if interpreted is None:
                        continue
                    if interpreted["event"] == "progress":
                        yield {
                            "event": "progress",
                            "index": index,
                            "done": interpreted["done"],
                            "total": interpreted["total"],
                            "phase": interpreted["phase"],
                        }
                        continue
                    envelope = interpreted["envelope"]
                    break
                if envelope is None:
                    raise EngineError("dqlrs --serve ended without a result")
                result = _public_result(statement, envelope)
                if result.get("ok") and result.get("kind") == "items":
                    result["columns"] = self._columns_locked(
                        proc, statement, result.get("items") or []
                    )
                if result.get("ok") and _changes_schema(statement):
                    self._shapes.clear()
                yield {"event": "result", "index": index, "result": result}
                if not result.get("ok"):
                    return

    def list_tables(self, pattern: str, *, refresh: bool = False) -> dict:
        # `ls refresh=True` rewrites dqlrs's description cache. Dropping the
        # shape cache makes the open table pick up that fresh schema.
        if refresh:
            with self._lock:
                self._shapes.clear()
        needle = (pattern or "").strip()
        command = "ls"
        if needle:
            command += " " + shlex.quote(needle)
        if refresh:
            command += " refresh=True"
        summary = self._exec(command)
        text = summary.get("message") or ""
        if not summary.get("ok"):
            message = _error_message(summary)
            if "not found" in message.lower():
                return {"tables": [], "note": ""}
            raise EngineError(message)
        tables = []
        for name in _ls_names(text):
            detail = self._exec(f"ls {shlex.quote(name)}")
            keys = []
            if detail.get("ok"):
                keys = _ls_keys(detail.get("message") or "")
            tables.append({"name": name, "keys": keys})
        return {"tables": tables, "note": _ls_note(text)}

    def table_rows(self, name: str, page: int) -> dict:
        if not _safe_table_name(name):
            raise EngineError(f"unsupported table name {name!r}")
        page = max(0, page)
        limit = PAGE_SIZE * (page + 1)
        with self._lock:
            proc = self._ensure_locked()
            envelope = self._exec_locked(proc, f"SCAN * FROM {name} LIMIT {limit}")
            if not envelope.get("ok"):
                raise EngineError(_error_message(envelope))
            items = envelope.get("items") or []
            start = PAGE_SIZE * page
            window = items[start : start + PAGE_SIZE]
            shape = self._shape_locked(proc, name)
        return {
            "name": name,
            "page": page,
            "page_size": PAGE_SIZE,
            "items": window,
            "columns": column_order(window, shape, None, None),
            "has_more": len(items) >= limit,
        }

    def _columns_locked(self, proc: subprocess.Popen[str], statement: str, rows: list) -> list[dict]:
        table, selection = read_query(statement)
        if not table:
            return column_order(rows, None, None, None)
        index = None
        try:
            explained = self._exec_locked(proc, "EXPLAIN " + statement)
        except EngineError:
            explained = None
        if explained and explained.get("ok"):
            index = explain_index(explained.get("message") or "")
        shape = self._shape_locked(proc, table)
        if index and shape is not None and index not in shape.indexes:
            self._shapes.pop(table, None)
            shape = self._shape_locked(proc, table)
        return column_order(rows, shape, index, selection)

    def _shape_locked(self, proc: subprocess.Popen[str], name: str) -> TableShape | None:
        cached = self._shapes.get(name)
        if cached is not None:
            return cached
        if not _safe_table_name(name):
            return None
        try:
            detail = self._exec_locked(proc, f"ls {name}")
        except EngineError:
            return None
        if not detail.get("ok"):
            return None
        shape = parse_table_shape(detail.get("message") or "")
        if shape is None:
            return None
        self._shapes[name] = shape
        return shape

    def _exec(self, dql: str) -> dict:
        with self._lock:
            proc = self._ensure_locked()
            return self._exec_locked(proc, dql)

    def _exec_locked(self, proc: subprocess.Popen[str], dql: str) -> dict:
        for value in self._exec_values(proc, dql):
            interpreted = interpret_serve_value(value)
            if interpreted is not None and interpreted["event"] == "envelope":
                return interpreted["envelope"]
        raise EngineError("dqlrs --serve ended without a result")

    def _ensure_locked(self) -> subprocess.Popen[str]:
        if self._proc is not None and self._proc.poll() is None:
            return self._proc
        argv = [self.binary, "--serve", "-r", self.region]
        if self.host:
            argv.extend(["-H", self.host, "-p", self.port])
        env = os.environ.copy()
        env.setdefault("AWS_EC2_METADATA_DISABLED", "true")
        proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
            bufsize=1,
        )
        self._proc = proc
        assert proc.stdin is not None
        proc.stdin.write(json.dumps({"id": "0", "op": "ping"}) + "\n")
        proc.stdin.flush()
        reply = None
        for value in self._read_values(proc):
            interpreted = interpret_serve_value(value)
            if interpreted is not None and interpreted["event"] == "envelope":
                reply = interpreted["envelope"]
                break
        if reply is None or not reply.get("ok"):
            self._proc = None
            proc.kill()
            detail = _error_message(reply) if reply else "no reply"
            raise EngineError(f"dqlrs --serve ping failed: {detail}")
        return proc

    def _exec_values(self, proc: subprocess.Popen[str], dql: str):
        request_id = str(self._next_id)
        self._next_id += 1
        assert proc.stdin is not None
        proc.stdin.write(json.dumps({"id": request_id, "op": "exec", "dql": dql}) + "\n")
        proc.stdin.flush()
        yield from self._read_values(proc)

    def _read_values(self, proc: subprocess.Popen[str]):
        stdout = proc.stdout
        if stdout is None:
            raise EngineError("dqlrs --serve has no stdout")
        while True:
            line = stdout.readline()
            if line == "":
                err = ""
                if proc.stderr:
                    err = proc.stderr.read() or ""
                raise EngineError(f"dqlrs --serve exited ({proc.poll()}): {err.strip()}")
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise EngineError(f"invalid serve line: {line!r}") from exc
            if isinstance(value, dict):
                yield value
                if "ok" in value:
                    return


def interpret_serve_value(value: object) -> dict | None:
    """Classify one dqlrs stdout object.

    Progress lines have no `ok` field. Envelopes do. Anything else is ignored.
    """
    if not isinstance(value, dict):
        return None
    if value.get("event") == "progress" and "ok" not in value:
        total = value.get("total")
        done = value.get("done")
        return {
            "event": "progress",
            "done": done if _json_int(done) else 0,
            "total": total if _json_int(total) else None,
            "phase": str(value.get("phase") or ""),
        }
    if "ok" in value:
        return {"event": "envelope", "envelope": value}
    return None


def _json_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def find_dqlrs() -> str:
    configured = (os.environ.get("DQLRS_BIN") or "").strip()
    if configured:
        if os.path.isfile(configured) and os.access(configured, os.X_OK):
            return configured
        raise EngineError(f"DQLRS_BIN is not executable: {configured}")
    found = shutil.which("dqlrs")
    if found:
        return found
    for candidate in (
        REPO_ROOT / "rust-impl" / "target" / "debug" / "dqlrs",
        REPO_ROOT / "rust-impl" / "target" / "release" / "dqlrs",
    ):
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    raise EngineError("dqlrs was not found. Set DQLRS_BIN or build rust-impl.")


def _public_result(statement: str, envelope: dict) -> dict:
    error = envelope.get("error") or {}
    return {
        "statement": statement,
        "ok": bool(envelope.get("ok")),
        "kind": envelope.get("kind"),
        "message": envelope.get("message") or error.get("message") or "",
        "affected": envelope.get("affected"),
        "items": envelope.get("items"),
        "error": error.get("message") or "",
    }


def _error_message(envelope: dict) -> str:
    error = envelope.get("error") or {}
    return error.get("message") or envelope.get("message") or "dqlrs request failed"


_INTELLIGENT_NOTE = re.compile(
    r'^No exact match for "[^"]*", so showing '
    r"(?:similar names and related keys|similar names|related keys)\.$"
)
_DESCRIBED_NAME = re.compile(r"^Name:\s+(\S+)")


def _ls_note(text: str) -> str:
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        return line if _INTELLIGENT_NOTE.match(line) else ""
    return ""


def _ls_names(text: str) -> list[str]:
    described: list[str] = []
    summary: list[str] = []
    in_summary = False
    past_header = False
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("No exact match for "):
            continue
        named = _DESCRIBED_NAME.match(line)
        if named:
            described.append(named.group(1))
            continue
        if line == "Tables":
            in_summary = True
            continue
        if not in_summary:
            continue
        if not past_header:
            if line.startswith("Name ") or line.startswith("Name\t"):
                past_header = True
            continue
        name = line.split()[0]
        if name not in {"Name", "Tables"}:
            summary.append(name)
    return described or summary


def _ls_keys(text: str) -> list[dict]:
    hash_key = ""
    range_key = ""
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("Hash Key:"):
            hash_key = line.split(":", 1)[1].strip().split(" (", 1)[0].strip()
        elif line.startswith("Range Key:"):
            range_key = line.split(":", 1)[1].strip().split(" (", 1)[0].strip()
    keys = []
    if hash_key:
        keys.append({"name": hash_key, "role": "hash"})
    if range_key:
        keys.append({"name": range_key, "role": "range"})
    return keys


@dataclass(frozen=True)
class TableShape:
    hash_key: str
    range_key: str | None
    indexes: dict[str, tuple[str, str | None]]


_READ_QUERY = re.compile(
    r"(?is)^\s*(?:select|scan)\s+(?P<select>.*?)\s+from\s+(?P<table>[A-Za-z_][A-Za-z0-9_.-]*)\b"
)
_SCHEMA_CHANGE = re.compile(r"(?i)^(create|drop|alter)\s+table\b")
_LSI_LINE = re.compile(
    r"^\s+(?P<name>\S+)\s+hash=(?P<hash>\S+)\s+range=(?P<range>\S+)\s+projection="
)
_GSI_LINE = re.compile(
    r"^\s+(?P<name>\S+)\s+\S+\s+\S+\s+\S+\s+(?P<hash>\S+)(?:\s+\([^)]+\))?\s+"
    r"(?P<range>\S+)(?:\s+\([^)]+\))?\s+\S+\s*$"
)
_EXPLAIN_INDEX = re.compile(r"""'index':\s+"([^"]+)\"""")


def parse_table_shape(text: str) -> TableShape | None:
    """Read hash, range, and index keys out of `ls <table>` detail text."""
    hash_key = ""
    range_key = None
    indexes: dict[str, tuple[str, str | None]] = {}
    section = ""
    for raw in text.splitlines():
        line = raw.rstrip()
        stripped = line.strip()
        if stripped.startswith("Hash Key:"):
            hash_key = stripped.split(":", 1)[1].strip().split(" (", 1)[0].strip()
            section = ""
            continue
        if stripped.startswith("Range Key:"):
            range_key = stripped.split(":", 1)[1].strip().split(" (", 1)[0].strip() or None
            section = ""
            continue
        if stripped == "Local Indexes:":
            section = "lsi"
            continue
        if stripped == "Global Indexes:":
            section = "gsi"
            continue
        if stripped.startswith("CREATE TABLE") or stripped.startswith("Name:"):
            section = ""
        if section == "lsi":
            match = _LSI_LINE.match(line)
            if match:
                indexes[match.group("name")] = (
                    match.group("hash"),
                    _blank_key(match.group("range")),
                )
        elif section == "gsi" and not stripped.startswith("Name "):
            match = _GSI_LINE.match(line)
            if match:
                indexes[match.group("name")] = (
                    match.group("hash"),
                    _blank_key(match.group("range")),
                )
    if not hash_key:
        return None
    return TableShape(hash_key, range_key, indexes)


def read_query(statement: str) -> tuple[str | None, list[str] | None]:
    """Return the table and selected output names.

    ``None`` for the names means ``*`` (or a statement that is not a read).
    """
    match = _READ_QUERY.match(_strip_comments(statement))
    if not match:
        return None, None
    selected = re.sub(r"(?i)^consistent\s+", "", match.group("select").strip())
    if selected == "*" or re.fullmatch(r"(?i)count\s*\(\s*\*\s*\)", selected):
        return match.group("table"), None
    names = []
    for part in _split_csv(selected):
        piece = part.strip()
        if not piece:
            continue
        alias = re.search(r"(?i)\s+AS\s+(\S+)\s*$", piece)
        names.append(_norm_ws(alias.group(1) if alias else piece))
    return match.group("table"), names


def explain_index(message: str) -> str | None:
    """Index name from an EXPLAIN schema string, ignoring the base table."""
    for name in _EXPLAIN_INDEX.findall(message or ""):
        if name not in {"TABLE", "-"}:
            return name
    return None


def column_order(
    rows: list,
    shape: TableShape | None,
    index: str | None,
    selection: list[str] | None,
) -> list[dict]:
    """Table keys, then index keys, then selection order or alphabetical.

    Each entry names the column and, when it is a key, whether that role is
    ``hash`` or ``range`` on the table and on the index used by the query.
    """
    present: list[str] = []
    seen: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            continue
        for key in row:
            if isinstance(key, str) and key not in seen:
                seen.add(key)
                present.append(key)
    ordered: list[str] = []
    roles: dict[str, dict] = {}

    def take(name: str | None) -> None:
        if name and name in seen and name not in ordered:
            ordered.append(name)

    def mark(name: str | None, kind: str, role: str) -> None:
        if not name or name not in seen:
            return
        entry = roles.setdefault(name, {"name": name, "table": None, "index": None})
        entry[kind] = role

    if shape is not None:
        mark(shape.hash_key, "table", "hash")
        take(shape.hash_key)
        mark(shape.range_key, "table", "range")
        take(shape.range_key)
        if index and index in shape.indexes:
            index_hash, index_range = shape.indexes[index]
            mark(index_hash, "index", "hash")
            take(index_hash)
            mark(index_range, "index", "range")
            take(index_range)
    if selection is None:
        rest = [key for key in present if key not in ordered]
    else:
        for name in selection:
            take(name)
        rest = [key for key in present if key not in ordered]
    rest.sort(key=lambda name: (name.casefold(), name))
    ordered.extend(rest)
    return [roles.get(name, {"name": name, "table": None, "index": None}) for name in ordered]


def _blank_key(name: str) -> str | None:
    return None if name in {"", "-"} else name


def _changes_schema(statement: str) -> bool:
    code = _strip_comments(statement).strip()
    return _SCHEMA_CHANGE.match(code) is not None


def _strip_comments(text: str) -> str:
    out: list[str] = []
    quote: str | None = None
    i = 0
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if quote is None and ch == "-" and nxt == "-":
            while i < len(text) and text[i] != "\n":
                i += 1
            continue
        if quote is not None:
            out.append(ch)
            if ch == quote and text[i - 1] != "\\":
                quote = None
            i += 1
            continue
        if ch in {"'", '"'}:
            quote = ch
        out.append(ch)
        i += 1
    return "".join(out)


def _split_csv(text: str) -> list[str]:
    parts: list[str] = []
    buf: list[str] = []
    depth = 0
    quote: str | None = None
    i = 0
    while i < len(text):
        ch = text[i]
        if quote is not None:
            buf.append(ch)
            if ch == "\\" and i + 1 < len(text):
                buf.append(text[i + 1])
                i += 2
                continue
            if ch == quote:
                quote = None
            i += 1
            continue
        if ch in {"'", '"'}:
            quote = ch
            buf.append(ch)
        elif ch == "(":
            depth += 1
            buf.append(ch)
        elif ch == ")":
            depth = max(0, depth - 1)
            buf.append(ch)
        elif ch == "," and depth == 0:
            parts.append("".join(buf))
            buf = []
        else:
            buf.append(ch)
        i += 1
    if buf:
        parts.append("".join(buf))
    return parts


def _norm_ws(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _safe_table_name(name: str) -> bool:
    if not name or any(ch.isspace() for ch in name):
        return False
    first, rest = name[0], name[1:]
    if not (first.isascii() and (first.isalpha() or first == "_")):
        return False
    return all(ch.isascii() and (ch.isalnum() or ch in {"_", "-", "."}) for ch in rest)
