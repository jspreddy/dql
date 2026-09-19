"""Jupyter wrapper kernel: cell text is DQL, executed via dql or dqlrs."""

from __future__ import annotations

import os

from ipykernel.kernelbase import Kernel

from .runner import BinaryNotFoundError, backend_binary_name, format_display, run_dql

BANNER = (
    "DQL notebook kernel. Each cell is passed to dql or dqlrs with -c.\n"
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

        try:
            result = run_dql(code, self.backend)
        except BinaryNotFoundError as exc:
            return self._error(type(exc).__name__, str(exc), silent)
        except Exception as exc:  # noqa: BLE001 — surface unexpected runner failures
            return self._error(type(exc).__name__, str(exc), silent)

        if not silent:
            bundle = format_display(result.stdout, result.stderr)
            if any(bundle.values()):
                self.send_response(
                    self.iopub_socket,
                    "display_data",
                    {"data": bundle, "metadata": {}},
                )

        if result.ok:
            return {
                "status": "ok",
                "execution_count": self.execution_count,
                "payload": [],
                "user_expressions": {},
            }

        binary = backend_binary_name(self.backend)
        message = (
            result.stderr.strip()
            or result.stdout.strip()
            or f"{binary} exited with status {result.returncode}"
        )
        return self._error("DQLError", message, silent)

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

    def do_inspect(self, code, cursor_pos, detail_level=0, omit_detail=False):
        del code, cursor_pos, detail_level, omit_detail
        return {"status": "ok", "found": False, "data": {}, "metadata": {}}
