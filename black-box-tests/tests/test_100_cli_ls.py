from cli import Cli


def test_100_cli_ls(cli: Cli) -> None:
    """ls lists the table created in setup."""
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.assert_stdout("ls", table)
