# DQL Rust Implementation

This workspace is the first Rust implementation slice for DQL. It is organized
around the parallel workstreams identified in `rust-plans/`:

- `crates/dql-parser`: typed AST and parser for the first statement set.
- `crates/dql-expr`: expression rendering, placeholders, value conversion, and
  JSON-safe serialization helpers.
- `crates/dql-models`: table/index metadata, throughput helpers, and pure query
  planning.
- `crates/dql-engine`: statement execution over in-memory and AWS SDK backends.
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
- Phase 4 execution (in progress): `SdkBackend` over `aws-sdk-dynamodb` with
  DynamoDB Local endpoint support, batch write chunking, paginated query/scan,
  UPDATE/ALTER/LOAD, explain/analyze capacity hooks, and token-bucket throttling.
- In-memory backend for fast unit tests; CLI connects to Local when `-H` is set.
- Rust parity tests mirror the Python suite by name. Implemented behavior runs
  normally; deferred tests are `#[ignore]` placeholders with source references.

## DynamoDB Local

Start DynamoDB Local on port 8000, then:

```bash
cargo run -p dql-cli -- -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
cargo test -p dql-engine --test dynamodb_local_smoke -- --ignored
```

Integration tests use `tests/support/mod.rs` (`LocalHarness`) to connect, run
statements, and tear down tables.

## Deferred compatibility work

- Parser options: `USING`, `KEYS IN`, `CONSISTENT`, `ORDER BY` on reads/writes.
- Full parity with `tests/test_queries.py` (index planner, FilterExpression,
  selection arithmetic, KEYS IN batch get).
- FragmentEngine and REPL meta-commands (Phase 5).
- Rich terminal formatting, completion, persistent history, and output modes.

## Local checks

Run from this directory:

- `cargo fmt --check`
- `cargo test --workspace`
- `cargo run -p dql-cli -- --version`
- `cargo run -p dql-cli -- --json -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"`
