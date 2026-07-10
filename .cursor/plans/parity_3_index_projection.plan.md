---
name: Parity 3 Index Projection Count
overview: "Finish index-aware reads: PK-only query/scan when the index does not project selected attributes, follow-up batch_get, and count(*) on smart indexes / GSIs. Un-ignore the three related engine parity tests."
todos:
  - id: step-1-aws-two-phase
    content: "Step 1: AWS two-phase read — PK projection then batch_get with selection"
    status: completed
  - id: step-2-memory-follow-up
    content: "Step 2: MemoryBackend implements follow_up_batch_get semantics"
    status: completed
  - id: step-3-count-index
    content: "Step 3: count(*) uses Query/Scan on chosen index (smart + USING GSI)"
    status: completed
  - id: step-4-unignore-tests
    content: "Step 4: Un-ignore test_select_non_projected, test_count_smart_index, test_count_on_index"
    status: completed
isProject: false
---

# Parity 3: Index Projection and Count-on-Index

## Current state

Planner already sets `follow_up_batch_get` when the index does not project all selected attributes (`dql-models` `query_plan` / `projects_all_attributes`). Engine passes the flag through.

Gaps:

1. **AWS** still projects the full selection on the initial query/scan, then optionally batch_gets — Python projects **primary key attributes only**, then `batch_get` for the rest (`dql/engine.py` `_select`, `fetch_attrs_after`).
2. **Memory** never applies `follow_up_batch_get`; it always reads full table items.
3. **Count on index** cases are ignored:
   - `test_count_smart_index` — `count(*)` should Query the auto-chosen index
   - `test_count_on_index` — `SCAN count(*) … USING gindex`
   - `test_select_non_projected` — KEYS/INCLUDE index + non-projected attrs

## Goal

| Scenario | Expected |
| --- | --- |
| SELECT non-projected attrs via LSI/GSI | Query index (PK attrs) → `batch_get` full/projected items |
| `count(*)` with hash+index predicate | Query on smart-chosen index, `Select::Count` |
| `SCAN count(*) USING gsi` | Scan that GSI with count |

## Design decisions

- Keep planner flag; fix **backends + engine projection choice**.
- Memory should simulate partial projection enough for parity tests (PK stubs → table `batch_get_keys`), not a full DynamoDB projection engine.
- Optional follow-up: attach `scanned_count` to count results for Python-style status strings (nice-to-have, not required to un-ignore).

## Implementation steps (one commit each)

### Step 1 — AWS two-phase read

**Files**

- [`rust-impl/crates/dql-engine/src/aws.rs`](rust-impl/crates/dql-engine/src/aws.rs) — `execute_read`, `query_items`, `scan_items`, `batch_get_items`
- [`rust-impl/crates/dql-engine/src/engine.rs`](rust-impl/crates/dql-engine/src/engine.rs) — pass PK attribute list when `follow_up_batch_get`
- [`rust-impl/crates/dql-models/src/lib.rs`](rust-impl/crates/dql-models/src/lib.rs) — `primary_key_attributes` helper if missing

**Work**

- When `follow_up_batch_get`:
  - Initial Query/Scan projection = table primary key attributes only (via expression names).
  - Collect keys → `batch_get_items` with the original selection projection / aliases.
- When not following up, keep current full projection path.
- Ensure EXPLAIN mentions the follow-up `batch_get` (already partially present).

**Gate:** DynamoDB Local: KEYS-only GSI/LSI select of a non-projected attribute returns full values.

**Commit:** `fix(aws): PK-only index read then batch_get for missing attrs`

---

### Step 2 — Memory follow-up batch_get

**Files**

- [`rust-impl/crates/dql-engine/src/memory.rs`](rust-impl/crates/dql-engine/src/memory.rs) — `execute_read`, index matching helpers

**Work**

- If plan has `follow_up_batch_get`, return PK-keyed stubs from the index path (or filter as today but strip non-projected attrs), then hydrate via `batch_get_keys` from the table store.
- Apply selection / aliases after hydration so results match AWS.

**Gate:** In-memory `test_select_non_projected` can pass.

**Commit:** `feat(memory): simulate partial index projection + batch_get`

---

### Step 3 — Count on smart index / GSI

**Files**

- [`rust-impl/crates/dql-engine/src/aws.rs`](rust-impl/crates/dql-engine/src/aws.rs) — `Select::Count` path already exists; verify index choice
- [`rust-impl/crates/dql-models/src/lib.rs`](rust-impl/crates/dql-models/src/lib.rs) — `plan_read` with empty selection attrs for `CountAll`
- [`rust-impl/crates/dql-engine/src/engine.rs`](rust-impl/crates/dql-engine/src/engine.rs) — `finalize_read_result` for counts

**Work**

- Ensure `Selection::CountAll` still runs `plan_read` with WHERE/USING so the correct index is chosen (smart index and explicit `USING`).
- `follow_up_batch_get` must be false for count-only (no attribute fetch).
- Memory: count items that would match the planned Query/Scan, not a full table scan when an index applies.

**Gate:** Smart-index count and `USING gindex` count match Python test expectations.

**Commit:** `fix(engine): count(*) respects index planning`

---

### Step 4 — Un-ignore parity tests

**Files**

- [`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`](rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs)

**Work**

- Port bodies from `tests/test_queries.py`:
  - `TestSelect::test_select_non_projected`
  - `TestSelect::test_count_smart_index`
  - `TestRegressions::test_count_on_index`
- Remove `#[ignore]`.

**Gate:** `cargo test -p dql-engine --test python_engine_query_model_parity` for those three names.

**Commit:** `test(engine): enable index projection and count parity tests`

## Out of scope

- Changing INCLUDE projection allow-lists beyond what Python tests cover
- Consumed-capacity formatting for ANALYZE

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| Double-read cost on AWS | Same as Python; only when projection incomplete |
| Memory over-simulates | Prefer minimal stub+hydrate; assert against Python tests |
| Count + LIMIT semantics | Copy Python test cases exactly |
