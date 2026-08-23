from cli import Cli


def test_100_drop_existing_table(cli: Cli) -> None:
    """DROP an existing table and confirm the dropped-table message."""
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.assert_stdout(f"DROP TABLE {table};", "Dropped table")
