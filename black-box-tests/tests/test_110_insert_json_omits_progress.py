from cli import Cli


def test_110_insert_json_omits_progress(cli: Cli) -> None:
    """-c --json INSERT stays item/status NDJSON and does not emit progress events."""
    table = cli.table()
    result = cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"INSERT INTO {table} (id) VALUES ('a'), ('b');\n"
        f"SELECT * FROM {table} WHERE id = 'b';",
        json=True,
    )
    if '"event":"progress"' in result.stdout.replace(" ", ""):
        raise AssertionError(
            "-c --json stdout must not include progress events\n%s" % result.stdout
        )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE id = 'a';",
        [{"id": "a"}],
    )
