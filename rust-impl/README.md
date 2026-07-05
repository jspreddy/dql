# DQL Rust Implementation

This workspace is the first Rust implementation slice for DQL. It is organized
around the parallel workstreams identified in `rust-plans/`:

- `crates/dql-parser`: typed AST and parser for the first statement set.
- `crates/dql-expr`: expression rendering, placeholders, value conversion, and
  JSON-safe serialization helpers.
- `crates/dql-models`: table/index metadata, throughput helpers, and pure query
  planning.
- `crates/dql-engine`: in-memory execution scaffold over the parser AST.
- `crates/dql-cli`: binary entrypoint for version, one-shot commands, JSON
  output, and a minimal multiline REPL.

## Implemented slice

- Phase 1 language-core parsing for `SELECT`, `SCAN`, `INSERT`, `UPDATE`,
  `DELETE`, `CREATE`, `DROP`, `ALTER`, `DUMP`, `LOAD`, `EXPLAIN`, and
  `ANALYZE`.
- Literal parsing for strings, numbers, booleans, nulls, binary values, lists,
  sets, maps, timestamps, and intervals.
- Typed parser ASTs for constraints, selections, update expressions, indexes,
  query options, and multiline fragment status.
- Phase 2 expression/value compatibility for DynamoDB-style field and value
  placeholders, condition/update/projection rendering, DQL value-to-attribute
  conversion, and JSON-safe value serialization.
- Phase 3 metadata/query planning for table fields, LSIs, GSIs, projection
  checks, throughput totals, index matching, scan rejection, key/filter splits,
  and follow-up batch-get detection.
- Phase 4 execution groundwork with a backend abstraction, in-memory backend
  implementation, explain/analyze call and capacity scaffolding, and read
  execution routed through planner/expression rendering.
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
- Full Phase 4 completion: DynamoDB Local/AWS adapter, batch retry/pagination,
  update/alter/load execution, and Local integration test waves.
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
