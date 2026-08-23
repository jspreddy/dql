from cli import Cli


def test_110_load_json_into_table(cli: Cli) -> None:
    """LOAD JSON-lines into a table and SELECT a loaded item."""
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"LOAD 'fixtures/load-users/seed.json' INTO {table};"
    )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE id = 'u1';",
        [{"id": "u1", "name": "Ada", "age": 36}],
    )
