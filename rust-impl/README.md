# DQL Rust Implementation

[![Rust CI](https://github.com/stevearc/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/stevearc/dql/actions/workflows/rust-workflows.yml)

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
- Phase 4 execution: `SdkBackend` over `aws-sdk-dynamodb` with DynamoDB Local
  endpoint support, batch write chunking, paginated query/scan, UPDATE/ALTER/LOAD
  (JSON lines and CSV), explain kwargs, analyze capacity hooks, and token-bucket
  throttling on memory and SDK backends.
- In-memory backend for fast unit tests; CLI connects to Local when `-H` is set.
- Rust parity tests mirror the Python suite by name. Implemented behavior runs
  normally; deferred tests are `#[ignore]` placeholders with source references.

## DynamoDB Local

Several integration tests require DynamoDB Local on port 8000 (override with
`DQL_LOCAL_HOST` and `DQL_LOCAL_PORT`). They run as part of the normal test
suite and fail if Local is not reachable.

Start DynamoDB Local, then:

```bash
cargo run -p dql-cli -- -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
cargo test -p dql-engine --test dynamodb_local_smoke
cargo test -p dql-engine --test dynamodb_local_parity
cargo test --workspace
```

Integration tests use `tests/support/mod.rs` (`LocalHarness`) to connect, run
statements, and tear down tables.

## Local checks

Run from this directory:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo build --release -p dql-cli`
- `./scripts/smoke_test.sh`
- `cargo run -p dql-cli -- --version`
- `cargo run -p dql-cli -- --json -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"`

## Install from this workspace

Build a release binary locally::

    cargo build --release -p dql-cli
    install -m 0755 target/release/dql ~/.local/bin/dql

Install from a git tag with Cargo::

    cargo install --git https://github.com/stevearc/dql.git --tag <version> --locked -p dql-cli --root ~/.local

Download a published binary with the install script from the repository root::

    curl -fsSL https://raw.githubusercontent.com/stevearc/dql/v-rust/bin/install-rust.sh | sh
