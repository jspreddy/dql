# DQL Rust Implementation

[![Rust CI](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml)

Cargo workspace that builds the `dqlrs` CLI and supporting libraries for DynamoDB
Query Language.

| Crate | Role |
| --- | --- |
| `dql-cli` | `dqlrs` binary: clap flags, meta-commands, ratatui REPL |
| `dql-parser` | Lexer/parser and typed statement AST |
| `dql-expr` | Expression rendering, placeholders, value conversion |
| `dql-models` | Table/index metadata and query planning |
| `dql-engine` | Statement execution (AWS SDK + in-memory backends) |
| `dql-output` | smart / column / expanded / json / rich formatters and display |

---

## For `dqlrs` users

### Install

**Release binary (local build):**

```bash
cd rust-impl
cargo build --release -p dql-cli
install -m 0755 target/release/dqlrs ~/.local/bin/dqlrs
```

**From a git tag:**

```bash
cargo install --git https://github.com/jspreddy/dql.git --tag <version> --locked -p dql-cli --root ~/.local
```

**Install script** (from the repository root):

```bash
curl -fsSL https://raw.githubusercontent.com/jspreddy/dql/v-rust/bin/install-rust.sh | sh
```

### Connect

| Mode | How |
| --- | --- |
| Live AWS | `dqlrs` (default when `-H` is unset; uses the AWS SDK credential chain) |
| DynamoDB Local | `dqlrs -H localhost -p 8000` |
| In-memory (offline) | `DQL_BACKEND=memory dqlrs …` |

Region defaults to `AWS_REGION`, else `us-west-1`. Override with `-r`.

Config and history paths: `~/.config/dql.json` and `~/.dql/history`.

### Flags

```text
-c, --command <command>  Run this command and exit
-r, --region <region>    AWS region
-H, --host <host>        Local DynamoDB host
-p, --port <port>        Local DynamoDB port (default 8000)
    --json               With -c, print results as JSON
    --version            Print version and exit
-h, --help               Print help
```

### Statements and meta-commands

DQL statements: `SELECT`, `SCAN`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`,
`DROP`, `ALTER`, `DUMP`, `LOAD`, `EXPLAIN`, `ANALYZE`.

In the REPL, type `help` or `help <statement>`. Meta-commands include `opt`,
`ls`, `use`, `local`, `file`, `throttle`, `unthrottle`, `whoami`, `shell`,
`clear`, `exit`, and `version`. Build with `--features watch` to enable the
`watch` CloudWatch dashboard.

Output formats (via `opt format`): `smart`, `column`, `expanded`, `json`,
`rich`. Non-TUI paths can page with `opt display less`.

### SAVE / LOAD

| Format | Extensions |
| --- | --- |
| JSON lines | `.json`, `.json.gz` |
| CSV | `.csv`, `.csv.gz` |
| MessagePack | `.msgpack`, `.msgpack.gz` (default binary) |

Legacy pickle (`.p` / `.pkl` / `.pickle`) is rejected; re-export from Python as
JSON or CSV first if needed.

### Quick examples

```bash
dqlrs --version
dqlrs -c "ls"
dqlrs --json -c "SCAN * FROM mytable LIMIT 5"
dqlrs -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
DQL_BACKEND=memory dqlrs -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
```

---

## For `rust-impl` maintainers

Work from this directory (`rust-impl/`).

### Workspace layout

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
```

Default CLI backend is live AWS. `DQL_BACKEND=memory` is for offline demos and
smoke tests. Engine unit tests use `MemoryBackend` directly. DynamoDB Local is
selected with `-H` / `-p`.

### Local checks

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

### DynamoDB Local tests

Integration tests expect Local on port 8000 (override with `DQL_LOCAL_HOST` /
`DQL_LOCAL_PORT`). They run with the normal suite and fail if Local is down.

```bash
# start DynamoDB Local, then:
cargo test -p dql-engine --test dynamodb_local_smoke
cargo test -p dql-engine --test dynamodb_local_parity
cargo test --workspace
cargo run -p dql-cli -- -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
```

Harness: `crates/dql-engine/tests/support/mod.rs` (`LocalHarness`).

### Toolchain

See `rust-toolchain.toml`. CI: `.github/workflows/rust-workflows.yml` (fmt,
clippy, tests, smoke).
