from cli import Cli


def test_200_scan_all_items(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER);\n"
        f"INSERT INTO {table} (id, n) VALUES ('a', 1), ('b', 2);"
    )
    cli.assert_json(
        f"SCAN * FROM {table};",
        [{"id": "a", "n": 1}, {"id": "b", "n": 2}],
    )
