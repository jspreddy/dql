# DQL Rust Implementation

[![Check](https://github.com/jspreddy/dql/actions/workflows/check.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/check.yml)

Maintainer guide for the Cargo workspace that builds `dqlrs`. User install,
flags, and examples: [`rust-docs/`](../rust-docs/). Python-to-Rust differences:
[`rust-docs/migration-from-python.md`](../rust-docs/migration-from-python.md).

This crate lives in [jspreddy/dql](https://github.com/jspreddy/dql), a fork of
[stevearc/dql](https://github.com/stevearc/dql).

## Target state

The target state of this repository is Rust, not Python. `dqlrs` is the primary
tool. Python remains for the original `dql` client and for integrations where
Rust is not feasible. New `dqlrs` features, including the web UI, are
implemented in Rust.

| Crate | Role |
| --- | --- |
| `dql-cli` | `dqlrs` binary: clap flags, meta-commands, ratatui REPL, `dqlrs web` |
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
```

Helpers live at the repo root: [`../scripts/`](../scripts/) (`rust-smoke-test.sh`, `install_dynamodb_local.sh`, `rust-install.sh`, `bump-version.sh`).

Default CLI backend is live AWS. `DQL_BACKEND=memory` is for offline demos and
smoke tests. Engine unit tests use `MemoryBackend` directly. DynamoDB Local is
selected with `-H` / `-p`.

## Local checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p dql-cli
../scripts/rust-smoke-test.sh
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
../scripts/install_dynamodb_local.sh background
cargo test -p dql-engine --test dynamodb_local_smoke
cargo test -p dql-engine --test dynamodb_local_parity
cargo test --workspace
cargo run -p dql-cli -- -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
```

Harness: `crates/dql-engine/tests/support/mod.rs` (`LocalHarness`).

## Toolchain

See `rust-toolchain.toml`. CI: `.github/workflows/check.yml` runs `just check`
(Python and Rust lint) on every merge into `v-rust`.

## Versioning

Python and Rust versions bump independently. Rust uses
[cargo-release](https://github.com/crate-ci/cargo-release) with
[`release.toml`](release.toml); install it once with
`cargo install cargo-release --locked`. The version lives only in `Cargo.toml`
(`[workspace.package]`); every crate inherits it. From the repository root:

```bash
just version -- rust --dry-run patch   # print what would change
just version rust patch                # 0.7.10 -> 0.7.11
just version rust minor                # 0.7.10 -> 0.8.0
just version rust alpha                # pre-release: 0.7.10 -> 0.7.11-alpha.1, then -alpha.2
just version rust beta                 # also rc
just version rust release              # drop the pre-release: 0.7.11-alpha.2 -> 0.7.11
just version rust 0.8.0-rc.1           # or set an exact version
just version -- rust release --tag     # also tag dqlrs-<version>
```

Pre-release levels only move forward (alpha, then beta, then rc); `release`
drops the suffix. Versions use semver pre-release identifiers (`-alpha.1`), not
the old `-devN` builds.

Each bump is one commit, `Bump version: OLD → NEW`, holding `Cargo.toml` and
`Cargo.lock`; [`scripts/changelog.py`](../scripts/changelog.py) starts a
changelog section at it. Nothing is published or pushed. Release tags are
`dqlrs-<version>` so they do not collide with Python `python-<version>` tags.

### Releasing from GitHub

**Actions → Release dqlrs → Run workflow**
([`release.yml`](../.github/workflows/release.yml)) releases without a checkout:

| Input | What it does |
| --- | --- |
| `level` | `patch`, `minor`, `major`, `alpha`, `beta`, `rc` or `release`, as above |
| `version` | An exact version such as `0.8.0` or `0.8.0-rc.1`; overrides `level` |
| `dry_run` | Shows the change in the job log without opening a pull request or tagging |

The job runs `scripts/bump-version.sh`, regenerates `CHANGELOG.md`, and for a
stable release points the README install commands at the new tag
(`scripts/release_pins.py`). `v-rust` takes changes only through pull
requests, so the job opens **Release dqlrs NEW**, merges it itself, and tags
the merge commit `dqlrs-NEW`. A pre-release is tagged too, but the README keeps
pointing at the last stable release.

When `version` is the current version and has no tag yet, nothing is bumped:
the job tags the `v-rust` commit where that version landed.
