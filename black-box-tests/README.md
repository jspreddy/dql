# Black-box tests

Black-box checks that drive the **`dql` and `dqlrs` binaries** against
**DynamoDB Local**, the same way a person would: load data, connect, query,
check the reply.

**Do not import DQL.** New code in this directory must not `import dql`,
`use dql_*`, or otherwise reach into `py-impl/` or `rust-impl/`. Talk to
the installed programs only.

## Quick start

From the repository root:

```bash
# Terminal 1 — leave this running
./scripts/install_dynamodb_local.sh

# Terminal 2 — binaries on PATH, or set DQL_BIN / DQLRS_BIN
./black-box-tests/harness/run.sh
./black-box-tests/harness/run.sh select          # family or slug filter
./black-box-tests/harness/run.sh --bin dqlrs
./black-box-tests/harness/run.sh --start-local   # start Local, then run
```

Local must listen on `localhost:8000` (override with `DQL_LOCAL_HOST` /
`DQL_LOCAL_PORT`). Dummy AWS keys are enough:

```bash
export AWS_ACCESS_KEY_ID=fakeid
export AWS_SECRET_ACCESS_KEY=fakekey
```

The harness sets those if they are unset.

## What belongs here

Allowed:

- `.dql` scripts, JSON/CSV fixtures, expected stdout/JSON files
- Shell and Python **stdlib** that spawn `dql` / `dqlrs` (or AWS CLI against Local)

Forbidden:

- Importing either implementation, patching internals, asserting on AST/SDK types
- Using `DQL_BACKEND=memory` as the default path (that is not DynamoDB Local)

Unit and crate tests stay in `py-impl/tests/` and `rust-impl/crates/*/tests/`.

## Layout

```text
black-box-tests/
  harness/          # runner (subprocess only)
  fixtures/         # shared datasets
  cases/<family>/<slug>/
  fake-users/       # ad-hoc sample data (not part of the harness)
  update-gsi-fails/ # ad-hoc DynamoDB Local GSI repro (not a gate)
```

Statement families match the language docs: `create`, `insert`, `select`,
`scan`, `update`, `delete`, `alter`, `drop`, `load`, `dump`, `explain`,
`analyze`, plus `cli` (flags) and `journeys` (multi-step stories).

## Case files

Each case is a directory under `cases/<family>/<slug>/`:

| File | Required | Meaning |
| --- | --- | --- |
| `README.md` | no | What a user is proving |
| `setup.dql` | no | `CREATE` / `INSERT` / `LOAD` before the asserted commands |
| `seed.json` | no | Fixture for `LOAD seed.json INTO {{TABLE}}` |
| `input.dql` | yes | Commands whose output is asserted |
| `teardown.dql` | no | Default: `DROP TABLE {{TABLE}}` (and `{{TABLE2}}` …) |
| `expected.json` | one of json/stdout | Item list (or JSON value) from `--json` |
| `expected.stdout` | one of json/stdout | Exact match, else substring |
| `expected.stderr` | no | Substring; empty file means stderr must be empty |
| `expected.exit` | no | Integer process exit code (default `0`) |
| `mode` | no | `oneshot` (default), `file`, or `repl-stdin` |

The harness replaces `{{TABLE}}`, `{{TABLE2}}`, … with unique names so
cases can share one Local process.

Relative `LOAD` / `SAVE` / `file` paths are resolved from the **case
directory**.

Prefer `--json` oracles. `expected.json` is always a **JSON array of items**,
even for a single row. Pretty `smart` / `rich` tables depend on terminal
width and are a poor acceptance target.

### Modes

- **`oneshot`:** `dql -H localhost -p 8000 [--json] -c "…"` (setup, then input, then teardown).
- **`file`:** write a temp `.dql` and run `-c "file <path>"`.
- **`repl-stdin`:** pipe lines into the process with no `-c`. **Python `dql` only**; `dqlrs` is skipped (TUI REPL).

## Adding a case

1. Copy `cases/select/hash-key/` to `cases/<family>/<slug>/`.
2. Edit `setup.dql` / `input.dql` / `expected.json`.
3. Run `./black-box-tests/harness/run.sh <slug>`.

## Ad-hoc folders (not harness cases)

- [`fake-users/`](fake-users/) — sample `CREATE` / `LOAD` script and JSON. The generator uses `dynamo3`; do not copy that pattern into new cases.
- [`update-gsi-fails/`](update-gsi-fails/) — documents a DynamoDB Local GSI `UpdateTable` failure via AWS CLI. Not a DQL acceptance gate.
