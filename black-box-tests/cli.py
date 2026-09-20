"""pexpect spawn helper for dql / dqlrs -c against DynamoDB Local. No dql import."""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

import pexpect
import pytest

import report
from compare import json_equal, parse_cli_json, stdout_matches

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
