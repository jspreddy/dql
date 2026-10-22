from cli import Cli


def _without_cache_note(text: str) -> str:
    """Drop the `cache:` hint `ls` prints ahead of a table description."""
    lines = []
    skip_blank = False
    for line in text.splitlines():
        if line.startswith("cache:"):
            skip_blank = True
            continue
        if skip_blank and not line.strip():
            skip_blank = False
            continue
        skip_blank = False
        lines.append(line)
    return "\n".join(lines).strip()


def _family(name: str, wildcard: str) -> str:
    """at_<slug>_<bin>_<index>_<pid> -> at_<slug>_<bin>_<wildcard>_<pid>."""
    prefix, _, pid = name.rpartition("_")
    stem, _, _index = prefix.rpartition("_")
    return "%s_%s_%s" % (stem, wildcard, pid)


def test_100_describe_table(cli: Cli) -> None:
    """DESCRIBE prints the same description as ls of that one table."""
    cli.require_dqlrs()
    table = cli.table()
    other = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (id STRING HASH KEY, n NUMBER RANGE KEY);\n"
        f"CREATE TABLE {other} (id STRING HASH KEY);"
    )
    described = cli.assert_stdout(f"DESCRIBE {table};", "Hash Key")
    assert table in described.stdout
    assert "Range Key" in described.stdout
    assert other not in described.stdout
    listed = cli.assert_stdout(f"ls {table}", "Hash Key")
    assert _without_cache_note(described.stdout) == _without_cache_note(listed.stdout)


def test_100_cli_ls_intelligent_match(cli: Cli) -> None:
    """A partial name lists similar tables and says they are similar names."""
    cli.require_dqlrs()
    table = cli.table()
    other = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.oneshot(f"CREATE TABLE {other} (id STRING HASH KEY);")
    prefix, _, _pid = table.rpartition("_")
    stem, _, _index = prefix.rpartition("_")
    listed = cli.assert_stdout(
        "ls %s" % stem,
        'No exact match for "%s", so showing similar names.' % stem,
    )
    assert table in listed.stdout
    assert other in listed.stdout
    assert "Hash Key" not in listed.stdout
    solo = "zzposts%s" % cli.pid
    cli.tables.append(solo)
    cli.oneshot("CREATE TABLE %s (id STRING HASH KEY);" % solo)
    typo = "zzpsots%s" % cli.pid
    one = cli.assert_stdout(
        "ls %s" % typo,
        'No exact match for "%s", so showing similar names.' % typo,
    )
    assert solo in one.stdout
    assert "Hash Key" in one.stdout
    assert table not in one.stdout
    keyed = cli.table()
    other_keyed = cli.table()
    key = "zzqqbuyer"
    cli.oneshot("CREATE TABLE %s (%s STRING HASH KEY);" % (keyed, key))
    cli.oneshot("CREATE TABLE %s (%s STRING HASH KEY);" % (other_keyed, key))
    related = cli.assert_stdout(
        "ls %s" % key,
        'No exact match for "%s", so showing related keys.' % key,
    )
    assert keyed in related.stdout
    assert other_keyed in related.stdout
    assert "Hash Key" not in related.stdout


def test_100_cli_ls_one_match_describes(cli: Cli) -> None:
    """ls prints a description for one match and a list for several."""
    table = cli.table()
    other = cli.table()
    cli.oneshot(f"CREATE TABLE {table} (id STRING HASH KEY);")
    cli.oneshot(f"CREATE TABLE {other} (id STRING HASH KEY);")
    one = cli.assert_stdout(f"ls {table}", "Hash Key")
    assert table in one.stdout
    assert other not in one.stdout
    listed = cli.assert_stdout("ls %s" % _family(table, "*"), table)
    assert other in listed.stdout
    assert "Hash Key" not in listed.stdout
