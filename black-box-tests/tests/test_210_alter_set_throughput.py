from cli import Cli


def test_210_alter_set_throughput(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY, THROUGHPUT (1, 1));")
    cli.assert_stdout(
        f"ALTER TABLE {table} SET THROUGHPUT (2, 2);\nDUMP SCHEMA {table};",
        "(2, 2)",
    )
