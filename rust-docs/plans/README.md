# Rust plans

Design notes, open work, and the record of finished plans for `dqlrs`. User
docs are one level up in [`rust-docs/`](../README.md). Code in
[`rust-impl/`](../../rust-impl/) is the source of truth; these files track
intent.

## Layout

```text
rust-docs/plans/
  README.md              # this index
  architecture.md        # crate and module boundaries (living)
  testing-strategy.md    # parity gates, test layers, CI gates (living)
  todo/                  # open design debt
  archive/               # finished plans, kept for history
    security/            # cache encryption, key store, API token (shipped)
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
tree produced most of these follow-ups; `installers.md` covers packaging.
None are done yet.

| Todo | Priority | Focus |
| --- | --- | --- |
| [`installers.md`](todo/installers.md) | High | Prebuilt binaries, CI releases, and an installer that works (README offers git-URL installs only until then) |
| [`split_monolith_modules.md`](todo/split_monolith_modules.md) | High | Split god files (`parser`, `expr`, `models`, `engine`) |
| [`typed_expression_ast.md`](todo/typed_expression_ast.md) | High | Typed selection/update AST end-to-end |
| [`engine_backend_boundaries.md`](todo/engine_backend_boundaries.md) | High | Slim `Engine` / `DynamoBackend`, plan-aware SDK delete, typed AWS errors |
| [`unify_cli_pipelines.md`](todo/unify_cli_pipelines.md) | High | One `-c` / REPL / `--serve` execution path; fix `meta`→`repl` dependency |
| [`invert_output_dependencies.md`](todo/invert_output_dependencies.md) | High | `dql-output` must not depend on `dql-engine`; shared rich renderer |
| [`dedupe_cross_cutting.md`](todo/dedupe_cross_cutting.md) | Medium | Shared base64, AWS config, bool parse, display backends |
| [`config_opt_registry.md`](todo/config_opt_registry.md) | Medium | Single `opt` / config registry; typed width/pagesize |
| [`split_web_app_js.md`](todo/split_web_app_js.md) | Medium | Split the web app's 5,850-line `app.js` into modules, leaves first |
| [`table_reads_server.md`](todo/table_reads_server.md) | High | Table browser sends its read as JSON; the server builds the statement. Overview of steps 1 to 6 |
| [`table_reads_1_contract.md`](todo/table_reads_1_contract.md) | High | The read request shape and the golden cases both builders must pass |
| [`table_reads_2_builder.md`](todo/table_reads_2_builder.md) | High | The Rust builder and validator, with DQL quoting the lexer reads |
| [`table_reads_3_endpoints.md`](todo/table_reads_3_endpoints.md) | High | `/api/table-read`, and `read` on `/api/rows` and `/api/explain` |
| [`table_reads_4_browser.md`](todo/table_reads_4_browser.md) | High | The browser sends its read and shows the server's preview |
| [`table_reads_5_cleanup.md`](todo/table_reads_5_cleanup.md) | High | Remove the `dql` path and the JavaScript builder |
| [`table_reads_6_statement.md`](todo/table_reads_6_statement.md) | Medium | Deferred: build the `Statement` directly, with a DQL printer |
| [`mcp_grant_flow.md`](todo/mcp_grant_flow.md) | Medium | Overall workflow: the agent requests a query, the user inspects and runs it, then grants or denies |
| [`mcp_grant_events.md`](todo/mcp_grant_events.md) | Medium | After the user grants, denies, or revokes, push that fact to MCP clients on a standing `/mcp` stream |
| [`mcp_grant_panel.md`](todo/mcp_grant_panel.md) | Medium | An agent's query pops up; the user inspects it, runs it, and grants or denies the result |
| [`mcp_grant_sensitivity.md`](todo/mcp_grant_sensitivity.md) | Medium | On that result, name sensitive attributes and values before the user grants |

## Archive

Finished plans: the six rewrite phases, the parity plans, the headless worker,
the black-box harness, the repo split, and the security work (sealed caches,
key store, API token). See
[`archive/README.md`](archive/README.md).

## Adding a plan

- New open work goes in `todo/` (design debt) or a new topic folder. Add a row
  to this index.
- When a plan is done, `git mv` it into `archive/` and move its row to
  [`archive/README.md`](archive/README.md). Do not rewrite it.
- Cursor saves new plans to `.cursor/plans/`. Once one is finished, move it to
  `archive/cursor/`.
