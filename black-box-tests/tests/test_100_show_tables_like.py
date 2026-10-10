from cli import Cli


def _family(name: str, wildcard: str) -> str:
    """at_<slug>_<bin>_<index>_<pid> -> at_<slug>_<bin>_<wildcard>_<pid>."""
    prefix, _, pid = name.rpartition("_")
    stem, _, _index = prefix.rpartition("_")
    return "%s_%s_%s" % (stem, wildcard, pid)


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
    cli.assert_json(
        "SHOW TABLES LIKE '%s';" % _family(table, "%"),
        [{"name": table}, {"name": other}],
    )


def test_100_cli_ls_glob_refresh(cli: Cli) -> None:
    """ls <glob> refresh=True lists matching tables and can describe one."""
    table = cli.table()
    other = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.oneshot(f"CREATE TABLE {other} (id STRING HASH KEY);")
    listed = cli.assert_stdout("ls %s refresh=True" % _family(table, "*"), table)
    assert other in listed.stdout
    cli.assert_stdout(f"ls {table} refresh=True", "Hash Key")
