# Rust skills review of `rust-impl/`

Review of the `dqlrs` Cargo workspace against
[leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) v1.5.1
(265 rules, 26 categories). No production code was changed for this review.

**Scope:** `rust-impl/crates/{dql-parser,dql-expr,dql-models,dql-engine,dql-output,dql-cli}`,
workspace `Cargo.toml`, `rust-toolchain.toml`, and `.github/workflows/rust-workflows.yml`.

**Method:** Apply skill categories in priority order (CRITICAL → HIGH → MEDIUM → LOW).
Cite rule IDs from the skill (`err-thiserror-lib`, `own-borrow-over-clone`, …).
Findings are observations plus suggested direction, not a change list.

**Already tracked:** Several issues overlap existing `todo_*.md` files. Those are
called out rather than restated as new design work.

---

## Snapshot

The rewrite is in good shape for a parity-focused CLI: a real workspace, a thin
`main.rs`, no `unsafe`, CI that runs `fmt` + clippy `-D warnings` + tests, and
release profile settings that already match several `opt-` / `perf-` rules.

The main skill gaps are library error types (`err-*`), clone-heavy value
conversion (`own-*` / `anti-clone-excessive`), stringly-typed AWS and config
surfaces (`type-no-stringly`, `anti-stringly-typed`), missing crate-level docs
(`doc-*`), and god files (`proj-mod-by-feature`). Those last two are already on
the design-debt list.

---

## What already matches the skill

| Skill rule | Evidence |
| --- | --- |
| `proj-workspace-large`, `proj-workspace-deps` | Six-crate workspace with `[workspace.package]` and `[workspace.dependencies]` in `rust-impl/Cargo.toml`. |
| `proj-lib-main-split` | `dql-cli/src/main.rs` is three lines: panic hooks, then `dql_cli::run()`. |
| `unsafe-*` | No `unsafe` in the tree. Miri CI is not needed. |
| `err-anyhow-app` | The binary uses `color-eyre` / `eyre::Result` and `.wrap_err(...)` in `dql-cli`. |
| `err-result-over-panic` (mostly) | Parser, engine, and backends return `Result`. Test `unwrap()` is appropriate. |
| `opt-lto-release`, `opt-codegen-units`, `perf-release-profile` | `[profile.release]` has `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`. |
| `lint-rustfmt-check`, `lint-deny-correctness` (via CI) | `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`. |
| `test-integration-dir`, `test-snapshot-testing` | Crate `tests/` plus `insta` snapshots in `dql-cli`. |
| `test-cfg-test-module` | In-crate `#[cfg(test)] mod tests` is used widely. |
| `test-descriptive-names` | Parity tests name the Python behavior they cover. |
| `test-tokio-async` | Engine tests stay sync; AWS is wrapped with an owned Tokio runtime (see async findings). |
| `name-crate-no-rs` | Library crates are `dql-*`. The binary is `dqlrs` (product name), not a `-rs` crate suffix. |
| `api-common-traits` | Domain types typically derive `Debug`, `Clone`, `PartialEq`. |
| `coll-map-choice` | `BTreeMap` for ordered items/JSON; `HashMap` for SDK caches and AWS maps. |
| `perf-io-buffering` | History and SAVE/LOAD use `BufReader`. |

`lto = "thin"` is a reasonable trade-off versus the skill's `lto = "fat"` example
(`opt-lto-release`). Fat LTO would slow CI/release builds for a modest CLI.

---

## Findings by skill category

Severity:

- **P0** — correctness or user-visible panic / lost errors
- **P1** — idiomatic library quality, likely to bite as the crate graph grows
- **P2** — maintainability, docs, or performance that is not currently hot-path proven

### 1. Error handling (CRITICAL) — P0 / P1

Rules: `err-thiserror-lib`, `err-custom-type`, `err-from-impl`, `err-source-chain`,
`err-context-chain`, `err-no-unwrap-prod`, `err-expect-bugs-only`,
`err-lowercase-msg`, `err-doc-errors`, `anti-unwrap-abuse`, `anti-stringly-typed`.

**Library errors are string bags, not typed enums.**

