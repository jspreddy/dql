---
name: Parity 2 ORDER BY
overview: "Complete ORDER BY / ASC / DESC parity with Python: wire ScanIndexForward for range-key ordering, keep client-side sort for non-key ORDER BY, and un-ignore the three select parity tests."
todos:
  - id: step-1-parser-bare-order
    content: "Step 1: Parse bare ASC/DESC into QueryOptions (Python ordering clause)"
    status: pending
  - id: step-2-engine-policy
    content: "Step 2: Engine policy — ScanIndexForward vs client sort_items"
    status: pending
  - id: step-3-aws-scan-index-forward
    content: "Step 3: SdkBackend query sets scan_index_forward from plan"
    status: pending
  - id: step-4-memory-range-order
    content: "Step 4: MemoryBackend sorts by range key for query plans"
    status: pending
  - id: step-5-unignore-tests
    content: "Step 5: Implement and un-ignore test_order_by, test_order_by_index, test_reverse"
    status: pending
isProject: false
---

# Parity 2: ORDER BY / ASC / DESC

## Current state

- Parser supports `ORDER BY field [ASC|DESC]` (`QueryOptions.order_by` in `dql-parser`).
- Engine always post-sorts via `finalize_read_result` → `sort_items` (`dql-engine/src/engine.rs`).
- AWS `query_items` / `scan_items` **never set `ScanIndexForward`**; `ReadRequest.order_by` is unused on the wire.
- Python has **two** mechanisms (`dql/engine.py`):
  - Bare `ASC` / `DESC` after the query (grammar `ordering`) → `kwargs["desc"]` when ordering by index range key.
  - `ORDER BY field` → client-side sort when the field is not the range key.

Ignored tests in `python_engine_query_model_parity.rs`:

| Test | Reason |
| --- | --- |
| `test_reverse` | needs ORDER BY DESC support |
| `test_order_by` | needs ORDER BY support |
| `test_order_by_index` | needs ORDER BY with index support |

## Goal

Match Python `order()` / `_select` behavior:

1. Bare `DESC`/`ASC` on a **query** whose sort key is the index/table range key → DynamoDB `ScanIndexForward`.
2. Bare `DESC`/`ASC` on a **scan** without `ORDER BY` → error (Python rejects this).
3. `ORDER BY <non-range-field>` → fetch then client `sort_items`.
4. `ORDER BY` + `KEYS IN` → already rejected; keep that.

## Design decisions

- Add `QueryOptions.descending: Option<bool>` (or reuse `OrderBy.descending` with a sentinel) for bare `ASC`/`DESC`.
- Prefer server-side order when it matches DynamoDB semantics; client sort only when necessary.
- EXPLAIN should surface `desc` / `scan_index_forward` for parity with Python explain output where practical.

## Implementation steps (one commit each)

### Step 1 — Parser: bare ASC / DESC

**Files**

- [`rust-impl/crates/dql-parser/src/lib.rs`](rust-impl/crates/dql-parser/src/lib.rs) — `QueryOptions`, `parse_query_options`
- [`rust-impl/crates/dql-parser/tests/python_parser_parity.rs`](rust-impl/crates/dql-parser/tests/python_parser_parity.rs)

**Work**

- Parse standalone `ASC` | `DESC` in query options (Python `ordering` in `dql/grammar/__init__.py`).
- Keep existing `ORDER BY field [ASC|DESC]`.
- Reject conflicting combinations if Python does (document if allowed).

**Gate:** Parser tests for `SELECT * FROM t WHERE id = 'a' DESC` and `… ORDER BY foo ASC`.

**Commit:** `feat(parser): accept bare ASC/DESC query ordering`

---

### Step 2 — Engine ordering policy

**Files**

- [`rust-impl/crates/dql-engine/src/engine.rs`](rust-impl/crates/dql-engine/src/engine.rs) — `execute_read`, `finalize_read_result`, validation helpers
- [`rust-impl/crates/dql-engine/src/lib.rs`](rust-impl/crates/dql-engine/src/lib.rs) — `ReadRequest` fields for descending / server-side order

**Work**

- Mirror Python `order()` (`dql/engine.py` ~668–676):
  - If action is query and (`order_by` is None or equals index range key) → set server-side descending flag; skip client sort for that case.
  - If `order_by` is a different field → client `sort_items` after fetch.
  - If scan + bare DESC/ASC without ORDER BY → `EngineError` / syntax error.
- Pass flags through `ReadRequest`.

**Gate:** Unit tests on policy helpers with synthetic `QueryPlan` + options.

**Commit:** `feat(engine): choose ScanIndexForward vs client ORDER BY`

---

### Step 3 — AWS `scan_index_forward`

**Files**

- [`rust-impl/crates/dql-engine/src/aws.rs`](rust-impl/crates/dql-engine/src/aws.rs) — `query_items`, explain kwargs

**Work**

- On Query (not Scan), call `.scan_index_forward(!descending)` when the plan requests server-side order.
- Include flag in EXPLAIN kwargs when present.
- Leave Scan without `ScanIndexForward` (N/A).

**Gate:** DynamoDB Local parity: `SELECT * FROM t WHERE id = 'a' DESC` returns reverse range order.

**Commit:** `feat(aws): wire ScanIndexForward for ordered queries`

---

### Step 4 — Memory backend range ordering

**Files**

- [`rust-impl/crates/dql-engine/src/memory.rs`](rust-impl/crates/dql-engine/src/memory.rs) — `execute_read` / `apply_read_options`

**Work**

- For query plans with a range key, sort matching items by range key ASC/DESC before LIMIT when server-side order is requested.
- Preserve client `sort_items` path for non-key `ORDER BY` via engine finalize.

**Gate:** In-memory `test_reverse` / `test_order_by*` can pass without Local.

**Commit:** `feat(memory): honor range-key ASC/DESC for queries`

---

### Step 5 — Un-ignore parity tests

**Files**

- [`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`](rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs)

**Work**

- Replace ignored stubs with real implementations mirroring `tests/test_queries.py::TestSelect::{test_reverse,test_order_by,test_order_by_index}`.
- Remove `#[ignore]` once green.

**Gate:** `cargo test -p dql-engine --test python_engine_query_model_parity test_reverse test_order_by test_order_by_index`

**Commit:** `test(engine): enable ORDER BY parity tests`

## Out of scope

- Changing LIMIT / SCAN LIMIT interaction beyond Python parity
- Global secondary index sort-key edge cases not covered by the three tests

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| Client sort after paginated query is incomplete | Match Python: sort the collected page set; document if full-table ORDER BY still requires scan |
| DESC + LIMIT semantics differ Local vs AWS | Prefer Local integration assertions from Python tests |
