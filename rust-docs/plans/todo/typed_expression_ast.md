# TODO: Typed expression AST end-to-end

**Priority:** High  
**Smell / SOLID:** Architecture mismatch, duplicated parsing, fragile tokenization  
**Crates:** `dql-parser`, `dql-expr`, `dql-models`, `dql-engine`

## Problem

`architecture.md` calls for typed expression nodes. In code, several expression
surfaces are still opaque strings that downstream crates re-tokenize:

- `SelectionItem.expression: String` — SELECT projections captured via
  `collect_until` / comma splitting
- UPDATE bodies via `parse_update_expr_raw` / whitespace tokenization
- `dql-expr::render_raw_expression` / `render_function_expression` split on
  whitespace and `(`
- Field extraction duplicated in `dql-expr::extract_fields` and
  `dql-models::selection_fields`

Nested calls, quoted strings with spaces, map/list literals, and dashed paths
are easy to mis-tokenize. Planner and renderer can diverge.

## Recommendation

1. Introduce typed `Expr` / `Path` / `UpdateClause` AST nodes in `dql-parser`
   (or a shared `dql-ast` if the surface grows).
2. Parse selections and updates structurally once at parse time.
3. Have `dql-expr` render and evaluate from that AST only — no second
   whitespace tokenizer.
4. Centralize `selection_referenced_fields()` on the typed selection AST for
   both planning and projection rendering.
5. Keep `QueryOptions.save_file` out of core DynamoDB option semantics if
   practical (CLI/engine wrapper), or document it as an intentional
   statement-level modifier.

## Acceptance

- Selection and update expressions are typed in the parser AST.
- No `split_whitespace`-based update rendering for production paths.
- One shared field-reference extractor used by models and expr.
- Parity tests for reserved words, dashed paths, nested maps, and functions
  still pass.
