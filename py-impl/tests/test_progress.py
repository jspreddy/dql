"""Progress callback emission from Engine."""

from dql.engine import Engine, encode_progress_event, progress_json_enabled


def test_progress_callback_receives_events():
    engine = Engine()
    seen = []
    engine.progress_callback = lambda done, total, phase: seen.append(
        (done, total, phase)
    )
    engine._report_progress(0, 10, "write")
    engine._report_progress(10, 10, "write")
    assert seen == [(0, 10, "write"), (10, 10, "write")]


def test_encode_progress_event_omits_null_total():
    assert '"total"' not in encode_progress_event(3, None, "read")
    parsed_total = encode_progress_event(3, 9, "write")
    assert '"total":9' in parsed_total
    assert '"event":"progress"' in parsed_total


def test_progress_json_enabled(monkeypatch):
    monkeypatch.delenv("DQL_PROGRESS_JSON", raising=False)
    assert progress_json_enabled() is False
    monkeypatch.setenv("DQL_PROGRESS_JSON", "1")
    assert progress_json_enabled() is True
    monkeypatch.setenv("DQL_PROGRESS_JSON", "0")
    assert progress_json_enabled() is False
