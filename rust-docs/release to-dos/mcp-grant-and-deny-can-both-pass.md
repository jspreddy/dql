# Grant and Deny can both pass on the same MCP request

Status: open. Found while reviewing MCP grants in `0.7.12-alpha.1`.

## What goes wrong

Two clicks at once, Grant in one tab and Deny in another, can both be accepted. The request can end denied in the panel while `GRANT MCP READ` has already run, or the temp table can be dropped and then granted again.

The agent is still waiting on `request_access`. It can be told the request was denied and still receive a readable temp table, or the reverse.

## Why

The web server handles each connection on its own thread. `grant_access` and `deny_access` in `rust-impl/crates/dql-cli/src/web/server.rs` both call into `Store` and then take `lock_state` only for the DQL.

`Store::grant` in `rust-impl/crates/dql-cli/src/web/access.rs` returns the table name when the phase is `Ran` and does not change the phase. `Store::deny` then still sees `Ran`, marks the request denied, and returns the table to drop. `finish_grant` only moves a request that is still `Ran`, so a deny that landed first is left denied while `GRANT` can still succeed. The two handlers do not hold the store lock across that window.

## What to change

- Under the store lock, make Grant and Deny exclusive. A grant in progress must reject Deny, and a completed deny must reject Grant.
- Keep the agent waiting until `GRANT` has actually finished. Do not report granted or denied in the middle of that statement.
- If `GRANT` fails, put the request back to `Ran` so Deny and another Grant still work.
- Add a store test where one side grants and the other denies: only one call succeeds.
