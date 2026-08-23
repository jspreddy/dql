from cli import Cli


def test_100_cli_ls(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.assert_stdout("ls", table)
