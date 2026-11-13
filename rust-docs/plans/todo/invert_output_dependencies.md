# TODO: Invert output-layer dependencies

**Priority:** High  
**Smell / SOLID:** DIP (presentation depends on execution), duplicated table
detail formatting, shallow `Format` trait  
**Crates:** `dql-output`, `dql-engine`, `dql-cli`, `dql-models`

## Problem

### Wrong dependency direction

`dql-output` depends on `dql-engine` for `Item`, `StatementResult`, and
`json_util`. Formatting cannot be used without pulling in the executor.

### Split rich presentation

- `dql-output` has `formats/rich.rs` (layout + text fallback)
- `dql-cli/repl/rich_table.rs` scrapes ratatui `Table` via `TestBackend` into
  `Line`s
- `meta/ls` and query rich paths duplicate table-detail layout vs
  `table_meta::format_table_detail`

### Format API

`Format` is a thin `display(&mut dyn Write)` trait; `OutputConfig::formatter()`
is a closed match. `JsonFormat` ignores `lossy_json_float`. `SmartFormat`
duplicates column-width logic from `ColumnFormat`. `write_paged` is dead code.

### Models purity

`format_throughput` / `schema_dql` live in `dql-models` and are re-exported
through `dql-engine`, mixing planning with presentation.

## Recommendation

1. Introduce a minimal row/value view type in `dql-parser` or a tiny
   `dql-types` crate. `dql-output` depends only on that (+ `dql-models` for
   table meta views if needed). Engine/CLI adapt `StatementResult` at the
   boundary.
2. Move “layout → ratatui `Line`s” into `dql-output::formats::rich` with a
   shared `RichRenderer` API; delete or thin `repl/rich_table.rs`.
3. Build one `TableDetailView` (data extraction) rendered by plain-text and
   TUI backends.
4. Prefer an enum `Formatter` or registry over trait objects; wire
   `lossy_json_float`; share width helpers between smart and column.
5. Keep `format_throughput` in one place (`dql-models` or `dql-output`); drop
   engine passthrough if unused.

## Acceptance

- `dql-output/Cargo.toml` does not depend on `dql-engine`.
- Rich query and rich `ls` share one renderer module in `dql-output`.
- CLI REPL only chooses when to call the renderer, not how to lay out tables.
