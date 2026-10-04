from cli import Cli


def test_100_drop_if_exists_missing_table(cli: Cli) -> None:
    """DROP TABLE IF EXISTS on a missing table then CREATE in the same -c."""
    table = cli.table()
    result = cli.oneshot(
        f"DROP TABLE IF EXISTS {table};\n"
        f"CREATE TABLE {table} (id STRING HASH KEY);"
    )
    if "service error" in result.stdout.lower():
        raise AssertionError(
            "DROP IF EXISTS of a missing table should not be a service error\n%s"
            % result.stdout
        )
    cli.assert_json(
        f"INSERT INTO {table} (id) VALUES ('ok');\n"
        f"SELECT * FROM {table} WHERE id = 'ok';",
        [{"id": "ok"}],
    )
