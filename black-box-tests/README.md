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
./black-box-tests/harness/run.sh 1xx             # numeric group (also 11x, 2xx, …)
./black-box-tests/harness/run.sh -v              # log commands, DQL, and CLI output
./black-box-tests/harness/run.sh --bin dqlrs
./black-box-tests/harness/run.sh --start-local   # start Local only if the port is down
./black-box-tests/harness/run.sh --skip-teardown # leave tables in Local after each case
```

If DynamoDB Local is already listening, the harness and
`./scripts/install_dynamodb_local.sh` reuse it and do not start a second JVM.

Local must listen on `localhost:8000` (override with `DQL_LOCAL_HOST` /
`DQL_LOCAL_PORT`). Dummy AWS keys are enough:

```bash
export AWS_ACCESS_KEY_ID=fakeid
export AWS_SECRET_ACCESS_KEY=fakekey
```

The harness sets those if they are unset.

Output is colorized by default. Set `NO_COLOR` to disable it. `-v` / `--verbose`
prints the DQL and CLI output for each step.

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
  cases/<NNN>-<family>-<slug>/
  manual-cases/     # ad-hoc scripts; not run by the harness
```

The `family` segment in the folder name matches the language docs (`create`,
`insert`, `select`, …) plus `cli` and `journeys`. Numbering and groups are
documented in [cases/ordering.md](cases/ordering.md).

## Case files

Each case is a directory `cases/<NNN>-<family>-<slug>/` (must contain
`30-test.dql` plus `40-expected.json` or `40-expected.stdout`):

| File | Required | Meaning |
| --- | --- | --- |
| `00-README.md` | no | What a user is proving |
| `10-mode` | no | `oneshot` (default), `file`, or `repl-stdin` |
| `20-setup.dql` | no | `CREATE` / `INSERT` / `LOAD` before the asserted commands |
| `30-test.dql` | yes | Commands whose output is asserted |
| `40-expected.json` | one of json/stdout | Item list (or JSON value) from `--json` |
| `40-expected.stdout` | one of json/stdout | Exact match, else substring |
| `40-expected.stderr` | no | Substring; empty file means stderr must be empty |
| `40-expected.exit` | no | Integer process exit code (default `0`) |
| `50-teardown.dql` | no | Default: `DROP TABLE {{TABLE}}` (and `{{TABLE2}}` …). Skipped with `--skip-teardown`. |
| `seed.json` | no | DQL fixture for `LOAD seed.json INTO {{TABLE}}` (not a harness step) |

The harness replaces `{{TABLE}}`, `{{TABLE2}}`, … with unique names so cases
and `dql` / `dqlrs` can share one Local process even when teardown is skipped.
A substituted name looks like `at_010_create_hash_key_table_dql_1_58104`:

| Part | Example | Meaning |
| --- | --- | --- |
| `at` | `at` | Prefix for **a**cceptance **t**est tables, so they are easy to spot in Local next to anything else. |
| case folder | `010_create_hash_key_table` | The case directory (`010-create-hash-key-table`) with `/` and `-` turned into `_`, then stripped to `[A-Za-z0-9_]`. Identifies which case created the table. |
| binary | `dql` or `dqlrs` | Which CLI is under test. Stops Python and Rust from sharing a leftover table when teardown is skipped. |
| table index | `1` | Which placeholder: `{{TABLE}}` → `1`, `{{TABLE2}}` → `2`, … up to `{{TABLE20}}`. |
| harness pid | `58104` | `$$` of `run.sh`. A new harness process gets a new pid, so a second run does not reuse names from the first. |

The full pattern is `at_<case>_<bin>_<index>_<pid>`. A case that uses two tables
gets `…_1_<pid>` and `…_2_<pid>` with the same case/bin prefix.

Relative `LOAD` / `SAVE` / `file` paths are resolved from the **case
directory**.

Prefer `--json` oracles. `40-expected.json` is always a **JSON array of items**,
even for a single row. Pretty `smart` / `rich` tables depend on terminal
width and are a poor acceptance target.

### Modes

`10-mode` chooses **how** the harness invokes the CLI, not which DQL to run.
Omit the file to use `oneshot`. If present, the file is a single word
(`oneshot`, `file`, or `repl-stdin`); whitespace is stripped and case is
ignored. Any other value fails the case.

Teardown is always a separate `-c` after the test (unless
`--skip-teardown`). `--json` is used only when the case has
`40-expected.json`.

| Mode | Invocation | Asserted stdout | When to use |
| --- | --- | --- | --- |
| `oneshot` | Separate `-c` for setup (no `--json`), then `-c` for the test script | Test only. Setup failure is reported as setup, not a test mismatch. | Default. Assert one command’s result. |
| `file` | Concatenate setup + test into a temp `.dql`, then `-c "file <path>"` | Setup and test together (one process) | The feature under test is the `file` command. |
| `repl-stdin` | Pipe setup + test + `exit` into stdin; no `-c` | Setup and test together | The feature under test is the line-oriented REPL. **Python `dql` only**; `dqlrs` is skipped (TUI). |

Example (`file`):

```text
cases/200-select-via-file/10-mode    # contents: file
```

## Adding a case

1. Copy `cases/200-select-hash-key/` to `cases/<NNN>-<family>-<slug>/`. See [cases/ordering.md](cases/ordering.md) for the numbering.
2. Edit `20-setup.dql` / `30-test.dql` / `40-expected.json`.
3. Run `./black-box-tests/harness/run.sh 2xx` (or the new folder name).

## Manual cases (not harness cases)

[`manual-cases/`](manual-cases/) holds ad-hoc scripts the harness does not discover or run. They have no `30-test.dql` and are not an acceptance gate.

- [`manual-cases/fake-users/`](manual-cases/fake-users/) — sample `CREATE` / `LOAD` script and JSON. The generator uses `dynamo3`; do not copy that pattern into new harness cases.
- [`manual-cases/update-gsi-fails/`](manual-cases/update-gsi-fails/) — documents a DynamoDB Local GSI `UpdateTable` failure via AWS CLI.
