"""Talk to a long-lived `dqlrs --serve` process."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import threading
from fnmatch import fnmatch
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
                yield {"event": "result", "index": index, "result": result}
                if not result.get("ok"):
                    return

    def list_tables(self, pattern: str) -> list[dict]:
        summary = self._exec("ls")
        if not summary.get("ok"):
            raise EngineError(_error_message(summary))
        names = _ls_names(summary.get("message") or "")
        needle = (pattern or "").strip()
        if needle:
            names = [name for name in names if fnmatch(name, needle)]
        tables = []
        for name in names:
            detail = self._exec(f"ls {name}")
            keys = ""
            if detail.get("ok"):
                keys = _ls_keys(detail.get("message") or "")
            tables.append({"name": name, "keys": keys})
        return tables

    def table_rows(self, name: str, page: int) -> dict:
        if not _safe_table_name(name):
            raise EngineError(f"unsupported table name {name!r}")
        page = max(0, page)
        limit = PAGE_SIZE * (page + 1)
        envelope = self._exec(f"SCAN * FROM {name} LIMIT {limit}")
        if not envelope.get("ok"):
            raise EngineError(_error_message(envelope))
        items = envelope.get("items") or []
        start = PAGE_SIZE * page
        window = items[start : start + PAGE_SIZE]
        return {
            "name": name,
            "page": page,
            "page_size": PAGE_SIZE,
            "items": window,
            "has_more": len(items) >= limit,
        }

    def _exec(self, dql: str) -> dict:
        with self._lock:
            proc = self._ensure_locked()
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


def _ls_names(text: str) -> list[str]:
    names: list[str] = []
    past_header = False
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        if not past_header:
            if line == "Tables" or line.startswith("Name "):
                if line.startswith("Name "):
                    past_header = True
                continue
            past_header = True
        name = line.split()[0]
        if name not in {"Name", "Tables"}:
            names.append(name)
    return names


def _ls_keys(text: str) -> str:
    hash_key = ""
    range_key = ""
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("Hash Key:"):
            hash_key = line.split(":", 1)[1].strip().split(" (", 1)[0].strip()
        elif line.startswith("Range Key:"):
            range_key = line.split(":", 1)[1].strip().split(" (", 1)[0].strip()
    parts = []
    if hash_key:
        parts.append(f"{hash_key} HASH")
    if range_key:
        parts.append(f"{range_key} RANGE")
    return " · ".join(parts)


def _safe_table_name(name: str) -> bool:
    if not name or any(ch.isspace() for ch in name):
        return False
    first, rest = name[0], name[1:]
    if not (first.isascii() and (first.isalpha() or first == "_")):
        return False
    return all(ch.isascii() and (ch.isalnum() or ch in {"_", "-", "."}) for ch in rest)
