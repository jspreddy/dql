"""Long-lived `dqlrs --serve` stdio client for notebook cells."""

from __future__ import annotations

import json
import os
import subprocess
import threading
from typing import Any, Callable, Mapping, Optional

from .progress import parse_progress_line
from .runner import DEFAULT_PORT, DEFAULT_REGION, BinaryNotFoundError, resolve_binary

ProgressCallback = Callable[[int, Optional[int], str], None]


class ServeError(RuntimeError):
    """A `--serve` worker failed to start or returned an error envelope."""


class ServeSession:
    """One `dqlrs --serve` child process, reused across cells."""

    def __init__(self, env: Optional[Mapping[str, str]] = None) -> None:
        self._env = os.environ.copy()
        if env:
            self._env.update({key: str(value) for key, value in env.items()})
        self._proc: Optional[subprocess.Popen[str]] = None
        self._lock = threading.Lock()
        self._next_id = 1

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
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()

    def exec_dql(
        self,
        dql: str,
        *,
        on_progress: Optional[ProgressCallback] = None,
        timeout: Optional[float] = None,
    ) -> dict[str, Any]:
        with self._lock:
            proc = self._ensure_locked()
            req_id = str(self._next_id)
            self._next_id += 1
            payload = {"id": req_id, "op": "exec", "dql": dql}
            assert proc.stdin is not None
            assert proc.stdout is not None
            proc.stdin.write(json.dumps(payload) + "\n")
            proc.stdin.flush()
            envelope = self._read_envelope_locked(proc, on_progress, timeout)
        return envelope

    def _ensure_locked(self) -> subprocess.Popen[str]:
        if self._proc is not None and self._proc.poll() is None:
            return self._proc
        binary = resolve_binary("rust", self._env)
        argv = [binary, "--serve"]
        region = (self._env.get("AWS_REGION") or DEFAULT_REGION).strip() or DEFAULT_REGION
        argv.extend(["-r", region])
        host = (self._env.get("DQL_HOST") or "").strip()
        if host:
            port = (self._env.get("DQL_PORT") or DEFAULT_PORT).strip() or DEFAULT_PORT
            argv.extend(["-H", host, "-p", port])
        try:
            proc = subprocess.Popen(
                argv,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=self._env,
                bufsize=1,
            )
        except OSError as exc:
            raise BinaryNotFoundError(str(exc)) from exc
        self._proc = proc
        ping = {"id": "0", "op": "ping"}
        assert proc.stdin is not None
        proc.stdin.write(json.dumps(ping) + "\n")
        proc.stdin.flush()
        reply = self._read_envelope_locked(proc, None, 10)
        if not reply.get("ok"):
            self.close()
            raise ServeError(f"dqlrs --serve ping failed: {reply}")
        return proc

    def _read_envelope_locked(
        self,
        proc: subprocess.Popen[str],
        on_progress: Optional[ProgressCallback],
        timeout: Optional[float],
    ) -> dict[str, Any]:
        stdout = proc.stdout
        if stdout is None:
            raise ServeError("dqlrs --serve has no stdout")
        # timeout is reserved for a future interruptible reader; serve replies
        # are line-delimited and we block on the next line.
        del timeout
        while True:
            line = stdout.readline()
            if line == "":
                stderr = ""
                if proc.stderr:
                    stderr = proc.stderr.read() or ""
                raise ServeError(
                    f"dqlrs --serve exited {proc.poll()}: {stderr.strip()}"
                )
            progress = parse_progress_line(line)
            if progress is not None:
                if on_progress is not None:
                    done = int(progress.get("done") or 0)
                    total = progress.get("total")
                    total_i = int(total) if total is not None else None
                    on_progress(done, total_i, str(progress.get("phase") or "write"))
                continue
            try:
                envelope = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ServeError(f"invalid serve line: {line!r}") from exc
            if isinstance(envelope, dict) and "ok" in envelope:
                return envelope
