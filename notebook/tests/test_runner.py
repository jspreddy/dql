from __future__ import annotations

import os
import stat
from pathlib import Path

import pytest

from dql_notebook_kernel.install import write_dql_kernels
from dql_notebook_kernel.runner import (
    BinaryNotFoundError,
    build_argv,
    format_display,
    resolve_binary,
    try_parse_json,
)


def test_build_argv_default_region_and_json(tmp_path, monkeypatch):
    binary = tmp_path / "dql"
    binary.write_text("#!/bin/sh\n", encoding="utf-8")
    binary.chmod(binary.stat().st_mode | stat.S_IEXEC)
    env = {
        "PATH": str(tmp_path),
        "DQL_BIN": str(binary),
        "AWS_REGION": "",
        "DQL_HOST": "",
        "DQL_NOTEBOOK_JSON": "1",
    }
    monkeypatch.delenv("DQL_HOST", raising=False)
    argv = build_argv("SCAN * FROM t", "python", env=env)
    assert argv[0] == str(binary)
    assert argv[1:3] == ["-r", "us-west-1"]
    assert "--json" in argv
    assert argv[-2:] == ["-c", "SCAN * FROM t"]
    assert "-H" not in argv


def test_build_argv_local_host_and_json_off(tmp_path):
    binary = tmp_path / "dqlrs"
    binary.write_text("#!/bin/sh\n", encoding="utf-8")
    binary.chmod(binary.stat().st_mode | stat.S_IEXEC)
    env = {
        "PATH": str(tmp_path),
        "DQLRS_BIN": str(binary),
        "AWS_REGION": "us-east-1",
        "DQL_HOST": "localhost",
        "DQL_PORT": "8000",
        "DQL_NOTEBOOK_JSON": "0",
    }
    argv = build_argv("ls", "rust", env=env)
    assert argv == [
        str(binary),
        "-r",
        "us-east-1",
        "-H",
        "localhost",
        "-p",
        "8000",
        "-c",
        "ls",
    ]


def test_resolve_binary_missing(monkeypatch):
    monkeypatch.setenv("PATH", "/nonexistent")
    monkeypatch.delenv("DQL_BIN", raising=False)
    with pytest.raises(BinaryNotFoundError, match="dql"):
        resolve_binary("python", env={"PATH": "/nonexistent"})


def test_format_display_json_table():
    bundle = format_display(
        '[{"username":"steve","postid":1},{"username":"drdice","postid":2}]',
        "",
    )
    assert "steve" in bundle["text/plain"]
    assert "<table" in bundle["text/html"]
    assert "steve" in bundle["text/html"]


def test_format_display_includes_stderr():
    bundle = format_display("ok\n", "boom\n")
    assert "ok" in bundle["text/plain"]
    assert "boom" in bundle["text/plain"]


def test_try_parse_json_invalid():
    assert try_parse_json("not json") is None
    assert try_parse_json("") is None


def test_write_dql_kernels(tmp_path: Path):
    written = write_dql_kernels(tmp_path / "kernels", "/opt/venv/bin/python")
    names = {path.parent.name for path in written}
    assert names == {"dql-python", "dql-rust"}
    rust = (tmp_path / "kernels" / "dql-rust" / "kernel.json").read_text(
        encoding="utf-8"
    )
    assert "--backend" in rust
    assert "rust" in rust
    assert "/opt/venv/bin/python" in rust


def test_unknown_backend():
    with pytest.raises(ValueError):
        resolve_binary("jvm", env=os.environ)
