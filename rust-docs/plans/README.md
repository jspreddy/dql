# Rust plans

Design notes, open work, and the record of finished plans for `dqlrs`. User
docs are one level up in [`rust-docs/`](../README.md). Code in
[`rust-impl/`](../../rust-impl/) is the source of truth; these files track
intent.

Python-only plans live in [`py-plans/`](../../py-plans/).

## Layout

```text
rust-docs/plans/
  README.md              # this index
  architecture.md        # crate and module boundaries (living)
  testing-strategy.md    # parity gates, test layers, CI gates (living)
  todo/                  # open design debt
  security/              # open security plans (not started)
  archive/               # finished plans, kept for history
    cursor/              # Cursor plan files that used to live in .cursor/plans/
```

## Living reference

Keep these current when the code changes.

| Document | Focus |
| --- | --- |
| [`architecture.md`](architecture.md) | Workspace crates, module boundaries, known boundary gaps |
| [`testing-strategy.md`](testing-strategy.md) | Parser, expression, planner, Local, CLI, and black-box test layers |

## Open: design debt (`todo/`)

Phases 1–6 are implemented in `rust-impl/`. A SOLID / code-smell review of that
tree produced these follow-ups. None are done yet.

| Todo | Priority | Focus |
| --- | --- | --- |
| [`split_monolith_modules.md`](todo/split_monolith_modules.md) | High | Split god files (`parser`, `expr`, `models`, `engine`) |
| [`typed_expression_ast.md`](todo/typed_expression_ast.md) | High | Typed selection/update AST end-to-end |
| [`engine_backend_boundaries.md`](todo/engine_backend_boundaries.md) | High | Slim `Engine` / `DynamoBackend`, plan-aware SDK delete, typed AWS errors |
| [`unify_cli_pipelines.md`](todo/unify_cli_pipelines.md) | High | One `-c` / REPL / `--serve` execution path; fix `meta`→`repl` dependency |
| [`invert_output_dependencies.md`](todo/invert_output_dependencies.md) | High | `dql-output` must not depend on `dql-engine`; shared rich renderer |
| [`dedupe_cross_cutting.md`](todo/dedupe_cross_cutting.md) | Medium | Shared base64, AWS config, bool parse, display backends |
| [`config_opt_registry.md`](todo/config_opt_registry.md) | Medium | Single `opt` / config registry; typed width/pagesize |

## Open: security (`security/`)

Plans for locking the on-disk caches and the local API. The query-result cache
stays in the workspace. Plans only; no code yet.

| Plan | Focus |
| --- | --- |
| [`security/README.md`](security/README.md) | Index and diagram |
| [`security/prd.md`](security/prd.md) | Product requirements, threat model, phases |
| [`security/disk-caches.md`](security/disk-caches.md) | Table cache and workspace query-result cache |
| [`security/encryption-and-keys.md`](security/encryption-and-keys.md) | AES-256-GCM and where the key lives |
| [`security/local-api.md`](security/local-api.md) | Web API, MCP, and `--serve` on loopback |
| [`security/agents.md`](security/agents.md) | What an agent can still read |

## Archive

Finished plans: the six rewrite phases, the parity plans, the headless worker,
the black-box harness, and the repo split. See
[`archive/README.md`](archive/README.md).

## Adding a plan

- New open work goes in `todo/` (design debt) or a topic folder like
  `security/`. Add a row to this index.
- When a plan is done, `git mv` it into `archive/` and move its row to
  [`archive/README.md`](archive/README.md). Do not rewrite it.
- Cursor saves new plans to `.cursor/plans/`. Once one is finished, move it to
  `archive/cursor/`.
