"""Project-local JupyterLab config. Bound to localhost only."""

from pathlib import Path

# jupyter_server injects get_config() when loading this file.
c = get_config()  # noqa: F821

HERE = Path(__file__).resolve().parent

c.ServerApp.ip = "127.0.0.1"
c.ServerApp.allow_remote_access = False
c.ServerApp.root_dir = str(HERE)
c.ServerApp.preferred_dir = str(HERE / "examples")
c.ServerApp.open_browser = True

c.KernelSpecManager.extra_search_paths = [
    str(HERE / "share" / "jupyter" / "kernels")
]
c.KernelSpecManager.allowed_kernelspecs = {
    "dql-python",
    "dql-rust",
    "python-dql",
}

c.FileContentsManager.hide_globs = [
    "__pycache__",
    "*.pyc",
    "*.pyo",
    ".venv",
    ".jupyter-data",
    ".jupyter-runtime",
    ".jupyter-config",
    ".ipynb_checkpoints",
    ".pytest_cache",
    "*.egg-info",
    "share",
]
