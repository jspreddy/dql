from cli import Cli


def test_200_select_single_page_omits_progress(cli: Cli) -> None:
    """A one-page SELECT/SCAN over --serve does not emit read progress events."""
    cli.require_dqlrs()
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER);\n"
        f"INSERT INTO {table} (id, n) VALUES ('a', 1), ('b', 2);"
    )
    with cli.serve() as session:
        select_progress, selected = session.exec_dql(
            f"SELECT * FROM {table} WHERE id = 'a';"
        )
        scan_progress, scanned = session.exec_dql(f"SCAN * FROM {table};")
    if selected.get("ok") is not True:
        raise AssertionError("SELECT failed: %s" % selected)
    if scanned.get("ok") is not True:
        raise AssertionError("SCAN failed: %s" % scanned)
    select_reads = [event for event in select_progress if event.get("phase") == "read"]
    scan_reads = [event for event in scan_progress if event.get("phase") == "read"]
    if select_reads or scan_reads:
        raise AssertionError(
            "single-page read progress should be omitted; select=%s scan=%s"
            % (select_reads, scan_reads)
        )
