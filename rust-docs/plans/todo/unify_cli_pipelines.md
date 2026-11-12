# TODO: Unify CLI one-shot and REPL execution

**Priority:** High  
**Smell / SOLID:** Dual pipelines (DRY), god `Session` / `ReplApp`, inverted
`meta` → `repl` dependency, thread-local control flow  
**Crates:** `dql-cli` (touches `dql-output` for presentation)

## Problem

### Dual execution paths

- `Session::run_command` / `dispatch_line` for `-c`
- `ReplApp::handle_submit` for interactive input

They overlap but diverge: rich `ls` is special-cased in the REPL, `clear` is
post-processed in the UI, and display/`less` / buffer rendering are not shared.

### God objects

`Session` + `RuntimeEngine` orchestrate config, connection, history, throttle,
output, meta dispatch, and engine access. `RuntimeEngine` is a large
`Memory | Remote` match façade. `handle_submit` mixes transcript, meta, rich
layout, and clear-screen side effects.

### Inverted dependency

`meta/ls.rs` imports `crate::repl::rich_table::{…}`. Meta-commands should not
depend on REPL presentation; the REPL then bypasses `ls::handle` for rich mode.

### Hidden control flow

`exit` and `watch` use thread-locals (`SHOULD_EXIT`, `PENDING_WATCH`) instead of
return values. Hard to test and reuse outside the REPL.

### Related

- Three copies of TTY/display-backend selection (`session`, `meta/file`,
  `repl` `BufferBackend`)
- History undo via magic `remove_items(1)` after meta commands
- `apply_rate_limit` calls `describe_all` before every fragment
- Tab completion reaches into `cached_descriptions` internals

## Recommendation

1. Single entry: `Session::execute_input(line, ExecutionContext) -> ExecutionOutcome`
   used by both `-c` and REPL.
2. Replace thread-locals with
   `enum MetaOutcome { None, Result(...), Exit, Watch(...), ClearScreen }`.
3. Move rich line renderers to `dql-output` (see
   `todo_invert_output_dependencies.md`). Meta exposes view data; presentation
   chooses text vs ratatui lines.
4. Slim `Session` into connection + config + engine handle; extract
   `CommandExecutor` / `OutputPipeline`.
5. One shared in-memory display backend; one history policy for meta commands
   (do not add, or explicit retract API).
6. Cache rate-limit resolution on throttle/table-list change, not per query.
7. Add `RuntimeEngine::cached_table_names()` for completion.

## Acceptance

- `-c` and REPL share one execution function; behavior diffs are explicit in
  `ExecutionContext` only.
- No `meta` → `repl` imports.
- No thread-locals for exit/watch.
- Rich `ls` and rich query results use the same presentation layer.