`dql-parser::ParseError`, `dql-expr::ExprError`, and `dql-models::ModelError` are
each a `message: String`. `dql-engine::EngineError` is:

```text
Parse(ParseError) | Runtime(String)
```

`file_io::parse_file_spec` returns `Result<FileSpec, String>`. SDK failures go
through `SdkBackend::aws_error(err: impl Display) -> EngineError::Runtime(err.to_string())`.
Expr/model/IO errors are flattened with `err.to_string()` at engine call sites.

Consequences:

- No `#[source]` chain (`err-source-chain`). `EngineError` implements `Error` with
  no `source()`.
- Callers cannot match `TableNotFound` vs parse vs IO vs AWS (`err-custom-type`).
- `?` cannot convert `ExprError` / `ModelError` / `io::Error` (`err-from-impl`).
- Libraries should use `thiserror` (`err-thiserror-lib`); the CLI already uses
  the application-side equivalent (`color-eyre`).

This is the same gap as `todo_engine_backend_boundaries.md` (“typed AWS errors”
and “`file_io` returns `Result<_, String>`”).

**AWS errors are classified by substring.** In `dql-engine/src/aws.rs`:

- `err.to_string().contains("ResourceNotFoundException")`
- `contains("ResourceInUseException")`
- `contains("already exists")` / `contains("does not exist")`

That is brittle across SDK Display formats and locales (`type-no-stringly`,
`anti-stringly-typed`). Prefer `ProvideErrorMetadata` / service error enums.

**Production panics (not tests).**

| Location | Rule | Note |
| --- | --- | --- |
| `dql-output/src/lib.rs` `format_items` — `formatter.display(...).unwrap()` | `err-no-unwrap-prod` | Writing to `Vec<u8>` is extremely unlikely to fail, but the API still panics. Return `io::Result<String>` or use `expect` with an invariant comment. Same pattern in `rich_layout_to_text`. |
| `dql-engine/src/aws.rs` `PutRequest::builder()...build().expect("valid put request")` | `err-expect-bugs-only` | SDK builders can fail if required fields are missing. Map to `EngineError`. |
| `dql-engine/src/aws.rs` `primary_key_from_meta` — `item_to_attributes(...)?.remove(...).unwrap()` | `err-no-unwrap-prod` | After a successful convert, the key should exist; still a panic path. Use `ok_or_else`. |
| `dql-models` `schema_dql` — `attrs.get(&self.hash_key).expect("hash key")` | `err-expect-bugs-only` | OK only if `TableMeta` construction guarantees the invariant. Document `# Panics` (`doc-panics-section`) or return `Result`. |
| `dql-expr` `SelectionParser::parse_additive` / `parse_multiplicative` — `self.next().unwrap()` after `peek()` | `err-expect-bugs-only` | Local invariant; `unreachable!` on the op match is the same class. Fine if kept next to a comment; better as `let-else`. |
| `dql-engine` `execute_keys_in_read` — `options.keys_in.as_ref().expect("keys_in checked above")` | `err-expect-bugs-only` | Same: encode with `let-else` (`pat-let-else`) after `validate_read_keys_in`. |

Test-only `unwrap` / `panic!("unexpected …")` is acceptable and is **not** listed.

**Error message style is mixed** (`err-lowercase-msg`): `"expected update expression"`
vs `"Cannot perform SELECT without an indexed WHERE clause..."`. Library errors
should start lowercase without trailing punctuation; user-facing CLI copy can
stay capitalized at the `color-eyre` boundary.

**Silent error drops** (`anti-empty-catch`):

- `CliConfig::load` uses `.ok()` / `.and_then(serde_json::from_str.ok())` and
  falls back to defaults if the file is unreadable or invalid JSON. A corrupt
  `~/.config/dql.json` is indistinguishable from “no config”.
- History load/save failures `eprintln!` and continue (`dql-cli/src/history.rs`).
  That is reasonable for a REPL, but there is no structured log (`obs-*`).

### 2. Ownership and cloning (CRITICAL) — P1

Rules: `own-borrow-over-clone`, `own-clone-explicit`, `anti-clone-excessive`,
`own-slice-over-vec`, `anti-string-for-str`, `mem-take-replace`.

