"""Black-box pytest config: Local, binaries, diagnostic groups, Cli fixture."""

from __future__ import annotations

import inspect
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import pytest

from cli import BinSpec, Cli
import report
from compare import self_test as compare_self_test

SUITE_ROOT = Path(__file__).resolve().parent
REPO_ROOT = SUITE_ROOT.parent


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--bin",
        action="store",
        default="both",
        choices=("dql", "dqlrs", "both"),
        help="Which CLI binary to run against (default: both, skipping missing)",
    )
    parser.addoption(
        "--start-local",
        action="store_true",
        default=False,
        help="Start DynamoDB Local if the port is down",
    )
    parser.addoption(
        "--skip-teardown",
        action="store_true",
        default=False,
        help="Do not DROP TABLE after each test",
    )
    parser.addoption(
        "--group",
        action="store",
        default="",
        help="Numeric group (1xx, 11x, 2xx) or substring of the test name",
    )


def pytest_configure(config: pytest.Config) -> None:
    failures = compare_self_test()
    if failures:
        pytest.exit("compare.py self-test failed: " + ", ".join(failures), returncode=1)


def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    group = (config.getoption("--group") or "").strip()
    if not group:
        return
    selected: list[pytest.Item] = []
    deselected: list[pytest.Item] = []
    if re.fullmatch(r"[0-9xX]+", group):
        pat = "".join("[0-9]" if char in "xX" else re.escape(char) for char in group)
        rx = re.compile(r"test_" + pat + r"_")
        for item in items:
            name = item.name.split("[")[0]
            if rx.match(name):
                selected.append(item)
            else:
                deselected.append(item)
    else:
        for item in items:
            name = item.name.split("[")[0]
            node = item.nodeid
            if group in name or group in node:
                selected.append(item)
            else:
                deselected.append(item)
    if deselected:
        config.hook.pytest_deselected(items=deselected)
        items[:] = selected
    if group and not selected:
        pytest.exit("no tests matching group %r" % group, returncode=1)


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "cli" not in metafunc.fixturenames:
        return
    bins = resolve_bins(metafunc.config.getoption("--bin"))
    metafunc.parametrize("cli", bins, indirect=True, ids=[spec.label for spec in bins])


def local_host() -> str:
    return os.environ.get("DQL_LOCAL_HOST", "localhost")


def local_port() -> int:
    return int(os.environ.get("DQL_LOCAL_PORT", "8000"))


def local_available(host: str, port: int) -> bool:
    tried: list[str] = []
    for candidate in (host, "127.0.0.1", "localhost"):
        if candidate in tried:
            continue
        tried.append(candidate)
        try:
            with socket.create_connection((candidate, port), timeout=1):
                return True
        except OSError:
            continue
    return False


def wait_for_local(host: str, port: int, tries: int = 30) -> bool:
    for _ in range(tries):
        if local_available(host, port):
            return True
        time.sleep(1)
    return False


def start_local(host: str, port: int) -> None:
    if local_available(host, port):
        return
    script = REPO_ROOT / "scripts" / "install_dynamodb_local.sh"
    if not os.access(script, os.X_OK):
        pytest.exit("missing %s" % script, returncode=1)
    subprocess.run([str(script), "background"], check=True, cwd=str(REPO_ROOT))
    if not wait_for_local(host, port):
        pytest.exit(
            "DynamoDB Local did not become reachable at %s:%s" % (host, port),
            returncode=1,
        )


def require_local(host: str, port: int) -> None:
    if local_available(host, port):
        return
    pytest.exit(
        "DynamoDB Local is not reachable at %s:%s\n"
        "  Start it from the repository root:\n"
        "    ./scripts/install_dynamodb_local.sh\n"
        "  Or re-run with --start-local." % (host, port),
        returncode=1,
    )


def find_named_bin(name: str, env_var: str) -> Path | None:
    from_env = os.environ.get(env_var, "")
    if from_env:
        path = Path(from_env)
        if os.access(path, os.X_OK):
            return path
        pytest.exit("%s is set but not executable: %s" % (env_var, from_env), returncode=1)
    found = shutil.which(name)
    return Path(found) if found else None


def resolve_bins(choice: str) -> list[BinSpec]:
    specs: list[BinSpec] = []
    wanted = (("dql", "DQL_BIN"), ("dqlrs", "DQLRS_BIN"))
    for name, env_var in wanted:
        if choice not in (name, "both"):
            continue
        path = find_named_bin(name, env_var)
        if path is not None:
            specs.append(BinSpec(label=name, path=path))
        elif choice == name:
            pytest.exit(
                "%s not found (set %s or install %s on PATH)" % (name, env_var, name),
                returncode=1,
            )
        else:
            sys.stderr.write("warning: %s not found; skipping (set %s to require it)\n" % (name, env_var))
    if not specs:
        pytest.exit("no dql or dqlrs binary found", returncode=1)
    return specs


def spawn_env(home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["HOME"] = str(home)
    env["XDG_CONFIG_HOME"] = str(home / ".config")
    env.setdefault("AWS_ACCESS_KEY_ID", "fakeid")
    env.setdefault("AWS_SECRET_ACCESS_KEY", "fakekey")
    env.setdefault("AWS_DEFAULT_REGION", "us-west-1")
    env.setdefault("AWS_REGION", "us-west-1")
    env["NO_COLOR"] = "1"
    env["PAGER"] = "cat"
    env["COLUMNS"] = "120"
    env["LINES"] = "40"
    env.pop("DQL_BACKEND", None)
    (home / ".config").mkdir(parents=True, exist_ok=True)
    (home / ".dql").mkdir(parents=True, exist_ok=True)
    return env


@pytest.fixture(scope="session")
def suite_root() -> Path:
    return SUITE_ROOT


@pytest.fixture(scope="session")
def isolation_home() -> Path:
    path = Path(tempfile.mkdtemp(prefix="dql-at-home."))
    yield path
    shutil.rmtree(path, ignore_errors=True)


@pytest.fixture(scope="session", autouse=True)
def _ensure_local(pytestconfig: pytest.Config) -> None:
    host, port = local_host(), local_port()
    if pytestconfig.getoption("--start-local"):
        start_local(host, port)
    else:
        require_local(host, port)


@pytest.fixture
def cli(request: pytest.FixtureRequest, isolation_home: Path, suite_root: Path) -> Cli:
    spec: BinSpec = request.param
    timeout = float(os.environ.get("DQL_BB_TIMEOUT", "120"))
    verbose = request.config.option.verbose >= 1
    helper = Cli(
        binary=spec.path,
        label=spec.label,
        host=local_host(),
        port=local_port(),
        region=os.environ.get("AWS_REGION", "us-west-1"),
        env=spawn_env(isolation_home),
        cwd=suite_root,
        timeout=timeout,
        nodeid=request.node.nodeid,
        pid=os.getpid(),
        verbose=verbose,
    )
    if verbose:
        name = request.node.name.split("[")[0]
        report.print_test_header(name, inspect.getdoc(request.function), spec.label)
    yield helper
    if verbose:
        report.console.print()
    if request.config.getoption("--skip-teardown"):
        if verbose:
            report.print_note_step("Teardown", "skipped (--skip-teardown)")
            report.print_test_footer()
        return
    for name in helper.tables:
        helper.oneshot("DROP TABLE IF EXISTS %s;" % name, check=False)
    if verbose:
        report.print_test_footer()
