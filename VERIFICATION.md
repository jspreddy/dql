# Verification

Commands to confirm both implementations still **install**, **lint**, **test**, and **build** after the `py-*` / `rust-*` split. Run them from the **repository root**. Java is required for DynamoDB Local (tests).

## DynamoDB Local (port 8000)

Python pytest and Rust Local tests both need Local on **8000**. Start it in a **separate terminal** and leave it running (a `… &` child is killed when that shell exits).

```bash
./scripts/install_dynamodb_local.sh
```

That script downloads the jar into `.dynamo-local/` on first run, then starts Java in the foreground. Confirm:

```bash
nc -z localhost 8000 && echo "DynamoDB Local is up"
```

To run Local in the background from an already-open terminal that you will keep:

```bash
java -Djava.library.path="$PWD/.dynamo-local/DynamoDBLocal_lib" \
  -jar "$PWD/.dynamo-local/DynamoDBLocal.jar" -inMemory -sharedDb
```

---

## Python (`py-impl/`)

Requires [uv](https://docs.astral.sh/uv/).

```bash
cd py-impl

# Install
uv sync --dev

# Lint (mypy, isort, black, pylint)
uv run task lint

# Test (needs DynamoDB Local on :8000)
uv run task test

# Build sdist + wheel
uv build
```

Optional: `uv run task package` also builds a pex binary at `py-impl/build/dql`.

---

## Rust (`rust-impl/`)

Requires a stable Rust toolchain (`rust-impl/rust-toolchain.toml`).

```bash
cd rust-impl

# Install / resolve crates (Cargo.lock)
cargo fetch --locked

# Lint
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings

# Test (workspace tests; Local integration needs :8000)
cargo test --workspace
cargo test -p dql-engine --test dynamodb_local_smoke
cargo test -p dql-engine --test dynamodb_local_parity

# Build release CLI
cargo build --release -p dql-cli

# Smoke the binary (DQL_REQUIRE_LOCAL=1 fails if Local is down)
DQL_REQUIRE_LOCAL=1 ../scripts/rust-smoke-test.sh
```

---

## Acceptance (binaries + Local)

Black-box cases under [`black-box-tests/`](black-box-tests/) spawn `dql` / `dqlrs` and
assert CLI output. They do not import either implementation. Local must be on
**8000** (same as above).

```bash
# Point at built binaries if they are not on PATH
export DQL_BIN="$PWD/py-impl/.venv/bin/dql"          # after: cd py-impl && uv sync --dev
export DQLRS_BIN="$PWD/rust-impl/target/release/dqlrs"  # after: cargo build --release -p dql-cli

./black-box-tests/harness/run.sh
./black-box-tests/harness/run.sh --bin dqlrs
./black-box-tests/harness/run.sh --start-local   # starts Local only if the port is down
```

Details and the case file convention: [`black-box-tests/README.md`](black-box-tests/README.md).

---

## One-shot (copy-paste from repo root)

Foreground Local would block, so this starts Java in the same shell, waits for port 8000, then runs both trees:

```bash
set -euo pipefail

JAR_DIR=".dynamo-local"
if [[ ! -f "$JAR_DIR/DynamoDBLocal.jar" ]]; then
  ./scripts/install_dynamodb_local.sh background
else
  java -Djava.library.path="$JAR_DIR/DynamoDBLocal_lib" \
    -jar "$JAR_DIR/DynamoDBLocal.jar" -inMemory -sharedDb &
fi
for i in $(seq 1 30); do nc -z localhost 8000 && break; sleep 1; done
nc -z localhost 8000

( cd py-impl && uv sync --dev && uv run task lint && uv run task test && uv build )

( cd rust-impl && cargo fetch --locked \
  && cargo fmt --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test --workspace \
  && cargo test -p dql-engine --test dynamodb_local_smoke \
  && cargo test -p dql-engine --test dynamodb_local_parity \
  && cargo build --release -p dql-cli \
  && DQL_REQUIRE_LOCAL=1 ../scripts/rust-smoke-test.sh )
```

---

## Results from this checkout (2026-08-20)

| Step | Result |
| --- | --- |
| `py-impl`: `uv sync --dev` | Pass |
| `py-impl`: `uv run task lint` | Pass |
| `py-impl`: `uv run task test` | **166 passed, 2 failed, 2 skipped** |
| `py-impl`: `uv build` | Pass (`dist/dql-0.6.4.dev12.tar.gz`, `.whl`) |
| `rust-impl`: `cargo fetch --locked` | Pass |
| `rust-impl`: `cargo fmt --check` | Pass (after rustfmt import order in `dql-cli` `error.rs`) |
| `rust-impl`: `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `rust-impl`: `cargo test --workspace` | Pass (a few tests ignored, as before) |
| `rust-impl`: Local smoke + parity tests | Pass (1 + 5) |
| `rust-impl`: `cargo build --release -p dql-cli` | Pass |
| `rust-impl`: `DQL_REQUIRE_LOCAL=1 ../scripts/rust-smoke-test.sh` | Pass |

Python failures (not caused by missing packages after the move; the suite loads from `py-impl/`):

- `tests/test_cli.py::TestCli::test_help_docs` — help text wrapped at a narrower width than the assertion string.
- `tests/test_cli.py::TestCliCommands::test_ls` — Rich `ls` snapshot is 200 columns in git; this run rendered ~80 columns (`conftest.py` sets `rich.get_console().width = 200`, but `ls` still used a narrower console here).
