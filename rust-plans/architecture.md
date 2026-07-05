# Target Rust Architecture

The Rust rewrite should keep DQL split by responsibility. The current Python
code mixes some parser, execution, formatting, and REPL concerns, so the Rust
structure should make those boundaries explicit from the start.

## Proposed workspace layout

```text
crates/
  dql-cli/       # clap arguments, REPL, config, history, meta-commands
  dql-parser/    # grammar, lexer/parser, typed AST
  dql-expr/      # expressions, visitors, placeholders, value coercion
  dql-models/    # table/index metadata and query-planning helpers
  dql-engine/    # statement execution, DynamoDB SDK integration
  dql-output/    # table, JSON, expanded, smart, and pager output
```

The public binary should be a thin shell around `dql-cli`. Library users should
be able to depend on `dql-engine` and `dql-parser` without pulling in the REPL.

## Current-to-target mapping

| Python area | Rust target | Notes |
| --- | --- | --- |
| `dql/__init__.py` | `dql-cli/src/main.rs` | Use `clap` for `-c`, `--json`, region, host, port, and version flags. |
| `dql/cli.py` | `dql-cli` | Use `rustyline` for history, completion, and multiline input. Keep meta-commands separate from DQL statements. |
| `dql/grammar/` | `dql-parser` | Prefer a grammar-first parser such as `pest`, or a typed combinator parser if error recovery needs more control. |
| `dql/expressions/` | `dql-expr` | Build typed constraint, selection, and update expressions; render DynamoDB expression strings and placeholders. |
| `dql/models.py` | `dql-models` | Represent `TableMeta`, `QueryIndex`, fields, projections, throughput, and billing mode as Rust structs. |
| `dql/engine.py` | `dql-engine` | Dispatch typed statements, plan queries, call `aws-sdk-dynamodb`, track capacity, and handle throttling. |
| `dql/output.py` | `dql-output` | Use terminal-aware formatting crates while preserving JSON and tabular output semantics. |
| `dql/history.py` | `dql-cli` | Store and append history with the same user-visible behavior where practical. |
| `dql/help.py` | `dql-cli` | Embed or generate command help from markdown or RST source. |
| `dql/monitor.py` | `dql-cli` or optional feature | Keep CloudWatch/watch support optional until the core engine is stable. |

## Key design boundaries

### Parser and AST

The parser should return a typed AST, not a generic parse tree. Each statement
variant should hold already-normalized names, literals, options, and expression
nodes. This avoids recreating Python's `ParseResults` coupling in Rust.

### Expression rendering

Expression rendering should be its own service that produces:

- DynamoDB expression strings
- expression attribute names
- expression attribute values

This module must preserve reserved-word escaping, dashed or underscored path
handling, list indexing, nested map paths, and stable placeholder generation.

### Query planning

Index selection should live in `dql-models` or a small planner module consumed
by `dql-engine`. The planner should decide:

- whether a statement can use `query` or must use `scan`
- which table, local index, or global index matches constraints
- whether projected index attributes are enough
- when a follow-up batch get is required
- whether `SELECT` must reject a scan because `allow_select_scan` is false

### DynamoDB client layer

The Rust SDK is lower level than `dynamo3`, so the rewrite needs a narrow
adapter over `aws-sdk-dynamodb` for:

- table creation, update, deletion, and description
- query, scan, batch get, batch write, update item, and delete item
- capacity tracking
- retry and pagination behavior
- DynamoDB Local endpoint configuration

### CLI and REPL

The CLI should preserve both one-shot and interactive workflows. The REPL should
route shell/meta-commands before DQL parsing, keep multiline statement support,
and expose configuration for display, format, page size, width, throttling, and
scan safety.

### Output

Output should be treated as compatibility-sensitive. JSON output is easiest to
compare mechanically and should be ported first. Smart, column, expanded, rich,
and pager behavior can follow once the engine returns compatible values.

## Initial dependency candidates

- CLI parsing: `clap`
- REPL and history: `rustyline`
- AWS access: `aws-config`, `aws-sdk-dynamodb`, and optionally
  `aws-sdk-cloudwatch`
- Numeric values: `rust_decimal`
- Time parsing: `chrono` plus a small compatibility layer for current interval
  syntax
- JSON: `serde`, `serde_json`
- Table output: `comfy-table` or custom formatting if parity requires it
- Rate limiting: `governor` or a simple token bucket tailored to DynamoDB
  capacity units
