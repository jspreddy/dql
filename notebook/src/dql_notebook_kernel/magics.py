"""IPython magics: %%dql and %%dqlrs from a Python notebook."""

from __future__ import annotations

from IPython.core.magic import Magics, cell_magic, magics_class
from IPython.display import HTML, display

from .progress import progress_html
from .runner import (
    backend_binary_name,
    format_display,
    format_envelope,
    run_dql,
    use_serve,
)
from .serve_client import ServeError, ServeSession

_SERVE: ServeSession | None = None
_SERVE_FAILED = False


@magics_class
class DqlMagics(Magics):
    @cell_magic
    def dql(self, line, cell):
        """Run the cell with the Python dql binary."""
        del line
        return _run_magic("python", cell)

    @cell_magic
    def dqlrs(self, line, cell):
        """Run the cell with the Rust dqlrs binary."""
        del line
        return _run_magic("rust", cell)


def _progress_updater():
    handle = {"display": None}

    def on_progress(done: int, total, phase: str) -> None:
        markup = progress_html(done, total, phase)
        if handle["display"] is None:
            handle["display"] = display(HTML(markup), display_id=True)
        else:
            handle["display"].update(HTML(markup))

    return on_progress


def _run_magic(backend: str, cell: str):
    code = (cell or "").strip()
    if not code:
        return None
    on_progress = _progress_updater()
    if backend == "rust" and use_serve():
        global _SERVE, _SERVE_FAILED
        if not _SERVE_FAILED:
            try:
                if _SERVE is None:
                    _SERVE = ServeSession()
                envelope = _SERVE.exec_dql(code, on_progress=on_progress)
                bundle = format_envelope(envelope)
                _show_bundle(bundle)
                if not envelope.get("ok"):
                    error = envelope.get("error") or {}
                    raise RuntimeError(error.get("message") or "dqlrs --serve exec failed")
                return None
            except ServeError:
                _SERVE_FAILED = True
                _SERVE = None
    result = run_dql(code, backend, on_progress=on_progress)
    bundle = format_display(result.stdout, result.stderr)
    _show_bundle(bundle)
    if not result.ok:
        binary = backend_binary_name(backend)
        raise RuntimeError(
            f"{binary} exited with status {result.returncode}"
        )
    return None


def _show_bundle(bundle: dict[str, str]) -> None:
    if bundle.get("text/html"):
        display(HTML(bundle["text/html"]))
    elif bundle.get("text/plain"):
        print(bundle["text/plain"], end="")


def load_ipython_extension(ipython) -> None:
    ipython.register_magics(DqlMagics)
