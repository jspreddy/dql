from cli import Cli


def test_100_dump_schema(cli: Cli) -> None:
    """DUMP SCHEMA for a table created with explicit throughput."""
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY, THROUGHPUT (2, 3));")
    cli.assert_stdout(f"DUMP SCHEMA {table};", "(2, 3)")
