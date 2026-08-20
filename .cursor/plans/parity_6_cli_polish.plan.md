---
name: Parity 6 CLI Polish
overview: "Close remaining CLI/UX gaps versus Python: wire display=less, ParseError caret offsets, ls CloudWatch metrics, rich table detail, watch dashboard, and un-ignore stale GSI throughput / format_exc tests."
todos:
  - id: step-1-display-less
    content: "Step 1: Honor opt display=less in -c and REPL result paths"
    status: completed
  - id: step-2-parse-offset
    content: "Step 2: Populate ParseError.offset and un-ignore test_format_exc"
    status: completed
  - id: step-3-throughput-test
    content: "Step 3: Un-ignore test_total_throughput (likely already implemented)"
    status: completed
  - id: step-4-ls-metrics
    content: "Step 4: ls metrics=True via CloudWatch get_metric_statistics"
    status: completed
  - id: step-5-rich-detail
    content: "Step 5: Richer ls table detail formatting"
    status: completed
  - id: step-6-watch
    content: "Step 6: watch meta-command behind Cargo feature (ratatui + CloudWatch)"
    status: completed
isProject: false
---

# Parity 6: CLI Polish

## Current state

| Feature | Python | Rust today |
| --- | --- | --- |
| `opt display less` | Results via less | Implemented in `dql-output`, but `-c` / REPL hardcode stdout / buffer |
| Parse error caret | `exc.loc` under fragment | `ParseError.offset` always `None` |
| GSI throughput totals | `TableMeta.total_*` | Implemented in `dql-models`; parity test still `#[ignore]` (likely stale) |
| `ls … metrics=True` | CloudWatch RCU/WCU | Kwarg parsed, ignored (`let _ = metrics`) |
| Rich table detail | Rich panels | Plain `format_table_detail` |
| `watch` | curses Monitor | Stub message only |

## Goal

Ship the remaining user-visible CLI behaviors, feature-gating CloudWatch-heavy pieces (`metrics`, `watch`) if that keeps default builds light.

## Design decisions

- **`display=less`**: reuse existing `less_display` / `DisplayMode` for one-shot and non-TUI paths; REPL may keep an in-pane buffer but should respect `less` when `display=less` for large dumps or document TUI exception.
- **`watch` + `ls metrics`**: optional Cargo feature `watch` on `dql-cli` pulling `aws-sdk-cloudwatch` (as Phase 5 originally intended).
- **Rich detail**: prefer `comfy-table` or ratatui-friendly structured text — not a Python Rich dependency. Match information density of `TableMeta.pformat(rich)`, not pixel-identical panels.
- Un-ignore `test_total_throughput` early; it may already pass.

## Implementation steps (one commit each)

### Step 1 — Wire `display=less`

**Files**

- [`rust-impl/crates/dql-cli/src/session.rs`](rust-impl/crates/dql-cli/src/session.rs) — `run_command`
- [`rust-impl/crates/dql-cli/src/repl/app.rs`](rust-impl/crates/dql-cli/src/repl/app.rs)
- [`rust-impl/crates/dql-cli/src/meta/file.rs`](rust-impl/crates/dql-cli/src/meta/file.rs) — reference pattern
- [`rust-impl/crates/dql-output/src/display.rs`](rust-impl/crates/dql-output/src/display.rs)

**Work**

- Map `config.display` → `DisplayMode` for `-c` / `run_command` (same as `file`).
- REPL: either spawn less for query results when configured, or document that TUI mode always buffers and only non-TUI honors less.
- Keep `--json` / pipe-friendly stdout when output is not a TTY if that matches Python.

**Gate:** `opt display less` then `-c "SCAN …"` opens less when a TTY is present.

**Commit:** `fix(cli): honor display=less for command output`

---

### Step 2 — ParseError offsets + caret

**Files**

- [`rust-impl/crates/dql-parser/src/lib.rs`](rust-impl/crates/dql-parser/src/lib.rs) — `ParseError`, tokenizer, `Parser::error`
- [`rust-impl/crates/dql-engine/src/fragment.rs`](rust-impl/crates/dql-engine/src/fragment.rs) — `pformat_exc`
- Parity test `test_format_exc`

