# Archived Rust plans

Finished plans, kept for history. They describe the code as it was when each
plan was written. Paths and "current state" sections inside them are not kept
up to date, and links inside `cursor/` files may point at old locations
(`rust-plans/`, `.cursor/plans/`). Use [`rust-impl/`](../../../rust-impl/) and
the open plans in [`../README.md`](../README.md) for the current picture.

## Rewrite (phases 1–6)

| Plan | Focus |
| --- | --- |
| [`migration-roadmap.md`](migration-roadmap.md) | The six compatibility phases, all done |
| [`cursor/phase_5_cli_repl_1d33257c.plan.md`](cursor/phase_5_cli_repl_1d33257c.plan.md) | `dql-output`, meta-commands, config/history, ratatui REPL |
| [`cursor/phase_6_packaging.plan.md`](cursor/phase_6_packaging.plan.md) | Release builds, Rust CI, smoke tests, GitHub release binaries |

## Python parity (post–phase 6)

| Plan | Focus |
| --- | --- |
| [`cursor/parity_1_default_aws.plan.md`](cursor/parity_1_default_aws.plan.md) | Default live AWS connection (not memory) |
| [`cursor/parity_2_order_by.plan.md`](cursor/parity_2_order_by.plan.md) | ORDER BY / ASC / DESC / ScanIndexForward |
| [`cursor/parity_3_index_projection.plan.md`](cursor/parity_3_index_projection.plan.md) | Non-projected index attrs + count on index |
| [`cursor/parity_4_expression_regressions.plan.md`](cursor/parity_4_expression_regressions.plan.md) | Reserved words + dashed field paths |
| [`cursor/parity_5_save_load.plan.md`](cursor/parity_5_save_load.plan.md) | SAVE/LOAD; MessagePack replaces pickle |
| [`cursor/parity_6_cli_polish.plan.md`](cursor/parity_6_cli_polish.plan.md) | less, caret errors, metrics, rich ls, watch |
| [`cursor/rich_format_ratatui_78c8339d.plan.md`](cursor/rich_format_ratatui_78c8339d.plan.md) | `opt format rich` query results via ratatui |

## Headless worker (`--serve`)

| Plan | Focus |
| --- | --- |
| [`headless_worker.md`](headless_worker.md) | `dqlrs --serve` JSON-lines worker (stdio or loopback `--bind`, one `Session`) |
| [`cursor/dqlrs_headless_worker.plan.md`](cursor/dqlrs_headless_worker.plan.md) | Implementation todos for the same work |

The user-facing description is in [`rust-docs/README.md`](../../README.md#headless-worker---serve).

## Black-box acceptance tests

| Plan | Focus |
| --- | --- |
| [`cursor/black_box_acceptance_tests.plan.md`](cursor/black_box_acceptance_tests.plan.md) | Language-agnostic acceptance suite against both binaries |
| [`cursor/pexpect_black-box_harness_e74fc52e.plan.md`](cursor/pexpect_black-box_harness_e74fc52e.plan.md) | pytest + pexpect harness |
| [`cursor/rename_case_step_files_5ef1f739.plan.md`](cursor/rename_case_step_files_5ef1f739.plan.md) | Run-order prefixes for case step files |
| [`cursor/rich_verbose_transcripts_763cbe3a.plan.md`](cursor/rich_verbose_transcripts_763cbe3a.plan.md) | Rich `-v` transcripts per test |

One item was left open in `black_box_acceptance_tests.plan.md`: an optional CI
job that builds both binaries, starts DynamoDB Local, and runs the harness.

## Repository layout

| Plan | Focus |
| --- | --- |
| [`cursor/segregate_python_rust_ae06315b.plan.md`](cursor/segregate_python_rust_ae06315b.plan.md) | Split the repo into sibling `py-*` and `rust-*` trees |

## Original rewrite brief

This was the top of `rust-plans/README.md` while the rewrite was in progress.

### Goals

1. Preserve DQL syntax and command behavior before expanding the language.
2. Keep parser, planner, executor, and terminal UI concerns isolated.
3. Use typed Rust domain models instead of carrying Python parse result shapes
   across layers.
4. Treat the existing Python tests and documentation examples as the behavioral
   oracle for the first Rust implementation.
5. Make DynamoDB Local integration tests the main proof that the Rust engine is
   compatible with existing query behavior.

### Python surfaces used as the oracle

- CLI entrypoint and REPL: `py-impl/dql/__init__.py` and `py-impl/dql/cli.py`
- Query execution and DynamoDB integration: `py-impl/dql/engine.py`
- Grammar and parse tree construction: `py-impl/dql/grammar/`
- Expression AST and DynamoDB expression rendering: `py-impl/dql/expressions/`
- Table and index metadata: `py-impl/dql/models.py`
- Output, paging, JSON handling, history, help, and monitoring:
  `py-impl/dql/output.py`, `py-impl/dql/history.py`, `py-impl/dql/help.py`, and
  `py-impl/dql/monitor.py`
- Compatibility tests: `py-impl/tests/test_parser.py`,
  `py-impl/tests/test_queries.py`, `py-impl/tests/test_engine.py`,
  `py-impl/tests/test_cli.py`, and related focused tests

### Non-goals for the first pass

- Changing DQL syntax.
- Adding new DynamoDB features before parity is established.
- Replacing the documented CLI workflow with an incompatible interface.
- Preserving Python-specific serialization formats such as pickle unless a
  compatibility requirement is explicitly accepted. SAVE/LOAD binary format in
  Rust uses MessagePack instead (see
  [`cursor/parity_5_save_load.plan.md`](cursor/parity_5_save_load.plan.md)).
