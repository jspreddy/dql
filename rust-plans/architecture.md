# Target Rust Architecture

The Rust rewrite should keep DQL split by responsibility. The current Python
code mixes some parser, execution, formatting, and REPL concerns, so the Rust
structure should make those boundaries explicit from the start.

## Workspace layout (implemented)

```text
rust-impl/crates/
  dql-cli/       # clap arguments, ratatui TUI shell, config, meta-commands
  dql-parser/    # lexer/parser, typed statement AST
  dql-expr/      # expression rendering, placeholders, value coercion
  dql-models/    # table/index metadata and query-planning helpers
  dql-engine/    # statement execution, DynamoDB SDK + memory backends
  dql-output/    # smart, column, expanded, JSON, rich, pager, table meta
```

The public binary is a thin shell around `dql-cli`. Library users can depend on
`dql-engine` and `dql-parser` without pulling in the REPL.

### Known boundary gaps (tracked as todos)

Code is ahead of the ideal boundaries in a few places; see `todo_*.md`:

- Selection/update expressions are still partly opaque strings
(`todo_typed_expression_ast.md`).
- `dql-output` currently depends on `dql-engine` for result types
(`todo_invert_output_dependencies.md`).
- Several crates are large single-file modules
(`todo_split_monolith_modules.md`).
- CLI one-shot and REPL still have divergent execution paths
(`todo_unify_cli_pipelines.md`).



## Python-to-Rust mapping


| Python area        | Rust target                   | Notes                                                                                                                                                               |
| ------------------ | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `py-impl/dql/__init__.py`  | `dql-cli/src/main.rs`         | Use `clap` for `-c`, `--json`, region, host, port, and version flags.                                                                                               |
| `py-impl/dql/cli.py`       | `dql-cli`                     | Use `ratatui` for the interactive terminal UI, with a line-input/history layer for completion and multiline input. Keep meta-commands separate from DQL statements. |
| `py-impl/dql/grammar/`     | `dql-parser`                  | Prefer a grammar-first parser such as `pest`, or a typed combinator parser if error recovery needs more control.                                                    |
| `py-impl/dql/expressions/` | `dql-expr`                    | Build typed constraint, selection, and update expressions; render DynamoDB expression strings and placeholders.                                                     |
| `py-impl/dql/models.py`    | `dql-models`                  | Represent `TableMeta`, `QueryIndex`, fields, projections, throughput, and billing mode as Rust structs.                                                             |
| `py-impl/dql/engine.py`    | `dql-engine`                  | Dispatch typed statements, plan queries, call `aws-sdk-dynamodb`, track capacity, and handle throttling.                                                            |
| `py-impl/dql/output.py`    | `dql-output`                  | Use terminal-aware formatting crates while preserving JSON and tabular output semantics.                                                                            |
| `py-impl/dql/history.py`   | `dql-cli`                     | Store and append history with the same user-visible behavior where practical.                                                                                       |
| `py-impl/dql/help.py`      | `dql-cli`                     | Embed or generate command help from markdown or RST source.                                                                                                         |
| `py-impl/dql/monitor.py`   | `dql-cli` or optional feature | Keep CloudWatch/watch support optional until the core engine is stable.                                                                                             |




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

**Current gap:** projections and update RHS are often stored/rendered as
strings with ad hoc tokenization. Target state is a typed expression AST shared
by parser, planner, and renderer (`todo_typed_expression_ast.md`).

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
scan safety. The interactive mode should be built as a `ratatui` application so
DQL can grow from a prompt-compatible REPL into a richer terminal UI for command
history, table browsing, help, query results, and watch/monitor views.

### Output

Output lives in `dql-output` and is compatibility-sensitive. Implemented
formats: JSON, smart, column, expanded, rich (text fallback + REPL ratatui
lines), and pager/`less` for non-TUI paths. One-shot command output remains
pipe-friendly.

**Current gap:** presentation still depends on `dql-engine` types, and rich
ratatui scraping lives partly under `dql-cli/repl`
(`todo_invert_output_dependencies.md`).

## Initial dependency candidates

- CLI parsing: `clap`
- Interactive CLI/TUI: `ratatui` with a terminal backend such as `crossterm`
- Line editing and history: integrate a line-input/history layer compatible
with the `ratatui` event loop
- AWS access: `aws-config`, `aws-sdk-dynamodb`, and optionally
`aws-sdk-cloudwatch`
- Numeric values: `rust_decimal`
- Time parsing: `chrono` plus a small compatibility layer for current interval
syntax
- JSON: `serde`, `serde_json`
- Table output: `ratatui` widgets for interactive views, plus custom
pipe-friendly formatting for non-interactive output if parity requires it
- Rate limiting: `governor` or a simple token bucket tailored to DynamoDB
capacity units