The tree clones `String` / `BTreeMap` / `Item` on almost every DynamoDB round-trip:

- `dql-expr`: `value_to_dynamo` / `dynamo_to_value` / `value_to_json` clone every
  number and string; `Visitor` clones maps out of fields/values.
- `dql-engine/src/convert.rs` and `json_util.rs`: item ↔ AttributeValue ↔ JSON
  clone keys and nested values.
- `dql-models`: `TableMeta` construction clones hash/range keys, index names,
  projections, and throughput several times.
- `dql-engine/src/aws.rs`: `pending.clone()` on `batch_write_item` retries;
  `config.clone()` into `block_on`; table-meta cache returns `meta.clone()`.
- `dql-cli` REPL: `self.lines.clone()`, history `entry.clone()`, rich-table
  `column.name.clone()` into every `Cell`.

Some of this is forced by AWS SDK owned builders and by storing owned `Item =
BTreeMap<String, Value>`. Still:

- Accept `&str` / `&[T]` at helpers (`own-slice-over-vec`). `parse_bool` in
  `dql-cli/src/meta/ls.rs` takes `Option<&String>` (`anti-string-for-str`).
- Avoid `item.clone()` when projecting; build a new map with `mem::take` /
  moved values (`mem-take-replace`).
- Cache lookups can return `&TableMeta` or `Arc<TableMeta>` (`own-arc-shared`)
  instead of cloning the whole meta on every describe.

Do not chase clones in the parser AST without measuring (`anti-premature-optimize`,
`perf-profile-first`). The conversion and AWS adapter layers are the realistic
wins.

### 3. Type safety and API design (HIGH / MEDIUM) — P1

Rules: `type-no-stringly`, `type-enum-states`, `api-parse-dont-validate`,
`api-newtype-safety`, `conv-fromstr-parsing`, `api-non-exhaustive`,
`api-must-use`, `proj-pub-crate-internal`.

**Stringly config and throttle.** `CliConfig` stores `width`, `pagesize`, and
`throttle` as `serde_json::Value`. `TableLimits` stores read/write caps as
`BTreeMap<String, String>` and validates with `chars().all(is_ascii_digit)`.
`OutputFormat::from_name(&str)` is an ad-hoc parser instead of `FromStr`
(`conv-fromstr-parsing`). Tracked in `todo_config_opt_registry.md`.

**Public structs are bags of `pub` fields.** `Attribute`, `Throughput`,
`SdkConfig`, `ReadRequest`, `Item`, `TableMeta`, and most AST nodes expose every
field. That is convenient for a unpublished workspace (`publish = false` on
internal crates), but it blocks `api-parse-dont-validate` and
`type-newtype-validated`. `pub(crate)` (`proj-pub-crate-internal`) would shrink
the accidental API of `dql-engine` / `dql-parser`.

**Public enums are exhaustive.** `Statement`, `EngineError`, `StatementResult`,
`OutputFormat` have no `#[non_exhaustive]` (`api-non-exhaustive`). Fine while
crates are private; needed if anything is published.

**`format_items` and `render_result` ignore `#[must_use]`** (`api-must-use`):
not a bug today, but `io::Result` from display helpers should stay visible.

**`AttributeType::Other(String)`** keeps an escape hatch that is stringly typed.
Prefer a documented unknown variant plus `#[non_exhaustive]` if DynamoDB adds
types.

### 4. Async and concurrency (HIGH) — P1 (design), P2 (style)

Rules: `async-tokio-runtime`, `async-tokio-fs`, `async-no-lock-await`,
`async-fn-in-trait`.

`SdkBackend` owns a multi-thread Tokio `Runtime` and exposes a **sync**
`DynamoBackend` trait by calling `self.runtime.block_on(...)` on every SDK
operation (connect, list, CRUD, query/scan pagination, STS `whoami`). File SAVE/LOAD
uses `std::fs` (`async-tokio-fs` would apply only if those paths were async).

This is a coherent choice for a blocking REPL: one runtime, no nested
`block_on` from inside async, no locks held across `.await` in application
code. Costs:

