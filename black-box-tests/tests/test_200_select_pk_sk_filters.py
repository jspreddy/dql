import json
from pathlib import Path

from cli import Cli

_EXPECTED = json.loads(
    (Path(__file__).resolve().parents[1] / "fixtures" / "pk-sk-records" / "select_expected.json").read_text(
        encoding="utf-8"
    )
)


def test_200_select_pk_sk_filters(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (pk STRING HASH KEY, sk STRING RANGE KEY, THROUGHPUT (10, 10));\n"
        f"LOAD 'fixtures/pk-sk-records/seed.json' INTO {table};"
    )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE pk = 'acct-00' AND begins_with(sk, 'item#00')"
        f" AND status = 'active' AND region = 'us-west';",
        _EXPECTED,
    )
