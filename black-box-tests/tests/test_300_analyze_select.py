from cli import Cli


def test_300_analyze_select(cli: Cli) -> None:
    """ANALYZE SELECT still returns the matching item."""
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"INSERT INTO {table} (id) VALUES ('a');"
    )
    cli.assert_json(
        f"ANALYZE SELECT * FROM {table} WHERE id = 'a';",
        [{"id": "a"}],
    )
