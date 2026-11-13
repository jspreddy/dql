# TODO: Engine and backend boundaries

**Priority:** High  
**Smell / SOLID:** God object (SRP), fat interface (ISP/OCP), LSP backend drift,
stringly-typed errors  
**Crates:** `dql-engine`

## Problem

### God `Engine`

`Engine<B>` owns statement dispatch, query planning, explain/analyze, rate
limiting, schema cache, query context, post-read projection/sort, and SAVE
file I/O (`engine.rs`).

### Fat `DynamoBackend`

One trait (~12 methods) forces every backend to implement DDL, batch write,
keyed mutations, conditional bulk mutations, and reads. `ReadRequest` bundles
index, conditions, projection, ordering, pagination, and batch-get follow-up.
Adding a statement touches the trait and both backends.

### Backend behavioral drift (LSP)

`Engine::delete` builds a `QueryPlan` and passes it to the backend.
`MemoryBackend::delete_matching` honors key/filter conditions.
`SdkBackend::delete_matching` ignores `_plan` / `_options` and always scans via
`keys_for_condition` → `ReadOperation::Scan`. Same DQL can differ by backend.

### Fragile AWS errors

`aws.rs` classifies failures with
`err.to_string().contains("ResourceNotFoundException")` after flattening to
`EngineError::Runtime(String)`.

### Related smells

- Dual `RateLimit` ownership (`Engine` + `SdkBackend`) — risk of double throttle
- Dual table-meta caches (`Engine.cached_descriptions` + `SdkBackend.cache`)
- Near-duplicate `query_items` / `scan_items` pagination loops
- `record_update_read` always runs `plan_read_operation` even when explain is off
- `file_io` returns `Result<_, String>`; engine coupled to filesystem + sleep

## Recommendation

1. Split `Engine` into collaborators behind a thin façade:
   - statement dispatch
   - read pipeline (plan → execute → finalize)
   - explain/analyze state
   - table catalog / cache
2. Decompose `DynamoBackend` into smaller traits (`TableCatalog`, `ItemReader`,
   `ItemWriter`, `SchemaAdmin`) or a command enum; narrow `ReadRequest`.
3. Make `SdkBackend::delete_matching` plan-aware (query when the plan says
   query; scan only when planned). Add parity tests vs memory.
4. Map SDK service errors to typed `EngineError` variants
   (`TableNotFound`, `TableAlreadyExists`, …).
5. Single throttle owner and single metadata cache (engine-only or
   `CachingBackend<B>` wrapper).
6. Extract shared `paginate_read` for query/scan; gate explain planning on
   `self.explain`; inject `ItemStore` for SAVE/LOAD if tests need it.

Also split `lib.rs` into `error`, `result`, `backend`, `types` modules.

## Acceptance

- `SdkBackend` and `MemoryBackend` agree on delete-with-index / key-condition
  behavior under tests.
- AWS not-found / already-exists paths use typed errors, not substring match.
- One cache and one throttle policy path for normal CLI/engine use.
- `Engine::run` remains the public entry; internals are modular.
