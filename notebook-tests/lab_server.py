"""Start a project-local JupyterLab for Playwright."""

from __future__ import annotations

import os
import shutil
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
NOTEBOOK_DIR = REPO_ROOT / "notebook"
FIXTURES = Path(__file__).resolve().parent / "fixtures"


class LabStartupError(RuntimeError):
    """JupyterLab did not become ready."""


@dataclass
class LabServer:
    process: subprocess.Popen[str]
    base_url: str
    token: str
    workdir: Path
    log_path: Path

    @property
    def lab_url(self) -> str:
        return f"{self.base_url}/lab?token={self.token}"

    def stop(self) -> None:
        if self.process.poll() is not None:
            return
        os.killpg(self.process.pid, signal.SIGTERM)
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait(timeout=5)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _port_open(port: int) -> bool:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=1):
            return True
    except OSError:
        return False


def _require_local() -> None:
    port = int(os.environ.get("DQL_LOCAL_PORT", "8000"))
    if _port_open(port):
        return
    raise LabStartupError(
        "DynamoDB Local is not listening on port "
        f"{port}. Start it with ./notebook-tests/run.sh --start-local"
    )


def _resolve_binary(env_name: str, *candidates: Path) -> str:
    configured = (os.environ.get(env_name) or "").strip()
    if configured:
        path = Path(configured)
        if path.is_file() and os.access(path, os.X_OK):
            return str(path)
        raise LabStartupError(f"{env_name}={configured} is not executable")
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    names = ", ".join(str(path) for path in candidates)
    raise LabStartupError(
        f"Could not find {env_name}. Set {env_name} or build one of: {names}"
    )


def _notebook_python() -> Path:
    python = NOTEBOOK_DIR / ".venv" / "bin" / "python"
    jupyter = NOTEBOOK_DIR / ".venv" / "bin" / "jupyter"
    if not python.is_file() or not jupyter.is_file():
        raise LabStartupError(
            "Notebook env is missing. Run ./notebook/start.sh --dry-run "
            "(./notebook-tests/run.sh does this)."
        )
    return python


def _ensure_kernels(python: Path) -> None:
    prefix = NOTEBOOK_DIR / "share" / "jupyter"
    subprocess.run(
        [
            str(python),
            "-m",
            "dql_notebook_kernel.install",
            "--prefix",
            str(prefix),
            "--python",
            str(python),
        ],
        check=True,
        cwd=NOTEBOOK_DIR,
    )
    subprocess.run(
        [
            str(python),
            "-m",
            "ipykernel",
            "install",
            "--prefix",
            str(NOTEBOOK_DIR),
            "--name",
            "python-dql",
            "--display-name",
            "Python (dql)",
        ],
        check=True,
        cwd=NOTEBOOK_DIR,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def _wait_until_ready(base_url: str, token: str, process: subprocess.Popen[str], log_path: Path) -> None:
    deadline = time.monotonic() + 60
    last_error = "no response"
    url = f"{base_url}/api/status?token={token}"
    while time.monotonic() < deadline:
        if process.poll() is not None:
            tail = log_path.read_text(encoding="utf-8", errors="replace")[-4000:]
            raise LabStartupError(
                f"JupyterLab exited with status {process.returncode}\n{tail}"
            )
        try:
            with urllib.request.urlopen(url, timeout=1) as response:
                if response.status == 200:
                    return
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            last_error = str(exc)
        time.sleep(0.25)
    tail = log_path.read_text(encoding="utf-8", errors="replace")[-4000:]
    raise LabStartupError(f"JupyterLab did not become ready ({last_error})\n{tail}")


def start_lab(workdir: Path) -> LabServer:
    """Launch JupyterLab against a copy of the smoke notebook."""
    _require_local()
    python = _notebook_python()
    _ensure_kernels(python)
    dqlrs = _resolve_binary(
        "DQLRS_BIN",
        REPO_ROOT / "rust-impl" / "target" / "debug" / "dqlrs",
        REPO_ROOT / "rust-impl" / "target" / "release" / "dqlrs",
    )
    dql_candidates = [REPO_ROOT / "py-impl" / ".venv" / "bin" / "dql"]
    dql = ""
    try:
        dql = _resolve_binary("DQL_BIN", *dql_candidates)
    except LabStartupError:
        if (os.environ.get("DQL_BIN") or "").strip():
            raise

    workdir.mkdir(parents=True, exist_ok=True)
    shutil.copy(FIXTURES / "smoke-dqlrs.ipynb", workdir / "smoke-dqlrs.ipynb")
    runtime = workdir / "runtime"
    config = workdir / "config"
    data = workdir / "data"
    for path in (runtime, config, data):
        path.mkdir(parents=True, exist_ok=True)

    port = _free_port()
    token = os.urandom(16).hex()
    log_path = workdir / "jupyter.log"
    env = os.environ.copy()
    env.update(
        {
            "AWS_ACCESS_KEY_ID": env.get("AWS_ACCESS_KEY_ID") or "fakeid",
            "AWS_SECRET_ACCESS_KEY": env.get("AWS_SECRET_ACCESS_KEY") or "fakekey",
            "AWS_REGION": env.get("AWS_REGION") or "us-west-1",
            "AWS_DEFAULT_REGION": env.get("AWS_DEFAULT_REGION") or "us-west-1",
            "AWS_EC2_METADATA_DISABLED": "true",
            "DQL_HOST": "localhost",
            "DQL_PORT": env.get("DQL_LOCAL_PORT") or env.get("DQL_PORT") or "8000",
            "DQL_NOTEBOOK_JSON": "1",
            "DQL_NOTEBOOK_SERVE": "1",
            "DQLRS_BIN": dqlrs,
            "JUPYTER_DATA_DIR": str(data),
            "JUPYTER_RUNTIME_DIR": str(runtime),
            "JUPYTER_CONFIG_DIR": str(config),
            "JUPYTER_PATH": str(NOTEBOOK_DIR / "share" / "jupyter"),
            "JUPYTER_PLATFORM_DIRS": "1",
        }
    )
    if dql:
        env["DQL_BIN"] = dql

    jupyter = NOTEBOOK_DIR / ".venv" / "bin" / "jupyter"
    command = [
        str(jupyter),
        "lab",
        "--no-browser",
        "--ip",
        "127.0.0.1",
        "--port",
        str(port),
        "--notebook-dir",
        str(workdir),
        f"--ServerApp.token={token}",
        "--ServerApp.password=",
        "--ServerApp.open_browser=False",
        "--ServerApp.allow_remote_access=False",
    ]
    log_file = log_path.open("w", encoding="utf-8")
    process = subprocess.Popen(
        command,
        cwd=workdir,
        env=env,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    base_url = f"http://127.0.0.1:{port}"
    try:
        _wait_until_ready(base_url, token, process, log_path)
    except Exception:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
        raise
    return LabServer(
        process=process,
        base_url=base_url,
        token=token,
        workdir=workdir,
        log_path=log_path,
    )
