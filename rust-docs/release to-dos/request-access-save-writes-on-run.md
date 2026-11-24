# request_access can SAVE a file when the user clicks Run

Status: open. Found while reviewing MCP grants in `0.7.12-alpha.1`.

## What goes wrong

An agent can ask for access with a read that also writes a file, for example `SCAN * FROM people SAVE 'people.json'` or `SAVE 'people.json' CREATE PARENTS`. The panel shows a read. Run creates the temp table by executing that statement, and the SAVE runs before anyone clicks Grant.

The file is written as the user who started `dqlrs web`, into the workspace or an absolute path the statement names. Grant is not required for that write.

## Why

`Store::check_query` in `rust-impl/crates/dql-cli/src/web/access.rs` accepts any single `SELECT` or `SCAN`. It does not look at `QueryOptions::save_file`.

`run_access` builds `CREATE TEMPTABLE mcp_rN AS (<query>)`. `create_temp_table` in `rust-impl/crates/dql-engine/src/engine.rs` runs that inner statement, and a SELECT or SCAN with `save_file` writes the rows in `maybe_save`.

The table browser already rejects SAVE (`browser_read_dql` in `web/server.rs`). `request_access` does not.

## What to change

- Reject a `SELECT` or `SCAN` whose options include `save_file` inside `check_query`, before the request is stored.
- Say that `request_access` cannot SAVE.
- Cover it in the existing `only_one_read_is_accepted` test, including `SAVE ... CREATE PARENTS`.
