from cli import Cli


def _values(count: int) -> str:
    return ", ".join("('%s')" % i for i in range(count))


def _assert_chunked_write_progress(events: list, total: int = 30) -> None:
    if not events:
        raise AssertionError("expected progress events, got none")
    first, last = events[0], events[-1]
    if first.get("done") != 0:
        raise AssertionError("first progress done should be 0: %s" % events)
    if first.get("total") != total:
        raise AssertionError("first progress total should be %s: %s" % (total, events))
    if first.get("phase") != "write":
        raise AssertionError("progress phase should be write: %s" % events)
    if not any(event.get("done") == 25 for event in events):
        raise AssertionError("missing 25-item chunk progress: %s" % events)
    if last.get("done") != total:
        raise AssertionError("last progress done should be %s: %s" % (total, events))
    if any("ok" in event for event in events):
        raise AssertionError("progress lines must not include ok: %s" % events)


def test_110_insert_bulk_progress(cli: Cli) -> None:
    """INSERT 30 rows emits write progress at 0, 25, and 30 (serve or DQL_PROGRESS_JSON)."""
    table = cli.table()
    create = f"CREATE TABLE {table} (id STRING HASH KEY);"
    insert = f"INSERT INTO {table} (id) VALUES {_values(30)};"
    if cli.label == "dqlrs":
        with cli.serve() as session:
            _progress, created = session.exec_dql(
                f"DROP TABLE IF EXISTS {table};\n{create}"
            )
            if created.get("ok") is not True:
                raise AssertionError("serve CREATE failed: %s" % created)
            events, envelope = session.exec_dql(insert)
            if envelope.get("ok") is not True:
                raise AssertionError("serve INSERT failed: %s" % envelope)
            if envelope.get("kind") != "affected" or envelope.get("affected") != 30:
                raise AssertionError("expected 30 affected: %s" % envelope)
            _assert_chunked_write_progress(events)
        return
    cli.oneshot(create)
    events, _result = cli.progress_json_oneshot(insert)
    _assert_chunked_write_progress(events)