- Every engine method pays runtime enter/exit.
- `DynamoBackend` cannot be `async fn` in the trait (`async-fn-in-trait`), so
  `MemoryBackend` and `SdkBackend` share a sync façade.
- Throttle wait is `std::thread::sleep` inside `apply_throttle`, which blocks a
  Tokio worker if `block_on` is running on the runtime’s own thread. Today
  `block_on` is invoked from the REPL thread, so this is OK, but it is easy to
  regress.

`todo_engine_backend_boundaries.md` already wants a slimmer backend. If that
split happens, consider `async` traits for the SDK side and keep memory sync,
or document the owned-runtime wrapper as the permanent CLI contract.

No `Mutex` across `.await` was found (`async-no-lock-await` / `anti-lock-across-await`).
No `unsafe` `Send`/`Sync` impls.

### 5. Numeric safety (HIGH) — P2

Rules: `num-cast-try-from`, `num-overflow-explicit`, `num-float-compare`.

- Query/scan `limit((remaining.min(100)) as i32)` in `aws.rs` is a narrowing
  cast. `min(100)` makes it safe today; `TryFrom` would still be clearer
  (`num-cast-try-from`).
- `scanned_count() as usize` / `count() as usize` assume non-negative SDK
  counts.
- Terminal sizes: `len() as u16`, `row as u16` in the REPL. These saturate in
  some paths (`.min(area.height)`) and not others.
- `RateLimit` uses `f64` tokens and `Duration::from_secs_f64` of a negative
  ratio. Fine for capacity units; do not compare those floats with `==`
  (`num-float-compare`).
- Timestamp math uses `as i64` / `as f64` in `dql-expr` civil-date helpers —
  domain code, not a bug, but overflow is unchecked (`num-overflow-explicit`).

`WidthSetting::from_config` does `n.as_u64().unwrap_or(80) as usize` — fallback
is OK; `TryFrom` would still document the bound.

### 6. Memory / performance (CRITICAL skill, P2 here)

Rules: `mem-with-capacity`, `mem-avoid-format`, `mem-write-over-format`,
`perf-iter-over-index`, `perf-entry-api`, `anti-format-hot-path`,
`anti-premature-optimize`.

- Hand-rolled base64 encoders in `convert.rs`, `json_util.rs`, and `dql-expr`
  do not `String::with_capacity` (`mem-with-capacity`). Three copies of the
  same codec: `todo_dedupe_cross_cutting.md`.
- `format_field` and schema/EXPLAIN builders use `format!` to build nested
  strings (`mem-write-over-format`). Fine at CLI volume.
- `SmartFormat` / `ColumnFormat` clone map keys to measure widths (`perf-entry-api`
  could use `entry` with borrowed keys if the map were `HashMap` with a
  suitable hasher; `BTreeMap` keys are owned).

No change recommended without a profile (`perf-profile-first`). Duplicate
base64 is a correctness/DRY issue more than a speed issue.

### 7. Serde (MEDIUM) — P1

Rules: `serde-rename-all`, `serde-default-compat`, `serde-deny-unknown-fields`,
`serde-try-from-validate`, `api-serde-optional`.

`CliConfig` uses `#[serde(default)]` well (`serde-default-compat`) but:

- `width` / `pagesize` / `_throttle` are untyped JSON (`serde-try-from-validate`
  would parse into enums).
- No `deny_unknown_fields`, so typos in `dql.json` are ignored
  (`serde-deny-unknown-fields`). Combined with silent load failure, misconfig
  is hard to notice.
- Internal crates hard-depend on `serde`/`serde_json`. They are not published
  (`api-serde-optional` is N/A until publish).

`dql-cli` `TableLimits::load` does `serde_json::from_value::<Self>(data.clone())`
and ignores `Err` — another silent drop.

### 8. Pattern matching (MEDIUM) — P2

Rules: `pat-let-else`, `pat-exhaustive-enum`, `pat-matches-macro`.

Several `expect("… checked above")` sites should be `let Some(x) = … else { return Err(...) }`
(`pat-let-else`). Catch-all `_ => unreachable!()` after matching a closed set of
string keywords (update `SET`/`ADD`/…) is OK; prefer matching the already-parsed
enum.

