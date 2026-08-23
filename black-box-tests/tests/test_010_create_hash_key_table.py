from cli import Cli


def test_010_create_hash_key_table(cli: Cli) -> None:
    """Create a hash-key table, insert a row, and SELECT it back."""
    table = cli.table()
    cli.assert_json(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"INSERT INTO {table} (id) VALUES ('ok');\n"
        f"SELECT * FROM {table} WHERE id = 'ok';",
        [{"id": "ok"}],
    )
