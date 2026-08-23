# Black-box tests

Black-box checks that drive the **`dql` and `dqlrs` binaries** against
**DynamoDB Local**, the same way a person would: load data, connect, query,
check the reply.

**Do not import DQL.** New code in this directory must not `import dql`,
`use dql_*`, or otherwise reach into `py-impl/` or `rust-impl/`. Talk to
the installed programs only. pytest and pexpect are allowed; they spawn the
CLIs.

## Quick start

Requires [uv](https://docs.astral.sh/uv/). From the repository root:

```bash
# Terminal 1 — leave this running
./scripts/install_dynamodb_local.sh

# Terminal 2 — binaries on PATH, or set DQL_BIN / DQLRS_BIN
./black-box-tests/run.sh
./black-box-tests/run.sh 1xx             # numeric group (also 11x, 2xx, …)
./black-box-tests/run.sh --group 1xx     # same as the positional filter
./black-box-tests/run.sh -v              # pytest -v
./black-box-tests/run.sh --bin dqlrs
./black-box-tests/run.sh --start-local   # start Local only if the port is down
./black-box-tests/run.sh --skip-teardown # leave tables in Local after each test
```

Equivalent pytest (from `black-box-tests/`):

```bash
uv run pytest --bin both --group 1xx -k insert
```

If DynamoDB Local is already listening, the suite and
`./scripts/install_dynamodb_local.sh` reuse it and do not start a second JVM.

Local must listen on `localhost:8000` (override with `DQL_LOCAL_HOST` /
`DQL_LOCAL_PORT`). Dummy AWS keys are enough:

```bash
export AWS_ACCESS_KEY_ID=fakeid
export AWS_SECRET_ACCESS_KEY=fakekey
```

The runner sets those if they are unset.

## What belongs here

Allowed:

- pytest tests that spawn `dql` / `dqlrs` via [`cli.py`](cli.py) (pexpect)
- JSON/CSV fixtures loaded with DQL `LOAD`
- Shell or AWS CLI against Local in [`manual-cases/`](manual-cases/) only

Forbidden:

- Importing either implementation, patching internals, asserting on AST/SDK types
- Using `DQL_BACKEND=memory` as the default path (that is not DynamoDB Local)

Unit and crate tests stay in `py-impl/tests/` and `rust-impl/crates/*/tests/`.

## Layout

```text
black-box-tests/
  run.sh            # maps --bin / --group / --start-local onto pytest
  conftest.py       # Local, binaries, diagnostic groups, Cli fixture
  cli.py            # pexpect.spawn of dql/dqlrs -c
  compare.py        # JSON / stdout match helpers
  tests/test_NNN_*.py
  fixtures/         # shared LOAD datasets
  ordering.md       # diagnostic groups (0xx / 1xx / 2xx / 3xx)
  manual-cases/     # ad-hoc scripts; not collected by pytest
```

Tests are named `test_<NNN>_<family>_<slug>`. Numbering and groups are
documented in [ordering.md](ordering.md).

## How a test talks to the CLI

The `cli` fixture is parametrized over `dql` and `dqlrs` (`--bin dql|dqlrs|both`,
default both; a missing binary is skipped unless `--bin` named it).

- `cli.table()` — unique name `at_<NNN>_<slug>_<bin>_<index>_<pid>` so tests
  can share one Local process even when teardown is skipped.
- `cli.oneshot(dql)` — `pexpect.spawn` of `-H -p -c` (PTY). Setup uses this
  without `--json`.
- `cli.assert_json(dql, expected)` — same with `--json`; item lists are
  order-insensitive.
- `cli.assert_stdout(dql, expected)` — exact match, else substring / collapsed
  whitespace (DUMP SCHEMA wrapping).

Teardown is `DROP TABLE IF EXISTS` for every name from `cli.table()`, unless
`--skip-teardown`. Relative `LOAD` paths are resolved from the **suite root**.

Prefer `--json` oracles. Pretty `smart` / `rich` tables depend on terminal
width and are a poor acceptance target.

`--group` uses the same digit-wildcard filter as the old harness (`1xx`, `11x`,
`2xx` match the `NNN` in `test_NNN_…`). Any other string is a substring of the
test name (`create`, `select`).

## Adding a test

1. Copy `tests/test_200_select_hash_key.py` to `tests/test_<NNN>_<family>_<slug>.py`.
   See [ordering.md](ordering.md) for the numbering.
2. Call `cli.table()`, run setup with `cli.oneshot`, assert with `assert_json`
   or `assert_stdout`.
3. Run `./black-box-tests/run.sh 2xx` (or `-k` the new name).

## Manual cases (not pytest)

[`manual-cases/`](manual-cases/) holds ad-hoc scripts pytest does not discover
or run. They are not an acceptance gate.

- [`manual-cases/fake-users/`](manual-cases/fake-users/) — sample `CREATE` /
  `LOAD` script and JSON. The generator uses `dynamo3`; do not copy that
  pattern into new pytest tests.
- [`manual-cases/update-gsi-fails/`](manual-cases/update-gsi-fails/) —
  documents a DynamoDB Local GSI `UpdateTable` failure via AWS CLI.
