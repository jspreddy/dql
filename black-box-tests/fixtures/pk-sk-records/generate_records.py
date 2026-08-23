#!/usr/bin/env python3
"""Generate the shared 1000-row JSON-lines fixture under fixtures/pk-sk-records/.

Run this once (or to regenerate). The harness does not import this file and
does not need faker at test time.

    python3 -m pip install faker
    python3 generate_records.py
"""

from __future__ import annotations

import json
from pathlib import Path

from faker import Faker

SEED = 42
NUM_RECORDS = 1000
NUM_PARTITIONS = 20

QUERY_PK = "acct-00"
QUERY_SK_PREFIX = "item#00"
QUERY_STATUS = "active"
QUERY_REGION = "us-west"

STATUSES = ("active", "inactive", "pending", "shipped")
REGIONS = ("us-west", "us-east", "eu-west", "ap-south")

# Rows in acct-00 whose sk starts with item#00 (i=0,20,40,60,80). Mixed
# status/region so the extra filters drop some key-condition matches.
PLANTED = {
    0: (QUERY_STATUS, QUERY_REGION),
    20: (QUERY_STATUS, "us-east"),
    40: ("inactive", QUERY_REGION),
    60: (QUERY_STATUS, QUERY_REGION),
    80: ("pending", QUERY_REGION),
}

HERE = Path(__file__).resolve().parent
SUITE_ROOT = HERE.parent.parent
SEED_PATH = HERE / "seed.json"
SELECT_EXPECTED_PATH = (
    SUITE_ROOT / "cases" / "200-select-pk-sk-filters" / "40-expected.json"
)


def matches_select_query(row: dict) -> bool:
    return (
        row["pk"] == QUERY_PK
        and str(row["sk"]).startswith(QUERY_SK_PREFIX)
        and row["status"] == QUERY_STATUS
        and row["region"] == QUERY_REGION
    )


def build_records() -> list[dict]:
    fake = Faker()
    Faker.seed(SEED)
    fake.seed_instance(SEED)
    rows: list[dict] = []
    for i in range(NUM_RECORDS):
        pk = f"acct-{i % NUM_PARTITIONS:02d}"
        sk = f"item#{i:04d}"
        if i in PLANTED:
            status, region = PLANTED[i]
        else:
            status = fake.random_element(elements=STATUSES)
            region = fake.random_element(elements=REGIONS)
        rows.append(
            {
                "pk": pk,
                "sk": sk,
                "status": status,
                "region": region,
                "score": fake.random_int(min=1, max=100),
                "city": fake.city(),
                "email": fake.unique.email(),
                "first_name": fake.first_name(),
                "last_name": fake.last_name(),
                "phone": fake.phone_number(),
            }
        )
    return rows


def main() -> None:
    rows = build_records()
    if len(rows) != NUM_RECORDS:
        raise SystemExit(f"expected {NUM_RECORDS} rows, got {len(rows)}")
    widths = {len(row) for row in rows}
    if widths != {10}:
        raise SystemExit(f"expected 10 attributes per row, got widths {sorted(widths)}")

    with SEED_PATH.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")))
            handle.write("\n")

    expected = [row for row in rows if matches_select_query(row)]
    expected.sort(key=lambda row: (row["pk"], row["sk"]))
    if not expected:
        raise SystemExit("select query matched no rows; refuse to write an empty oracle")
    SELECT_EXPECTED_PATH.write_text(
        json.dumps(expected, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(rows)} json-lines to {SEED_PATH}")
    print(f"wrote {len(expected)} expected items to {SELECT_EXPECTED_PATH}")


if __name__ == "__main__":
    main()
