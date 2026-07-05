# DQL Rust Implementation

This workspace is the first Rust implementation slice for DQL. It is organized
around the parallel workstreams identified in `rust-plans/`:

- `crates/dql-parser`: typed AST and parser for the first statement set.
- `crates/dql-engine`: in-memory execution scaffold over the parser AST.
- `crates/dql-cli`: binary entrypoint for version, one-shot commands, JSON
  output, and a minimal multiline REPL.

## Implemented slice

- Statement parsing for `CREATE TABLE`, `DROP TABLE`, `INSERT`, `DELETE`,
  `SCAN`, `SELECT`, `DUMP SCHEMA`, `EXPLAIN`, and `ANALYZE`.
- Literal parsing for strings, numbers, booleans, nulls, binary values, lists,
  sets, and maps.
- In-memory table creation, insertion, scanning, simple `SELECT ... WHERE`
  filtering, `SCAN ... WHERE` filtering, `LIMIT`, `SCAN LIMIT`, boolean
  `WHERE` groups, deletion, schema dumping, and explain operation recording.
- CLI flags compatible with the Python entrypoint for `-c`, `--command`,
  `-r`, `--region`, `-H`, `--host`, `-p`, `--port`, `--json`, and
  `--version`.
- Rust parity tests mirror the Python suite by name. Tests for implemented
  behavior run normally; tests for deferred parser, DynamoDB, REPL, history,
  and output behavior are checked in as `#[ignore]` placeholders with source
  references.

## Deferred compatibility work

- DynamoDB Local and AWS SDK backends.
- Full expression grammar and DynamoDB expression rendering.
- Index planning, throughput, throttling, `ALTER`, `LOAD`, `UPDATE`, and
  `KEYS IN` paths.
- Rich terminal formatting, completion, persistent history, and full
  meta-command parity.

## Local checks

Run from this directory:

- `cargo fmt --check`
- `cargo test --workspace`
- `cargo run -p dql-cli -- --version`
- `cargo run -p dql-cli -- --json -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"`
