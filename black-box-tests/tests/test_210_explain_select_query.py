from cli import Cli


def test_210_explain_select_query(cli: Cli) -> None:
    """EXPLAIN SELECT shows the key condition without running the query."""
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"INSERT INTO {table} (id) VALUES ('a');"
    )
    cli.assert_stdout(
        f"EXPLAIN SELECT * FROM {table} WHERE id = 'a';",
        "id = :v1",
    )
