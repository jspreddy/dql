# Testing and Parity Strategy

The Rust rewrite should prove compatibility with the Python implementation
before the Python code is retired. The test strategy should migrate existing
coverage in an order that follows the rewrite phases.

## Success criteria

- DQL examples in `README.rst` and `doc/topics/queries/` parse and execute with
  the same user-visible behavior.
- Parser tests cover every supported statement family and important error case.
- Query tests pass against DynamoDB Local for table lifecycle, data mutation,
  reads, scans, index usage, dumping, loading, explain, and analyze behavior.
- CLI tests cover one-shot command execution, REPL commands, output formats,
  history, and configuration.
- JSON output is byte-stable or intentionally documented where ordering differs.

## Test migration order

### 1. Parser golden tests

Start with pure parser tests because they require no AWS setup and define the
language contract.

Sources to port:

- `tests/test_parser.py`
- statement examples from `README.rst`
- query documentation under `doc/topics/queries/`

Rust test shape:

- Parse input into typed AST.
- Compare against checked-in snapshots.
- Include negative tests for invalid syntax.
- Keep semicolon and multiline fragment tests separate from full statement AST
  tests.

### 2. Expression rendering tests

Expression rendering should be verified before any live DynamoDB calls.

Sources to port:

- `dql/expressions/`
- expression assertions currently embedded in parser and query tests

Rust test shape:

- Build AST nodes directly and through the parser.
- Assert rendered expression strings, attribute names, and attribute values.
- Cover reserved words, dashed names, underscored names, nested paths, indexes,
  comparison operators, functions, `BETWEEN`, `IN`, and update clauses.

### 3. Model and planner tests

Planner tests should run without DynamoDB by using table metadata fixtures.

Sources to port:

- `tests/test_models.py`
- index-sensitive cases from `tests/test_queries.py`

Rust test shape:

- Build table and index metadata fixtures.
- Assert selected operation, selected index, key condition, filter condition,
  projection coverage, and scan rejection decisions.
- Include cases for explicit `USING`, local indexes, global indexes, projected
  attributes, `KEYS IN`, ordering, limits, and `allow_select_scan`.

### 4. DynamoDB Local integration tests

Once parser, expression rendering, and planning are stable, run statement tests
against DynamoDB Local.

Sources to port:

- `tests/test_engine.py`
- `tests/test_queries.py`
- shared setup from `tests/__init__.py` and `tests/conftest.py`

Rust test shape:

- Use a test harness that connects to DynamoDB Local on the configured port.
  These tests fail if Local is not running; they are not skipped automatically.
- Create unique table names per test to avoid cross-test contamination.
- Patch or disable CloudWatch metric calls when DynamoDB Local is used.
- Assert both returned data and explain/analyze call details.

### 5. CLI and output tests

CLI tests should come after the engine because they need stable behavior from
all lower layers.

Sources to port:

- `tests/test_cli.py`
- `tests/test_history.py`
- `tests/test_readline_compat.py`
- `tests/test_save.py`
- snapshots under `tests/__snapshots__/`

Rust test shape:

- Invoke the compiled binary for one-shot commands.
- Use integration helpers for REPL command streams.
- Snapshot JSON, smart, column, expanded, and rich output separately.
- Validate history append/clear behavior with temporary config directories.

## Python-to-Rust comparison harness

During migration, keep a small compatibility runner that can execute the same
fixture through both implementations.

Recommended comparisons:

- Parse result: Python parse result normalized to JSON vs Rust typed AST
  normalized to JSON.
- Engine result: returned items and counts against DynamoDB Local.
- Explain result: ordered DynamoDB operation names and important request fields.
- CLI result: stdout, stderr, and exit code for representative commands.

This harness can be removed once the Rust implementation is the only supported
runtime.

## CI gates

Minimum CI before replacing the Python package:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- DynamoDB Local integration suite
- Binary smoke tests for `--version`, `-c`, `--json`, and local endpoint flags

## Manual validation checklist

Before an early Rust release, manually verify:

- `dql` opens a REPL and exits cleanly.
- Multiline `CREATE TABLE` and `INSERT` statements complete after `;`.
- `SELECT` rejects scans unless the scan option is enabled.
- JSON output can be piped to another process.
- `local` and `use` switch connection targets.
- `help`, `opt`, `throttle`, and `ls` remain discoverable.
- Terminal width and paging behavior are acceptable in a normal shell.

## Risk-focused regression areas

- Parser edge cases around nested expressions and statement termination.
- Decimal precision and lossy JSON float behavior.
- DynamoDB reserved-word escaping and placeholder stability.
- Index choice when several indexes satisfy a query.
- Batch get/write retries and partial failures.
- Output ordering and set serialization.
- History persistence and config directory isolation in tests.
