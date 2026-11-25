# Grant and Deny can both pass, and Run can write a file first

Status: fixed in #312 (`85fe411`). That fix brought a regression: [`mcp-grant-reservation-sticks-after-a-drop.md`](mcp-grant-reservation-sticks-after-a-drop.md).

## What goes wrong

Two clicks at once, Grant in one tab and Deny in another, can both be accepted. The request can end denied in the panel while `GRANT MCP READ` has already run, or the temp table can be dropped and then granted again. The agent is still waiting on `request_access`. It can be told the request was denied and still receive a readable temp table, or the reverse.

The same Run path can write a file before any grant. An agent can ask for access with `SCAN * FROM people SAVE 'people.json'`, or `SAVE 'people.json' CREATE PARENTS`. The panel shows a read. Run creates the temp table by executing that statement, and the SAVE runs before anyone clicks Grant. The file is written as the user who started `dqlrs web`, into the workspace or an absolute path the statement names.

## Why

The web server handles each connection on its own thread. `grant_access` and `deny_access` in `rust-impl/crates/dql-cli/src/web/server.rs` both call into `Store` and then take `lock_state` only for the DQL.

`Store::grant` in `rust-impl/crates/dql-cli/src/web/access.rs` returns the table name when the phase is `Ran` and does not change the phase. `Store::deny` then still sees `Ran`, marks the request denied, and returns the table to drop. `finish_grant` only moves a request that is still `Ran`, so a deny that landed first is left denied while `GRANT` can still succeed. The two handlers do not hold the store lock across that window.

`Store::check_query` accepts any single `SELECT` or `SCAN`. It does not look at `QueryOptions::save_file`. `run_access` builds `CREATE TEMPTABLE mcp_rN AS (<query>)`. `create_temp_table` in `rust-impl/crates/dql-engine/src/engine.rs` runs that inner statement, and a SELECT or SCAN with `save_file` writes the rows in `maybe_save`. The table browser already rejects SAVE (`browser_read_dql` in `web/server.rs`). `request_access` does not. The write happens on Run, before Grant.

## What to change

Under the store lock, make Grant and Deny exclusive. A grant in progress must reject Deny, and a completed deny must reject Grant. Keep the agent waiting until `GRANT` has finished. If `GRANT` fails, put the request back to `Ran`. Add a store test where one side grants and the other denies: only one call succeeds.

Reject a `SELECT` or `SCAN` whose options include `save_file` inside `check_query`, before the request is stored, and say that `request_access` cannot SAVE. Cover `SAVE` and `SAVE ... CREATE PARENTS` in `only_one_read_is_accepted`.
