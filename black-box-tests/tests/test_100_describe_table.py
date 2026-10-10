from cli import Cli


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
    assert described.stdout.strip() == listed.stdout.strip()


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
