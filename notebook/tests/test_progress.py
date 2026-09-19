from __future__ import annotations

import os
from pathlib import Path

import pytest

from dql_notebook_kernel.progress import parse_progress_line, progress_bundle
from dql_notebook_kernel.runner import format_envelope, use_serve
from dql_notebook_kernel.serve_client import ServeSession


def test_parse_progress_line():
    event = parse_progress_line(
        '{"id":"1","event":"progress","done":25,"total":30,"phase":"write"}'
    )
    assert event is not None
    assert event["done"] == 25
    assert event["total"] == 30
    assert parse_progress_line('{"ok":true,"kind":"affected"}') is None
    assert parse_progress_line("not json") is None


def test_progress_bundle_has_html_bar():
    bundle = progress_bundle(25, 30, "write")
    assert "25" in bundle["text/plain"]
    assert "<progress" in bundle["text/html"]
    assert 'value="25"' in bundle["text/html"]


def test_use_serve_defaults_on(monkeypatch):
    monkeypatch.delenv("DQL_NOTEBOOK_SERVE", raising=False)
    assert use_serve(env={"PATH": "/bin"}) is True
    assert use_serve(env={"DQL_NOTEBOOK_SERVE": "0"}) is False


def test_format_envelope_items_and_affected():
    items = format_envelope(
        {"ok": True, "kind": "items", "items": [{"id": "a"}, {"id": "b"}]}
    )
    assert "<table" in items["text/html"]
    affected = format_envelope({"ok": True, "kind": "affected", "affected": 30})
    assert "30 affected" in affected["text/plain"]


@pytest.mark.skipif(
    not Path("/workspace/rust-impl/target/debug/dqlrs").is_file(),
    reason="dqlrs debug binary is not built",
)
def test_serve_session_emits_insert_progress():
    env = {
        "DQLRS_BIN": "/workspace/rust-impl/target/debug/dqlrs",
        "DQL_BACKEND": "memory",
        "AWS_REGION": "us-west-1",
        "AWS_ACCESS_KEY_ID": "fakeid",
        "AWS_SECRET_ACCESS_KEY": "fakekey",
        "AWS_EC2_METADATA_DISABLED": "true",
        "DQL_HOST": "",
        "PATH": os.environ.get("PATH", ""),
    }
    session = ServeSession(env=env)
    try:
        created = session.exec_dql("CREATE TABLE t (id STRING HASH KEY);")
        assert created.get("ok") is True
        events: list[tuple[int, int | None, str]] = []
        values = ", ".join(f"('{i}')" for i in range(30))
        envelope = session.exec_dql(
            f"INSERT INTO t (id) VALUES {values};",
            on_progress=lambda done, total, phase: events.append((done, total, phase)),
        )
        assert envelope.get("ok") is True
        assert envelope.get("affected") == 30
        assert events[0] == (0, 30, "write")
        assert (25, 30, "write") in events
        assert events[-1] == (30, 30, "write")
    finally:
        session.close()
