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

| Topic | Python | Rust |
| --- | --- | --- |
| `LOAD` JSON lines | yes | yes |
| `LOAD` CSV | yes | yes |
| `LOAD` gzip / pickle | yes | not yet |
| `SAVE` exports | multiple formats | not yet |

## Known parity gaps

A small set of engine parity tests remain `#[ignore]` in
`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`:

- Parse-error caret display
- DynamoDB Local GSI throughput edge case
- GSI throughput metadata
- SAVE / gzip / pickle LOAD formats
- Reserved-word and dashed field path regressions

See `rust-impl/README.md` for the current implementation status and
`rust-plans/migration-roadmap.md` for the overall rewrite plan.

## Python package status

The Python package remains available for existing users. New installs on
`v-rust` should prefer the Rust binary. PyPI publication is not removed in this
phase; pip/pex are documented as legacy install paths in `README.rst`.

## Verifying your migration

```bash
dql --version
dql -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
dql --json -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
dql -H localhost -p 8000   # when using DynamoDB Local
```

For a full packaging gate from a clean checkout:

```bash
cd rust-impl
cargo build --release -p dql-cli
./scripts/smoke_test.sh
```
