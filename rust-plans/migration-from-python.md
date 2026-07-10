# Migrating from Python DQL to Rust DQL

The Rust rewrite on the `v-rust` branch ships a standalone `dql` binary that
replaces the Python console script for most workflows. DQL syntax and meta-
commands are intentionally preserved, but a few Python-specific behaviors differ
or remain incomplete.

## Install

| Topic | Python | Rust |
| --- | --- | --- |
| Recommended install | `pip install dql` or PEX from releases | GitHub release binary or `cargo install --git` |
| Install script | `bin/install.py` (PEX) | `bin/install-rust.sh` |
| Development branch | `v-next` | `v-rust` |

Rust install examples:

```bash
curl -fsSL https://raw.githubusercontent.com/stevearc/dql/v-rust/bin/install-rust.sh | sh

cargo install --git https://github.com/stevearc/dql.git --tag 0.6.4 --locked -p dql-cli --root ~/.local
```

## Configuration and history

These paths are unchanged:

- Config: `~/.config/dql.json`
- History: `~/.dql/history`

Existing `opt`, `throttle`, width, pagesize, and format settings carry over.

## AWS authentication

Python DQL uses `botocore`. Rust DQL uses the AWS SDK default credential
chain. The same environment variables and `~/.aws/credentials` file continue to
work.

Default region remains `us-west-1` unless `AWS_REGION` or `-r` overrides it.

### Connection defaults

| Mode | Behavior |
| --- | --- |
| `dql` (no `-H`) | Live AWS DynamoDB (SDK default credential chain) |
| `dql -H localhost -p 8000` | DynamoDB Local |
| `DQL_BACKEND=memory dql …` | In-memory backend (offline demos / CI smoke tests) |

`use <region>` switches the AWS region. `local` / `local off` toggles DynamoDB
Local; `local off` reconnects to live AWS.

## REPL and output

| Topic | Python | Rust |
| --- | --- | --- |
| REPL | readline + Rich | ratatui TUI |
| One-shot `-c` | stdout / JSON | stdout / JSON (pipe-friendly) |
| Pager | `less` via Rich | `less` for non-TUI output |
| `watch` | CloudWatch metrics | optional / limited |

Interactive navigation differs because Rust uses a terminal UI instead of
readline, but one-shot commands and shell scripting behavior are preserved.

## File formats

| Format | Extension(s) | Python | Rust |
| --- | --- | --- | --- |
| JSON lines | `.json`, `.json.gz` | SAVE + LOAD | SAVE + LOAD |
| CSV | `.csv`, `.csv.gz` | SAVE + LOAD | SAVE + LOAD |
| MessagePack | `.msgpack`, `.msgpack.gz`, or default binary | — | SAVE + LOAD |
| Pickle | `.p`, `.pkl`, `.pickle` (+ gzip) | SAVE + LOAD | **rejected** |

Rust replaces Python’s pickle binary format with [MessagePack](https://msgpack.org/).
Files use concatenated MessagePack maps (one object per item), optionally prefixed
with the 4-byte magic `DQL1`. Numbers are tagged as
`{"__dql_number__": "<decimal>"}` and sets as `{"__dql_set__": [...]}` so types
round-trip cleanly.

### Migrating pickle archives

Rust cannot read `.p` / `.pkl` / `.pickle` files. Re-export from Python DQL first:

```text
SELECT * FROM t SAVE 'x.json';
# or
SELECT * FROM t SAVE 'x.csv';
```

Then in Rust:

```text
LOAD 'x.json' INTO t;
# or SAVE/LOAD with MessagePack going forward:
SCAN * FROM t SAVE 'x.msgpack';
LOAD 'x.msgpack' INTO t;
```

Attempting `LOAD archive.p INTO t` returns a clear error pointing at MessagePack
or JSON/CSV re-export.

## Known parity gaps

A small set of engine parity tests remain `#[ignore]` in
`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`:

- Parse-error caret display
- DynamoDB Local GSI throughput edge case
- GSI throughput metadata

See `rust-impl/README.md` for the current implementation status and
`rust-plans/migration-roadmap.md` for the overall rewrite plan.

## Python package status

The Python package remains available for existing users. New installs on
`v-rust` should prefer the Rust binary. PyPI publication is not removed in this
phase; pip/pex are documented as legacy install paths in `README.rst`.

## Verifying your migration

```bash
dql --version
# Live AWS (requires credentials):
dql -c "ls"
# Offline in-memory (no AWS credentials):
DQL_BACKEND=memory dql -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
DQL_BACKEND=memory dql --json -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
dql -H localhost -p 8000   # when using DynamoDB Local
```

For a full packaging gate from a clean checkout:

```bash
cd rust-impl
cargo build --release -p dql-cli
./scripts/smoke_test.sh
```