`WriteRequest` / AWS `ProjectionType` use `Some(_)` catch-alls in `convert.rs`
(`pat-exhaustive-enum`) — new SDK variants would silently map to `None`.

### 9. Documentation (MEDIUM) — P1

Rules: `doc-all-public`, `doc-module-inner`, `doc-errors-section`,
`doc-panics-section`, `doc-examples-section`, `doc-cargo-metadata`,
`doc-crate-readme`.

- `dql-parser`, `dql-expr`, `dql-models`, and `dql-engine` crate roots have
  little or no `//!` module docs (`doc-module-inner`).
- Public `parse_statement`, `Engine::execute`, `render_condition`, etc. lack
  `# Errors` / `# Panics` (`doc-errors-section`, `err-doc-errors`).
- Workspace `readme` points at `rust-docs/README.md` (good, `doc-crate-readme`
  direction) but crate `lib.rs` files do not `include_str!` it.
- `dql-cli` package metadata is incomplete relative to workspace
  (`description` lives on `[workspace.package]`; binary crate has no extra
  keywords/categories — fine while unpublished).

User docs in `rust-docs/` are separate and are in good shape; this gap is
rustdoc for library crates.

### 10. Observability (MEDIUM) — P2

Rules: `obs-tracing-over-log`, `obs-structured-fields`, `obs-library-facade`,
`obs-no-sensitive-data`.

There is no `tracing` (or `log`) dependency. Diagnostics are `eprintln!` /
`println!` in the CLI (`unknown argument`, version, history IO errors) and
`color-eyre` reports in the REPL.

For a TUI this is acceptable. If engine/SDK retries and throttle waits need
diagnosis, add `tracing` in `dql-engine` and a subscriber only in `dql-cli`
(`obs-library-facade`). Do not log AWS keys; `SdkConfig` can hold
`secret_key: Option<String>` — keep that out of `Debug` if it is ever printed
(`obs-no-sensitive-data`). `SdkConfig` currently has no `Debug` impl on the
struct itself (fields are public).

### 11. Testing (MEDIUM) — P2

Rules: `test-integration-dir`, `test-arrange-act-assert`, `test-proptest-properties`,
`test-doctest-examples`, `test-criterion-bench`.

Strengths: parser/engine/CLI parity tests, DynamoDB Local jobs in CI, `insta`
snapshots, smoke test script.

Gaps:

- Almost no doctests (`test-doctest-examples`) because public APIs are
  undocumented.
- No property tests on the lexer, timestamp parser, or base64 round-trip
  (`test-proptest-properties`). Hand-rolled base64 is a good proptest target.
- Unit tests live inside 2k-line `lib.rs` files, which inflates those modules
  (`todo_split_monolith_modules.md`). Moving them to `tests/` or `src/…/tests.rs`
  is optional (`test-cfg-test-module` allows in-module tests).
- No `criterion` benches (`test-criterion-bench`). Not requested until a
  hot path is identified.

### 12. Project structure and lint config (LOW) — P1 for structure, P2 for lints

Rules: `proj-mod-by-feature`, `proj-flat-small`, `proj-msrv-declare`,
`proj-workspace-deps`, `lint-workspace-lints`, `lint-missing-docs`,
`lint-cfg-check`.

**God files** (also `todo_split_monolith_modules.md`):

| File | Lines |
| --- | ---: |
| `dql-parser/src/lib.rs` | 2154 |
| `dql-expr/src/lib.rs` | 1295 |
| `dql-engine/src/engine.rs` | 1264 |
| `dql-engine/src/aws.rs` | 1246 |
| `dql-engine/src/memory.rs` | 945 |
| `dql-models/src/lib.rs` | 930 |
| `dql-cli/src/repl/app.rs` | 822 |

**MSRV** is not declared (`proj-msrv-declare`). `rust-toolchain.toml` pins
`channel = "stable"`; CI uses `dtolnay/rust-toolchain@stable`. That can break
reproducible builds when stable moves. Prefer an explicit `rust-version` plus
the same channel in CI.

