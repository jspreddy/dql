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


def test_100_show_tables_like_intelligent_match(cli: Cli) -> None:
    """A LIKE that matches nothing lists similar names or related keys."""
    cli.require_dqlrs()
    table = cli.table()
    other = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY);\n"
        f"CREATE TABLE {other} (id STRING HASH KEY);"
    )
    prefix, _, _pid = table.rpartition("_")
    stem, _, _index = prefix.rpartition("_")
    listed = cli.assert_stdout(
        "SHOW TABLES LIKE '%s';" % stem,
        'No exact match for "%s", so showing similar names.' % stem,
    )
    assert table in listed.stdout
    assert other in listed.stdout
    rows = cli.json("SHOW TABLES LIKE '%s';" % stem)
    assert {"name": table} in rows
    assert {"name": other} in rows
    raw = cli.oneshot("SHOW TABLES LIKE '%s';" % stem, json=True)
    assert "No exact match" not in raw.stdout

    keyed = cli.table()
    other_keyed = cli.table()
    key = "zzqqbuyer"
    cli.oneshot("CREATE TABLE %s (%s STRING HASH KEY);" % (keyed, key))
    cli.oneshot("CREATE TABLE %s (%s STRING HASH KEY);" % (other_keyed, key))
    related = cli.assert_stdout(
        "SHOW TABLES LIKE '%s';" % key,
        'No exact match for "%s", so showing related keys.' % key,
    )
    assert keyed in related.stdout
    assert other_keyed in related.stdout

    missing = cli.assert_stdout("SHOW TABLES LIKE 'qqqxxyyzz';", "No results")
    assert "No exact match" not in missing.stdout


def test_100_cli_ls_glob_refresh(cli: Cli) -> None:
    """ls <glob> refresh=True lists matching tables and can describe one."""
    table = cli.table()
    other = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.oneshot(f"CREATE TABLE {other} (id STRING HASH KEY);")
    listed = cli.assert_stdout("ls %s refresh=True" % _family(table, "*"), table)
    assert other in listed.stdout
    cli.assert_stdout(f"ls {table} refresh=True", "Hash Key")
