from cli import Cli


def test_300_delete_where_hash(cli: Cli) -> None:
    """DELETE one hash-key item and SELECT the remaining row."""
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"INSERT INTO {table} (id) VALUES ('a'), ('b');"
    )
    cli.assert_json(
        f"DELETE FROM {table} WHERE id = 'a';\n"
        f"SELECT * FROM {table} WHERE id = 'b';",
        [{"id": "b"}],
    )
