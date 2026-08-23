from cli import Cli


def test_300_update_except_pk_sk_filters(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (pk STRING HASH KEY, sk STRING RANGE KEY, THROUGHPUT (10, 10));\n"
        f"LOAD 'fixtures/pk-sk-records/seed.json' INTO {table};"
    )
    cli.assert_stdout(
        f"UPDATE {table} SET patched = 1 WHERE NOT (pk = 'acct-00' AND begins_with(sk, 'item#00')"
        f" AND status = 'active' AND region = 'us-west');\n"
        f"SCAN count(*) FROM {table} WHERE attribute_exists(patched);",
        "998",
    )
