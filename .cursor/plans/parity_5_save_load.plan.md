---
name: Parity 5 SAVE LOAD Formats
overview: "Add SELECT/SCAN … SAVE and complete LOAD format parity using JSON lines, CSV, gzip wrappers, and MessagePack as the open-source binary replacement for Python pickle. Un-ignore test_file_formats with a Rust-native MessagePack round-trip."
todos:
  - id: step-1-parser-save
    content: "Step 1: Parse SAVE filename on SELECT/SCAN"
    status: pending
  - id: step-2-json-csv-gzip
    content: "Step 2: Engine SAVE/LOAD for JSON lines, CSV, and gzip"
    status: pending
  - id: step-3-messagepack
    content: "Step 3: MessagePack (.msgpack / default binary) replaces pickle"
    status: pending
  - id: step-4-migration-docs
    content: "Step 4: Document pickle→MessagePack migration and rejection of .p pickle"
    status: pending
  - id: step-5-unignore-tests
    content: "Step 5: Un-ignore/adapt test_file_formats for MessagePack"
    status: pending
isProject: false
---

# Parity 5: SAVE / LOAD Formats (MessagePack instead of pickle)

## Current state

- Rust **LOAD** supports JSON lines and CSV only (`dql-engine` `load`); no gzip; no SAVE.
- Parser has no `SAVE` clause on SELECT/SCAN.
- Python SAVE/LOAD (`dql/engine.py`, `tests/test_save.py`):
  - `.csv` / `.json` (+ optional `.gz` / `.gzip`)
  - Default binary: **pickle** — one `pickle.dump(dict)` per record (not a single list). Values are Python-native (`Decimal`, `set`, `bytes`, …).
  - Pickle is **Python-specific** and not a viable Rust interchange format.

## Goal

| Format | Extension(s) | Status target |
| --- | --- | --- |
| JSON lines | `.json`, `.json.gz` | SAVE + LOAD |
| CSV | `.csv`, `.csv.gz` | SAVE + LOAD |
| MessagePack (pickle replacement) | `.msgpack`, `.msgpack.gz`; also default when no known text ext | SAVE + LOAD |
| Legacy pickle | `.p`, `.pkl`, `.pickle` | **Reject with clear error** pointing to MessagePack |

## Design decisions

### Why MessagePack

- Open specification ([msgpack.org](https://msgpack.org/)), multi-language, compact, streaming-friendly.
- Maps cleanly to DQL `Value` (maps, arrays, strings, bools, nil, binary).
- Closer to pickle’s “one record after another” model than Parquet/Arrow (columnar) or requiring a schema registry.

### Record layout

- File = concatenated MessagePack **maps** (one object per item), same multi-record pattern as Python pickle dumps.
- Optional 4-byte magic / version prefix `DQL1` before the first record for future-proofing — if present, LOAD requires it; if absent, accept raw msgpack stream for tooling interop.
- Numbers stored as **strings** (or a tagged extension) to preserve DynamoDB decimal fidelity, matching how Rust already prefers stringly numbers in places — document the choice in the migration note.
- Binary → MessagePack bin; sets → arrays with a type tag or sorted arrays (document: LOAD restores as DQL `Set` when tagged, else `List`).

Recommended encoding for sets (pick one in Step 3 and stick to it):

```text
{ "__dql_set__": [ ...elements... ] }
```

### Dependencies

- `rmp-serde` or `rmp` + manual encode from `Value`
- `flate2` for gzip
- Existing `serde_json` / CSV writer path

## Implementation steps (one commit each)

### Step 1 — Parser: `SAVE`

**Files**

- [`rust-impl/crates/dql-parser/src/lib.rs`](rust-impl/crates/dql-parser/src/lib.rs) — `Select` / `Scan` AST, `parse_select` / `parse_scan`
- Parser parity tests

**Work**

- Parse trailing `SAVE <string>` (mirror Python grammar `save_file`).
- Reject `count(*)` + `SAVE` at parse or engine time (Python SyntaxError).

**Gate:** Parser tests for `SELECT * FROM t SAVE 'out.json'`.

**Commit:** `feat(parser): parse SAVE clause on SELECT and SCAN`

---

### Step 2 — JSON / CSV / gzip SAVE and LOAD

**Files**

- [`rust-impl/crates/dql-engine/src/engine.rs`](rust-impl/crates/dql-engine/src/engine.rs) — `select`/`scan` post-process, `load`, new `save_results`
- Small helper module e.g. `dql-engine/src/file_io.rs`

**Work**

- After a successful read with `save_file`, write projected items (same shape as display/JSON) to disk; return affected count / “Saved N records” status like Python.
- Extension sniffing: strip `.gz`/`.gzip`, then dispatch on `.json` / `.csv`.
- LOAD: open gzip when needed; keep existing JSON/CSV readers.
- Shared `open_smart(path) -> Read/Write` helper.

**Gate:** Round-trip JSON and CSV with and without gzip in engine tests.

**Commit:** `feat(engine): SAVE/LOAD JSON CSV and gzip`

---

### Step 3 — MessagePack binary format

**Files**

- `dql-engine/src/file_io.rs` (or `dql-expr` serialization helpers)
- [`rust-impl/crates/dql-engine/Cargo.toml`](rust-impl/crates/dql-engine/Cargo.toml)
- Workspace `Cargo.toml` if deps are workspace-shared

**Work**

- Implement `value_to_msgpack` / `msgpack_to_value` (or serde on a stable DTO).
- Default SAVE extension when unspecified / `.msgpack` → MessagePack stream.
- LOAD `.msgpack` / `.msgpack.gz`.
- On `.p` / `.pkl` / `.pickle`: return a clear `EngineError` — “pickle is not supported; use MessagePack (.msgpack) or export JSON/CSV from Python DQL”.

**Gate:** MessagePack round-trip unit test covering String, Number, Bool, Null, Binary, List, Set, Map.

**Commit:** `feat(engine): MessagePack SAVE/LOAD as pickle replacement`

---

### Step 4 — Migration documentation

**Files**

- [`rust-plans/migration-from-python.md`](rust-plans/migration-from-python.md)
- [`rust-impl/README.md`](rust-impl/README.md)
- Help text in [`rust-impl/crates/dql-cli/src/help.rs`](rust-impl/crates/dql-cli/src/help.rs)

**Work**

- Document format matrix and pickle → MessagePack break.
- One-liner migration: Python `SELECT * FROM t SAVE 'x.json'` (or CSV) then Rust LOAD; or a small note that `.p` files must be re-exported.
- Update SAVE/LOAD help strings.

**Commit:** `docs: MessagePack replaces pickle for SAVE/LOAD`

---

### Step 5 — Parity test

**Files**

- [`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`](rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs) — `test_file_formats`
- Optionally adapt expectations vs `tests/test_save.py` (swap `.p` cases for `.msgpack`)

**Work**

- Implement round-trips for `.csv`, `.json`, `.msgpack`, and gzip variants.
- Do **not** attempt to read Python pickle bytes.
- Remove `#[ignore]`.

**Gate:** `cargo test -p dql-engine --test python_engine_query_model_parity test_file_formats`

**Commit:** `test(engine): enable SAVE/LOAD format parity with MessagePack`

## Out of scope

- Reading legacy pickle files in Rust
- Parquet / Arrow / SQLite alternate backends
- Streaming SAVE of multi-GB result sets beyond what Python does today

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| Number precision loss | Store numbers as strings in MessagePack maps |
| Set vs list ambiguity | Explicit `__dql_set__` tag |
| Users with `.p` archives | Loud error + JSON/CSV re-export path in docs |
