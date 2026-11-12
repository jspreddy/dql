# TODO: Split monolith modules

**Priority:** High  
**Smell / SOLID:** God files, SRP  
**Crates:** `dql-parser`, `dql-expr`, `dql-models`, `dql-engine`

## Problem

Several crates are single-file (or near-single-file) modules that mix unrelated
responsibilities. Navigation, review, and incremental change are expensive.

| File | ~LOC | Mixed concerns |
| --- | --- | --- |
| `dql-parser/src/lib.rs` | 2,150 | Lexer, AST, all statement parsers, update-string capture, tests |
| `dql-engine/src/engine.rs` | 1,260 | Dispatch, planning, explain, throttle, SAVE, projection/sort |
| `dql-engine/src/aws.rs` | 1,240 | SDK client, DDL, reads, writes, pagination, cache, errors |
| `dql-expr/src/lib.rs` | 1,225 | Render, placeholders, value conversion, selection evaluator |
| `dql-models/src/lib.rs` | 930 | Metadata, planning, schema DDL strings, throughput display |

## Recommendation

1. **`dql-parser`:** Split into `ast`, `lexer`, `error`, and statement modules
   (`query`, `dml`, `ddl`). Keep `lib.rs` as a thin re-export facade.
2. **`dql-expr`:** Split into `render/`, `values/`, `timestamps/`; move
   `project_selection` / runtime evaluation toward engine or a small eval
   module (see also `todo_typed_expression_ast.md`).
3. **`dql-models`:** Keep structural metadata + `plan_read` here; move
   `schema_dql` / display helpers to `dql-output` (or a schema-format module).
4. **`dql-engine`:** Decompose `Engine` and `lib.rs` as described in
   `todo_engine_backend_boundaries.md`; split `aws.rs` read/write/DDL helpers.

## Acceptance

- No source file over ~600 LOC without a clear single responsibility.
- Public API unchanged (re-exports from crate roots).
- Existing workspace tests still pass.
