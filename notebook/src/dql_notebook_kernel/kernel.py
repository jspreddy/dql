"""Jupyter wrapper kernel: cell text is DQL, executed via dql or dqlrs."""

from __future__ import annotations

import os

from ipykernel.kernelbase import Kernel

from .progress import progress_bundle
from .runner import (
    BinaryNotFoundError,
    backend_binary_name,
    format_display,
    format_envelope,
    run_dql,
    use_serve,
)
from .serve_client import ServeError, ServeSession

BANNER = (
    "DQL notebook kernel. Rust cells use `dqlrs --serve` (progress events for "
    "bulk writes); Python cells spawn `dql -c`.\n"
    "Connection: AWS_REGION, DQL_HOST, DQL_PORT, DQL_BIN / DQLRS_BIN.\n"
)


class DqlKernel(Kernel):
    implementation = "dql-notebook"
    implementation_version = "0.1.0"
    language = "dql"
    language_version = "0.6"
    language_info = {
        "name": "dql",
        "mimetype": "text/x-sql",
        "file_extension": ".dql",
        "codemirror_mode": "sql",
        "pygments_lexer": "sql",
    }
    banner = BANNER

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        backend = (os.environ.get("DQL_NOTEBOOK_BACKEND") or "python").strip()
        if backend not in {"python", "rust"}:
            backend = "python"
        self.backend = backend
        self._serve: ServeSession | None = None
        self._serve_failed = False

    def do_execute(
        self,
        code,
        silent,
        store_history=True,
        user_expressions=None,
        allow_stdin=False,
        **kwargs,
    ):
        code = (code or "").strip()
        if not code:
            return {
                "status": "ok",
                "execution_count": self.execution_count,
                "payload": [],
                "user_expressions": {},
            }

        on_progress = None if silent else self._progress_updater()
        try:
            bundle, ok, error_message = self._run_cell(code, on_progress)
        except BinaryNotFoundError as exc:
            return self._error(type(exc).__name__, str(exc), silent)
        except Exception as exc:  # noqa: BLE001 — surface unexpected runner failures
            return self._error(type(exc).__name__, str(exc), silent)

        if not silent and any(bundle.values()):
            self.send_response(
                self.iopub_socket,
                "display_data",
                {"data": bundle, "metadata": {}},
            )

        if ok:
            return {
                "status": "ok",
                "execution_count": self.execution_count,
                "payload": [],
                "user_expressions": {},
            }

        binary = backend_binary_name(self.backend)
        message = error_message or f"{binary} failed"
        return self._error("DQLError", message, silent)

    def _run_cell(self, code: str, on_progress):
        if self.backend == "rust" and use_serve() and not self._serve_failed:
            try:
                if self._serve is None:
                    self._serve = ServeSession()
                envelope = self._serve.exec_dql(code, on_progress=on_progress)
                error = envelope.get("error") or {}
                message = error.get("message") if not envelope.get("ok") else None
                return format_envelope(envelope), bool(envelope.get("ok")), message
            except ServeError:
                self._serve_failed = True
                self._serve = None
        result = run_dql(code, self.backend, on_progress=on_progress)
        bundle = format_display(result.stdout, result.stderr)
        message = (
            result.stderr.strip()
            or result.stdout.strip()
            or f"{backend_binary_name(self.backend)} exited with status {result.returncode}"
        )
        return bundle, result.ok, None if result.ok else message

    def _progress_updater(self):
        display_id = f"dql-progress-{id(self)}-{self.execution_count}"
        shown = False

        def on_progress(done: int, total, phase: str) -> None:
            nonlocal shown
            bundle = progress_bundle(done, total, phase)
            msg = "update_display_data" if shown else "display_data"
            shown = True
            self.send_response(
                self.iopub_socket,
                msg,
                {
                    "data": bundle,
                    "metadata": {},
                    "transient": {"display_id": display_id},
                },
            )

        return on_progress

    def _error(self, ename: str, evalue: str, silent: bool):
        tb = [f"{ename}: {evalue}"]
        if not silent:
            self.send_response(
                self.iopub_socket,
                "error",
                {
                    "ename": ename,
                    "evalue": evalue,
                    "traceback": tb,
                },
            )
        return {
            "status": "error",
            "execution_count": self.execution_count,
            "ename": ename,
            "evalue": evalue,
            "traceback": tb,
        }

    def do_shutdown(self, restart):
        if self._serve is not None:
            self._serve.close()
            self._serve = None
        return {"status": "ok", "restart": restart}

    def do_inspect(self, code, cursor_pos, detail_level=0, omit_detail=False):
        del code, cursor_pos, detail_level, omit_detail
        return {"status": "ok", "found": False, "data": {}, "metadata": {}}
