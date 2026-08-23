from cli import Cli


def test_110_insert_multiple_values(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER);")
    cli.assert_json(
        f"INSERT INTO {table} (id, n) VALUES ('a', 1), ('b', 2);\n"
        f"SELECT * FROM {table} WHERE id = 'b';",
        [{"id": "b", "n": 2}],
    )
