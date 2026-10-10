from cli import Cli


def test_100_show_tables_like(cli: Cli) -> None:
    """SHOW TABLES LIKE returns the matching table name."""
    cli.require_dqlrs()
    table = cli.table()
    other = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"CREATE TABLE {other} (id STRING HASH KEY);"
    )
    cli.assert_json(f"SHOW TABLES LIKE '{table}';", [{"name": table}])
    stem = table.rsplit("_", 1)[0]
    cli.assert_json(
        f"SHOW TABLES LIKE '{stem}_%';",
        [{"name": table}, {"name": other}],
    )


def test_100_cli_ls_glob_refresh(cli: Cli) -> None:
    """ls <glob> refresh=True lists matching tables and can describe one."""
    table = cli.table()
    other = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.oneshot(f"CREATE TABLE {other} (id STRING HASH KEY);")
    stem = table.rsplit("_", 1)[0]
    listed = cli.assert_stdout(f"ls {stem}_* refresh=True", table)
    assert other in listed.stdout
    cli.assert_stdout(f"ls {table} refresh=True", "Hash Key")
