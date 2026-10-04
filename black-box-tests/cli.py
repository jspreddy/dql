"""pexpect spawn helper for dql / dqlrs -c against DynamoDB Local. No dql import."""

from __future__ import annotations

import json
import os
import re
import subprocess
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterator, Mapping, Optional

import pexpect
import pytest

import report
from compare import json_equal, parse_cli_json, parse_progress_lines, stdout_matches

ANSI_RE = re.compile(
    r"\x1b\[[0-9;?]*[ -/]*[@-~]"
    r"|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)"
    r"|\x1b[()][0-9A-Za-z]"
    r"|\x0f|\x0e"
)

DEFAULT_TIMEOUT = 120
PTY_ROWS = 40
PTY_COLS = 120


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text)


def sanitize_slug(slug: str) -> str:
    slug = slug.replace("/", "_").replace("-", "_")
    return "".join(ch if ch.isalnum() or ch == "_" else "_" for ch in slug)


def table_slug_from_nodeid(nodeid: str) -> str:
    """tests/test_010_foo.py::test_010_foo[dql] -> 010_foo"""
    name = nodeid.split("::")[-1].split("[")[0]
    if name.startswith("test_"):
        name = name[5:]
    return sanitize_slug(name)


@dataclass
class Result:
    stdout: str
    exitstatus: int
    argv: list[str]


@dataclass
class BinSpec:
    label: str
    path: Path


@dataclass
class Cli:
    binary: Path
    label: str
    host: str
    port: int
    region: str
    env: Mapping[str, str]
    cwd: Path
    timeout: float
    nodeid: str
    pid: int
    tables: list[str] = field(default_factory=list)
    verbose: bool = False

    def table(self) -> str:
        index = len(self.tables) + 1
        name = "at_%s_%s_%s_%s" % (
            table_slug_from_nodeid(self.nodeid),
            sanitize_slug(self.label),
            index,
            self.pid,
        )
        self.tables.append(name)
        return name

    def oneshot(
        self,
        script: str,
        *,
        json: bool = False,
        check: bool = True,
        step: str | None = None,
    ) -> Result:
        argv = [
            str(self.binary),
            "-H",
            self.host,
            "-p",
            str(self.port),
            "-r",
            self.region,
        ]
        if json:
            argv.append("--json")
        argv.extend(["-c", script])
        child = pexpect.spawn(
            argv[0],
            argv[1:],
            encoding="utf-8",
            timeout=self.timeout,
            cwd=str(self.cwd),
            env=dict(self.env),
            echo=False,
            dimensions=(PTY_ROWS, PTY_COLS),
            codec_errors="replace",
        )
        child.expect(pexpect.EOF)
        child.close()
        stdout = strip_ansi(child.before or "")
        if child.exitstatus is not None:
            rc = child.exitstatus
        elif child.signalstatus is not None:
            rc = 128 + child.signalstatus
        else:
            rc = -1
        result = Result(stdout=stdout, exitstatus=rc, argv=argv)
        self._emit_spawn_transcript(script, result, json_mode=json, check=check, step=step)
        if check and rc != 0:
            pytest.fail(
                "CLI exited %s\nargv: %s\n--- stdout ---\n%s"
                % (rc, " ".join(argv), stdout)
            )
        return result

    def _infer_step(self, script: str, json_mode: bool, check: bool, step: str | None) -> str:
        if step:
            return step
        if not check and script.lstrip().upper().startswith("DROP TABLE"):
            return "Teardown"
        if json_mode:
            return "Test"
        return "Setup"

    def _emit_spawn_transcript(
        self,
        script: str,
        result: Result,
        *,
        json_mode: bool,
        check: bool,
        step: str | None,
    ) -> None:
        if not self.verbose:
            return
        title = self._infer_step(script, json_mode, check, step)
        report.print_step(title, script, "sql")
        out = result.stdout
        lexer = report.lexer_for_output(out, json_mode)
        if lexer == "json":
            out = report.pretty_json_text(out)
        report.print_step("Output", out, lexer)
        if result.exitstatus != 0:
            report.print_step("stderr", result.stdout, "text")

    def json(self, script: str) -> Any:
        result = self.oneshot(script, json=True, check=True)
        try:
            return parse_cli_json(result.stdout)
        except ValueError as err:
            pytest.fail("%s\n--- stdout ---\n%s" % (err, result.stdout))

    def assert_json(self, script: str, expected: Any) -> None:
        actual = self.json(script)
        if self.verbose:
            report.print_step("Expect", report.pretty_json(expected), "json")
        if not json_equal(actual, expected):
            pytest.fail(
                "json mismatch\n--- actual ---\n%s\n--- expected ---\n%s"
                % (
                    json.dumps(actual, indent=2, sort_keys=True, default=str),
                    json.dumps(expected, indent=2, sort_keys=True, default=str),
                )
            )

    def assert_stdout(self, script: str, expected: str, *, json: bool = False) -> Result:
        result = self.oneshot(script, json=json, check=True, step="Test")
        if self.verbose:
            lexer = "json" if json else "text"
            report.print_step("Expect", expected, lexer)
        if not stdout_matches(result.stdout, expected):
            pytest.fail(
                "stdout mismatch\n--- actual ---\n%s\n--- expected ---\n%s"
                % (result.stdout, expected)
            )
        return result

    def require_dqlrs(self) -> None:
        if self.label != "dqlrs":
            pytest.skip("requires dqlrs")

    def run_args(self, args: list[str], *, check: bool = True) -> Result:
        """Spawn the binary with extra argv. stdout is stdout+stderr (help is often stderr)."""
        argv = [str(self.binary)] + list(args)
        try:
            completed = subprocess.run(
                argv,
                capture_output=True,
                text=True,
                env=dict(self.env),
                cwd=str(self.cwd),
                timeout=self.timeout,
            )
        except subprocess.TimeoutExpired as err:
            pytest.fail("timed out running %s" % " ".join(argv))
            raise err
        combined = "%s%s" % (completed.stdout or "", completed.stderr or "")
        result = Result(
            stdout=strip_ansi(combined),
            exitstatus=completed.returncode,
            argv=argv,
        )
        if self.verbose:
            report.print_step("Test", " ".join(argv), "text")
            report.print_step("Output", result.stdout, "text")
        if check and result.exitstatus != 0:
            pytest.fail(
                "CLI exited %s\nargv: %s\n--- output ---\n%s"
                % (result.exitstatus, " ".join(argv), result.stdout)
            )
        return result

    def progress_json_oneshot(self, script: str) -> tuple[list[Any], Result]:
        """Run `-c` with DQL_PROGRESS_JSON=1 (Python notebook path). Progress is on stderr."""
        env = dict(self.env)
        env["DQL_PROGRESS_JSON"] = "1"
        argv = [
            str(self.binary),
            "-H",
            self.host,
            "-p",
            str(self.port),
            "-r",
            self.region,
            "-c",
            script,
        ]
        try:
            completed = subprocess.run(
                argv,
                capture_output=True,
                text=True,
                env=env,
                cwd=str(self.cwd),
                timeout=self.timeout,
            )
        except subprocess.TimeoutExpired as err:
            pytest.fail("timed out running progress -c")
            raise err
        stdout = strip_ansi(completed.stdout or "")
        stderr = strip_ansi(completed.stderr or "")
        result = Result(stdout=stdout, exitstatus=completed.returncode, argv=argv)
        events = parse_progress_lines(stderr)
        if self.verbose:
            report.print_step("Test", script, "sql")
            report.print_step("Output", stdout, "text")
            report.print_step("stderr", stderr, "json" if events else "text")
        if result.exitstatus != 0:
            pytest.fail(
                "CLI exited %s\nargv: %s\n--- stdout ---\n%s\n--- stderr ---\n%s"
                % (result.exitstatus, " ".join(argv), stdout, stderr)
            )
        return events, result

    @contextmanager
    def serve(self) -> Iterator["ServeSession"]:
        self.require_dqlrs()
        session = ServeSession(self)
        try:
            yield session
        finally:
            session.close()


