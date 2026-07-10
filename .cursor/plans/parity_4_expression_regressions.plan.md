---
name: Parity 4 Expression Regressions
overview: "Fix reserved-word and dashed-field path regressions by extending the parser to accept DynamoDB-style attribute names (hyphens) and ensuring the full reserved-word set is applied at render time. Un-ignore test_filter_banned_word and test_filter_with_dash."
todos:
  - id: step-1-dashed-paths
    content: "Step 1: Parser accepts hyphenated field paths (WHERE, INSERT, UPDATE)"
    status: pending
  - id: step-2-reserved-words
    content: "Step 2: Full DynamoDB reserved-word list in dql-expr Visitor"
    status: pending
  - id: step-3-memory-resolve
    content: "Step 3: Memory compare_operand uses resolve_field_value"
    status: pending
  - id: step-4-unignore-tests
    content: "Step 4: Un-ignore test_filter_banned_word and test_filter_with_dash"
    status: pending
isProject: false
---

# Parity 4: Expression Regressions (Reserved Words / Dashed Paths)

## Current state

`dql-expr` already escapes reserved words, dashes, and leading underscores **per path segment** (`Visitor`, unit test `visitor_escapes_reserved_dashed_and_underscored_segments`). The main gap is **parsing**:

- Tokenizer treats `-` as `Token::Symbol('-')`, so `my-field` becomes three tokens.
- `parse_field_path` / keyword INSERT keys use `expect_ident()` only.
- Python allows hyphens in names: `Word(alphas + "_", alphanums + "_-.[]")` (`dql/grammar/common.py`) and `FIELD_RE` in `dql/expressions/visitor.py`.

`test_filter_banned_word` (`hash` attribute) may already render correctly once the full reserved-word set matches `dynamo3.constants.RESERVED_WORDS`. `test_filter_with_dash` cannot pass until the parser accepts `my-field`.

Ignored tests:

| Test | Reason |
| --- | --- |
| `test_filter_banned_word` | reserved-word escaping |
| `test_filter_with_dash` | dashed field path support |

## Goal

- Parse and round-trip attributes like `my-field`, `hash`, and nested `a.b-c`.
- Render ExpressionAttributeNames for every reserved / dashed / underscored segment.
- Memory and AWS backends evaluate the same filters.

## Design decisions

- Prefer extending **`parse_field_path`** (and INSERT key parsing) to consume `- ident` segments over inventing a new lexer token type — smaller blast radius.
- Embed the **full DynamoDB reserved-word list** as a static set in `dql-expr` (generate once from AWS docs / dynamo3 list).
- Do not change string-literal or binary syntax.

## Implementation steps (one commit each)

### Step 1 — Hyphenated field paths in the parser

**Files**

- [`rust-impl/crates/dql-parser/src/lib.rs`](rust-impl/crates/dql-parser/src/lib.rs) — `parse_field_path`, keyword INSERT, UPDATE attribute names, WHERE operands
- [`rust-impl/crates/dql-parser/tests/python_parser_parity.rs`](rust-impl/crates/dql-parser/tests/python_parser_parity.rs)

**Work**

- After an ident, allow zero or more `-` + ident segments before `.` nesting continues.
- Apply in: constraints, selections, update SET/REMOVE/ADD/DELETE targets, INSERT keyword keys.
- Add parser unit tests: `WHERE my-field = 1`, `INSERT INTO t (id='a', my-field=1)`, `SET my-field = 2`.

**Gate:** Parser tests green; no regressions on arithmetic `a - b` in selection expressions (disambiguate carefully — selection arithmetic vs field names).

**Commit:** `feat(parser): accept hyphenated DynamoDB attribute names`

---

### Step 2 — Full reserved-word set

**Files**

- [`rust-impl/crates/dql-expr/src/lib.rs`](rust-impl/crates/dql-expr/src/lib.rs) — `default_reserved_words`
- Optional: `rust-impl/crates/dql-expr/src/reserved_words.rs` generated list

**Work**

- Replace the short default list with the complete DynamoDB reserved words (case-insensitive match as today).
- Keep existing escape rules for `-` and leading `_`.
- Unit test: `hash`, `TIMESTAMP`, `STATUS`, etc. become `#n0`-style names.

**Gate:** `visitor_escapes_*` tests still pass; new cases for previously missing words.

**Commit:** `fix(expr): use full DynamoDB reserved-word list`

---

### Step 3 — Memory field resolution

**Files**

- [`rust-impl/crates/dql-engine/src/memory.rs`](rust-impl/crates/dql-engine/src/memory.rs) — `compare_operand`, `matches_condition`

**Work**

- For `ConditionOperand::Field`, use the same nested/dashed path resolver as projections (`resolve_field_value` or shared helper) instead of flat `item.get(field)`.
- Ensures dashed and nested filters behave in memory tests.

**Gate:** In-memory filter on `my-field` and nested paths.

**Commit:** `fix(memory): resolve dashed and nested fields in filters`

---

### Step 4 — Un-ignore regression tests

**Files**

- [`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`](rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs)

**Work**

- Implement `test_filter_banned_word` and `test_filter_with_dash` from `tests/test_queries.py::TestRegressions`.
- Remove `#[ignore]`.

**Gate:** `cargo test -p dql-engine --test python_engine_query_model_parity test_filter_banned_word test_filter_with_dash`

**Commit:** `test(engine): enable reserved-word and dashed-field regressions`

## Out of scope

- Bracket indexing `foo[0]` beyond what Python already supports in Rust
- Changing JSON key encoding

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| `a-b` ambiguous with subtraction | Only treat `-` as name continuation between idents with no whitespace (match Python tokenizer behavior) |
| Reserved-word list drift | Comment source URL / dynamo3 version; regenerate when AWS updates |
