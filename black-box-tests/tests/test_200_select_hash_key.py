from cli import Cli


def test_200_select_hash_key(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER);\n"
        f"INSERT INTO {table} (id, n) VALUES ('a', 1), ('b', 2);"
    )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE id = 'a';",
        [{"id": "a", "n": 1}],
    )
