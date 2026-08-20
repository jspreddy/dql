# DQL Rust Implementation

[![Rust CI](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml)

Maintainer guide for the Cargo workspace that builds `dqlrs`. User install,
flags, and examples: [`rust-docs/`](../rust-docs/). Python-to-Rust differences:
[`rust-docs/migration-from-python.md`](../rust-docs/migration-from-python.md).

This crate lives in [jspreddy/dql](https://github.com/jspreddy/dql), a fork of
[stevearc/dql](https://github.com/stevearc/dql).

| Crate | Role |
| --- | --- |
| `dql-cli` | `dqlrs` binary: clap flags, meta-commands, ratatui REPL |
| `dql-parser` | Lexer/parser and typed statement AST |
| `dql-expr` | Expression rendering, placeholders, value conversion |
| `dql-models` | Table/index metadata and query planning |
| `dql-engine` | Statement execution (AWS SDK + in-memory backends) |
| `dql-output` | smart / column / expanded / json / rich formatters and display |

Work from this directory (`rust-impl/`).

## Workspace layout

```text
crates/
  dql-cli/      # binary + REPL + meta-commands
  dql-parser/   # grammar / AST
  dql-expr/     # DynamoDB expression strings + value helpers
  dql-models/   # TableMeta, QueryPlan, index matching
  dql-engine/   # Engine<B>, MemoryBackend, SdkBackend, file I/O
  dql-output/   # formatters, display backends, table-meta text
scripts/
  smoke_test.sh
  install_dynamodb_local.sh
  install-rust.sh
```

Default CLI backend is live AWS. `DQL_BACKEND=memory` is for offline demos and
smoke tests. Engine unit tests use `MemoryBackend` directly. DynamoDB Local is
selected with `-H` / `-p`.

## Local checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p dql-cli
./scripts/smoke_test.sh
```

Optional CloudWatch/`watch` build:

```bash
cargo build -p dql-cli --features watch
```

Install a local build:

```bash
cargo install --path crates/dql-cli --locked --root ~/.local
```

## DynamoDB Local tests

Integration tests expect Local on port 8000 (override with `DQL_LOCAL_HOST` /
`DQL_LOCAL_PORT`). They run with the normal suite and fail if Local is down.

```bash
./scripts/install_dynamodb_local.sh background
cargo test -p dql-engine --test dynamodb_local_smoke
cargo test -p dql-engine --test dynamodb_local_parity
cargo test --workspace
cargo run -p dql-cli -- -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
```

Harness: `crates/dql-engine/tests/support/mod.rs` (`LocalHarness`).

## Toolchain

See `rust-toolchain.toml`. CI: `.github/workflows/rust-workflows.yml` (fmt,
clippy, tests, smoke).
