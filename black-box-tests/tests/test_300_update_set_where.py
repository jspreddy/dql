from cli import Cli


def test_300_update_set_where(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER);\n"
        f"INSERT INTO {table} (id, n) VALUES ('a', 1);"
    )
    cli.assert_json(
        f"UPDATE {table} SET n = 9 WHERE id = 'a';\n"
        f"SELECT * FROM {table} WHERE id = 'a';",
        [{"id": "a", "n": 9}],
    )