**Workspace lints** are not set in `Cargo.toml` (`lint-workspace-lints`). Clippy
is only `-D warnings` in CI, which is strong, but rustc lints
(`unused_must_use`, `missing_docs`, `unexpected_cfgs`) are not centralized
(`lint-cfg-check`, `lint-missing-docs`).

**`#[allow(dead_code)]`** on `write_paged` and JSON format helpers
(`dql-output`) hides unused API instead of `pub(crate)` or a feature gate.

**Edition** is 2021. The skill text targets edition 2024; there is no
requirement to move yet. `unsafe-extern-block` / `unsafe-no-mangle-unsafe` do
not apply.

**Duplicate helpers** (`todo_dedupe_cross_cutting.md`): base64 ×3, AWS
`build_client` in `aws.rs` and `cloudwatch.rs`, `parse_bool` in `ls.rs` vs
`opt.rs`.

**Dependency direction:** `dql-output` depends on `dql-engine`
(`todo_invert_output_dependencies.md`) — opposite of `proj-mod-by-feature`
layering.

### 13. Naming (MEDIUM) — P2

Rules: `name-funcs-snake`, `name-types-camel`, `name-no-get-prefix`,
`name-is-has-bool`.

Identifiers are idiomatic. Minor notes:

- `Visitor` in `dql-expr` is a placeholder generator, not a visitor pattern.
- `get_field` / `get_value` on `Visitor` allocate placeholders; `name-no-get-prefix`
  would suggest `field_key` / `value_key`.
- `is_memory` / `is_remote` / `is_local` follow `name-is-has-bool`.
- `format_throughput` lives in both models (via engine re-export) — naming is
  fine; ownership is the issue.

### 14. Macros / traits / const — no material findings

No proc macros. No over-generic public APIs (`anti-over-abstraction`). Few
`const fn` opportunities beyond small helpers (`const-fn`) — not worth a pass.

---

## Cross-walk to existing `todo_*.md`

| This review | Existing todo |
| --- | --- |
| God files, tests inside `lib.rs` | `todo_split_monolith_modules.md` |
| Stringly `EngineError::Runtime`, substring AWS errors, dual cache/throttle | `todo_engine_backend_boundaries.md` |
| `dql-output` → `dql-engine` | `todo_invert_output_dependencies.md` |
| Triple base64, dual `build_client`, dual `parse_bool` | `todo_dedupe_cross_cutting.md` |
| JSON `width`/`pagesize`/`throttle`, `KNOWN_FLAGS` vs clap | `todo_config_opt_registry.md` |
| Opaque update/selection strings, `SelectionParser` | `todo_typed_expression_ast.md` |
| REPL vs `-c` paths | `todo_unify_cli_pipelines.md` |

New relative to those todos: production `unwrap`/`expect` sites, missing
`thiserror` / `Error::source`, silent config load, no MSRV, no `tracing`, no
crate-level rustdoc, `Option<&String>` helpers, and SDK builder `expect`.

---

## Suggested fix order (when code changes are wanted)

Not in scope for this review. If a follow-up lands, this order matches skill
priority and user impact:

1. **Stop panicking in library helpers** — `format_items`, `PutRequest` build,
   `primary_key_from_meta` (`err-no-unwrap-prod`).
2. **Typed `EngineError` + `thiserror` + `From` impls**; classify AWS errors
   without `contains` (`err-thiserror-lib`, `err-from-impl`).
3. **Do not swallow `dql.json` parse failures**; optionally
   `deny_unknown_fields`.
4. **One base64 implementation** (crate or shared module) with round-trip tests.
5. **Split god files** per `todo_split_monolith_modules.md`; add `//!` crate docs
   as modules appear.
6. **Declare `rust-version`** and workspace lints; keep clippy `-D warnings`.
7. Only then: clone reduction in convert/expr, async trait, `tracing`.

---

## Skill install note

The review used `npx add-skill leonardomso/rust-skills` (forwards to
`npx skills add`). That writes `.agents/skills/rust-skills` and `skills-lock.json`
in the repo root. Those files are **not** part of this documentation change and
should stay untracked unless the project decides to vendor agent skills.
