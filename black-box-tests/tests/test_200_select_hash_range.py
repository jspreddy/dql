from cli import Cli


def test_200_select_hash_range(cli: Cli) -> None:
    """SELECT by hash and range key."""
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, sk NUMBER RANGE KEY);\n"
        f"INSERT INTO {table} (id, sk, n) VALUES ('p', 1, 10), ('p', 2, 20);"
    )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE id = 'p' AND sk = 1;",
        [{"id": "p", "sk": 1, "n": 10}],
    )