**Work**

- Track byte/char offset while tokenizing; set `ParseError.offset` on failures.
- Ensure `FragmentEngine::pformat_exc` places the caret under the error (Python `tests/test_engine.py::TestFragmentEngine::test_format_exc`).
- Un-ignore and implement the parity test.

**Gate:** `cargo test -p dql-engine --test python_engine_query_model_parity test_format_exc`

**Commit:** `fix(parser): populate ParseError offsets for caret display`

---

### Step 3 — GSI throughput parity test

**Files**

- [`rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs`](rust-impl/crates/dql-engine/tests/python_engine_query_model_parity.rs) — `test_total_throughput`
- [`rust-impl/crates/dql-models/src/lib.rs`](rust-impl/crates/dql-models/src/lib.rs) — verify `total_read_throughput` / `total_write_throughput`

**Work**

- Port assertion from `tests/test_models.py::TestModels::test_total_throughput`.
- Fix models only if the test reveals a real gap; otherwise just enable it.

**Gate:** Test passes without `#[ignore]`.

**Commit:** `test(models): enable total GSI throughput parity test`

---

### Step 4 — `ls metrics=True`

**Files**

- [`rust-impl/crates/dql-cli/src/meta/ls.rs`](rust-impl/crates/dql-cli/src/meta/ls.rs)
- [`rust-impl/crates/dql-engine/src/aws.rs`](rust-impl/crates/dql-engine/src/aws.rs) or new `cloudwatch.rs`
- [`rust-impl/crates/dql-cli/Cargo.toml`](rust-impl/crates/dql-cli/Cargo.toml) — optional `watch` feature / cloudwatch dep
- Python reference: `dql/engine.py` `get_capacity`

**Work**

- When `metrics=True` and backend is AWS (not Local/memory), fetch consumed RCU/WCU (Python uses a short CloudWatch window).
- Pass into table summary/detail formatters.
- No-op or warn on Local / memory.

**Gate:** Against a real table (or mocked CloudWatch), `ls tablename metrics=True` shows capacity columns.

**Commit:** `feat(cli): CloudWatch metrics for ls`

---

### Step 5 — Richer `ls` detail

**Files**

- [`rust-impl/crates/dql-output/src/table_meta.rs`](rust-impl/crates/dql-output/src/table_meta.rs)
- Optional format flag / always-on improved layout

**Work**

- Expand detail view: hash/range, LSI/GSI tables, throughput, status — information parity with Python `pformat(rich)`.
- Keep summary table for multi-table `ls`.

**Gate:** Snapshot or golden string test for a known `TableMeta`.

**Commit:** `feat(output): richer table detail for ls`

---

### Step 6 — `watch` dashboard

**Files**

- [`rust-impl/crates/dql-cli/src/meta/lifecycle.rs`](rust-impl/crates/dql-cli/src/meta/lifecycle.rs) — replace stub
- New `dql-cli/src/meta/watch.rs` or `monitor.rs`
- Feature flag `watch` in `dql-cli/Cargo.toml`
- Python reference: `dql/monitor.py`

**Work**

- Behind `--features watch`: ratatui (or crossterm) refresh loop (~30s) showing capacity for table globs.
- Without feature: keep current helpful rebuild message.
- Reuse CloudWatch helpers from Step 4.

**Gate:** Feature build runs `watch` against Local (skip metrics) or mocked CW; default build still compiles without cloudwatch.

**Commit:** `feat(cli): optional watch capacity dashboard`

## Out of scope

- Pixel-identical Rich / curses UI
- Fixing DynamoDB Local GSI throughput ALTER (`test_alter_index_throughput` — Local bug, also skipped in Python)

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| CloudWatch deps bloat default binary | Optional `watch` feature |
| less + ratatui fight over the TTY | Only use less outside alternate screen / non-REPL |
| Metrics flaky in CI | Mock CW client in unit tests; skip live metrics in CI |
