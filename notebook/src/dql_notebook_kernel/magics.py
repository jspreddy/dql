"""IPython magics: %%dql and %%dqlrs from a Python notebook."""

from __future__ import annotations

from IPython.core.magic import Magics, cell_magic, magics_class
from IPython.display import HTML, display

from .runner import backend_binary_name, format_display, run_dql


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


def _run_magic(backend: str, cell: str):
    code = (cell or "").strip()
    if not code:
        return None
    result = run_dql(code, backend)
    bundle = format_display(result.stdout, result.stderr)
    if bundle.get("text/html"):
        display(HTML(bundle["text/html"]))
    elif bundle.get("text/plain"):
        print(bundle["text/plain"], end="")
    if not result.ok:
        binary = backend_binary_name(backend)
        raise RuntimeError(
            f"{binary} exited with status {result.returncode}"
        )
    return None


def load_ipython_extension(ipython) -> None:
    ipython.register_magics(DqlMagics)
