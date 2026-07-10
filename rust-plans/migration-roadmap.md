# Migration Roadmap

The rewrite should progress through compatibility gates. Each phase should leave
behind runnable code, migrated tests, and a clear comparison point against the
Python implementation.

## Phase 1: Language core [DONE]

Build the Rust parser and typed AST before touching DynamoDB execution.

Deliverables:

- Statement AST for `SELECT`, `SCAN`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`,
  `DROP`, `ALTER`, `DUMP`, `LOAD`, `EXPLAIN`, and `ANALYZE`
- Literal parsing for strings, numbers, booleans, nulls, lists, maps, sets,
  timestamps, and intervals
- Constraint, selection, and update expression ASTs
- Multistatement parsing with semicolon handling
- Fragment parser for REPL multiline input

Compatibility gate:

- Port parser test cases from `tests/test_parser.py`.
- Add golden AST snapshots for documentation examples from `README.rst` and
  `doc/topics/queries/`.

Primary risks:

- The current grammar depends on old `pyparsing` parse result shapes.
- Update and constraint expressions have nested behavior that should become
  explicit typed AST nodes in Rust.

## Phase 2: Expression and value compatibility [DONE]

Implement DynamoDB expression rendering without making live AWS calls.

Deliverables:

- Field path escaping and reserved-word placeholder generation
- Expression value placeholder generation
- Selection projection rendering
- Update expression rendering
- DQL literal to DynamoDB attribute value conversion
- JSON-safe value serialization rules, including decimal behavior

Compatibility gate:

- Port focused expression tests and add round-trip tests for representative
  values.
- Compare rendered expressions against the Python visitor for reserved words,
  dashed names, underscored names, list indexes, and nested paths.

Primary risks:

- DynamoDB expression syntax is strict and errors often appear only at runtime.
- Python `Decimal`, binary, set, timestamp, and interval handling needs precise
  Rust equivalents.

## Phase 3: Metadata and query planning [DONE]

Port table/index models and the logic that decides which DynamoDB operation to
run.

Deliverables:

- Table metadata models for keys, fields, LSIs, GSIs, projections, throughput,
  billing mode, and status
- `QueryIndex` equivalent with projection checks
- Index matching and scan rejection rules
- Planning output that explains selected operation, index, filters, limits, and
  any required follow-up batch get

Compatibility gate:

- Port `tests/test_models.py`.
- Add planner-only tests extracted from query scenarios before connecting the
  AWS SDK.

Primary risks:

- Index matching is subtle and affects both correctness and cost.
- Partial index projections require a second read path for full item attributes.

## Phase 4: DynamoDB execution engine [DONE]

Connect the typed statement executor to DynamoDB Local and then to AWS.

Deliverables:

- DynamoDB Local endpoint support via `SdkBackend` / `SdkConfig` and CLI `-H/-p` wiring
- Table lifecycle statements: `CREATE`, `DROP`, `ALTER`, `DUMP`
- Data statements: `INSERT`, `SELECT`, `SCAN`, `UPDATE`, `DELETE`, `LOAD` (JSON lines and CSV)
- Explain/analyze hooks for calls and consumed capacity
- Batch write chunking (25 items) with unprocessed-item retry; paginated query/scan
- Throttling primitives via token-bucket `RateLimit` on memory and SDK backends

Status:

- `dql-engine` is split into `memory`, `aws`, `convert`, `engine`, `json_util`, and
  `throttle` modules with a generic `Engine<B: DynamoBackend>` dispatcher
- `SdkBackend` implements create/drop/describe/list, batch write, query/scan pagination,
  update/delete by key, and ALTER TABLE operations against `aws-sdk-dynamodb`
- `MemoryBackend` covers UPDATE, ALTER, index planning, FilterExpression, selection
  arithmetic, KEYS IN, DELETE USING, keyword INSERT, CSV LOAD, and explain kwargs
- CLI connects to DynamoDB Local when `-H` is provided
- Integration tests `dynamodb_local_smoke` and `dynamodb_local_parity` require
  DynamoDB Local and fail if it is not running
- `python_engine_query_model_parity` mirrors `tests/test_queries.py` with 128 passing
  tests; FragmentEngine and remaining regressions stay deferred to Phase 5

Compatibility gate:

- Port `tests/test_engine.py` and `tests/test_queries.py` incrementally by
  statement family.
- Run the Rust and Python suites against DynamoDB Local with the same fixture
  data where practical.

Primary risks:

- `dynamo3` currently hides several behaviors that must be rebuilt over the
  Rust SDK.
- Capacity reporting, pagination, and batch retry behavior must be deliberate.

## Phase 5: CLI, REPL, and output

Build the user-facing binary around the stable parser and engine.

Deliverables:

- `dql` binary with current flags and environment defaults
- Interactive `ratatui` terminal UI with multiline fragments, history,
  completion, and prompt behavior
- Meta-commands: `use`, `local`, `ls`, `file`, `opt`, `throttle`,
  `unthrottle`, `watch`, `whoami`, `shell`, `clear`, `cls`, `c`, `exit`, and
  `version`
- Output formats: JSON first, then smart, column, expanded, rich, and pager
  display; use `ratatui` widgets for interactive result browsing where useful
- Help text for DQL statements and options

Compatibility gate:

- Port `tests/test_cli.py`, `tests/test_history.py`, and output-focused tests.
- Snapshot command output for common one-shot and REPL flows.

Primary risks:

- Terminal formatting is user-visible; the `ratatui` UI should preserve
  pipe-friendly one-shot behavior while improving interactive navigation.
- `watch` and CloudWatch metrics can be isolated behind optional features if
  they slow core parity work.

## Phase 6: Packaging and release path

Replace the Python installation and pex story with Rust-native packaging.

Deliverables:

- Cargo workspace with locked dependencies
- Release binary build configuration
- Installation instructions for local development and published binaries
- Migration notes for users relying on Python-specific behavior

Compatibility gate:

- Package smoke test that runs `dql --version`, `dql -c`, and DynamoDB Local
  examples from a clean checkout.

Primary risks:

- Users may depend on pip/pex distribution.
- Platform-specific terminal and TLS behavior should be validated before a
  broad release.

## Post–Phase 6: Remaining parity tracks

Phases 1–6 are implemented in `rust-impl/`. Remaining gaps versus Python are
tracked as separate plans (one commit-series each) under `.cursor/plans/`:

1. [Default AWS connection](../.cursor/plans/parity_1_default_aws.plan.md)
2. [ORDER BY / ScanIndexForward](../.cursor/plans/parity_2_order_by.plan.md)
3. [Index projection + count](../.cursor/plans/parity_3_index_projection.plan.md)
4. [Expression regressions](../.cursor/plans/parity_4_expression_regressions.plan.md)
5. [SAVE/LOAD + MessagePack](../.cursor/plans/parity_5_save_load.plan.md) (pickle replaced by MessagePack)
6. [CLI polish](../.cursor/plans/parity_6_cli_polish.plan.md)