class ServeSession:
    """One `dqlrs --serve` child. JSON-lines on stdio; not a TTY."""

    def __init__(self, cli: Cli) -> None:
        self.cli = cli
        self._next_id = 1
        self._closed = False
        argv = [
            str(cli.binary),
            "--serve",
            "-H",
            cli.host,
            "-p",
            str(cli.port),
            "-r",
            cli.region,
        ]
        try:
            self.proc = subprocess.Popen(
                argv,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=dict(cli.env),
                cwd=str(cli.cwd),
                bufsize=1,
            )
        except OSError as exc:
            pytest.fail("failed to spawn dqlrs --serve: %s" % exc)
        ping = self.request("ping")
        if not ping.get("ok"):
            self.close()
            pytest.fail("dqlrs --serve ping failed: %s" % ping)

    def request(self, op: str, dql: Optional[str] = None) -> dict[str, Any]:
        _progress, envelope = self._exchange(op, dql)
        return envelope

    def exec_dql(self, dql: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
        return self._exchange("exec", dql)

    def _exchange(
        self, op: str, dql: Optional[str]
    ) -> tuple[list[dict[str, Any]], dict[str, Any]]:
        if self.proc.stdin is None or self.proc.stdout is None:
            pytest.fail("dqlrs --serve has no stdio")
        payload: dict[str, Any] = {"id": str(self._next_id), "op": op}
        self._next_id += 1
        if dql is not None:
            payload["dql"] = dql
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()
        progress: list[dict[str, Any]] = []
        while True:
            line = self.proc.stdout.readline()
            if line == "":
                stderr = ""
                if self.proc.stderr:
                    stderr = self.proc.stderr.read() or ""
                pytest.fail(
                    "dqlrs --serve exited %s: %s" % (self.proc.poll(), stderr.strip())
                )
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as exc:
                pytest.fail("invalid serve line %r: %s" % (line, exc))
            if not isinstance(obj, dict):
                pytest.fail("serve line is not an object: %r" % line)
            if obj.get("event") == "progress":
                progress.append(obj)
                continue
            if "ok" in obj:
                if self.cli.verbose:
                    if dql:
                        report.print_step("Test", dql, "sql")
                    if progress:
                        report.print_step(
                            "Output",
                            "\n".join(json.dumps(item) for item in progress),
                            "json",
                        )
                    report.print_step("Output", json.dumps(obj), "json")
                return progress, obj
            pytest.fail("unexpected serve line: %s" % line.strip())

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        proc = self.proc
        if proc.poll() is not None:
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
            proc.wait(timeout=5)
