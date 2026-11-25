# The table browser drops filters when a key name is not an identifier

Status: open. Found on `v-rust` at `90c492c` while reviewing #314 (`a93e9ff`).

## What goes wrong

DynamoDB allows any attribute name, so a table's hash key can be `user id`. In the table browser, Scan that table with a filter such as `status = 'open'` and click Run.

- The DQL preview is empty, and there is no error message.
- Run stays enabled.
- The rows that come back are every row in the table. The filter is not applied, and nothing says so.

Before #314 the same scan built `SCAN * FROM people WHERE status = 'open'` and worked: the key name only mattered when the statement used it.

## Why

`buildReadStatement` in `dqlrs-web/static/table-query.js` now returns `""` when any name from `statementNames` fails the identifier rule. `statementNames` includes the chosen source's `hashKey` and `rangeKey`, even in a Scan that does not use them.

`readError` checks the table, the key rows, the filter rows, and the index name. It does not check `hashKey` or `rangeKey`. So the statement is empty and the error is empty.

On Run, `onRun` in `dqlrs-web/static/app.js` sends that empty statement as `dql`. `browser_read_dql` in `rust-impl/crates/dql-cli/src/web/server.rs` turns an empty `dql` into `SCAN * FROM <table> LIMIT n`, the plain first page.

A Node check against the module: for that table and filter, `buildReadStatement` returns `""` and `readError` returns `""`. With the key named `id`, the same state builds `SCAN * FROM people WHERE status = 'open'`.

## What to change

Check only the names the statement will contain. A Scan or Query that does not use the source key does not need its name. When a name the statement does need is not an identifier, `readError` names it, so Run is blocked with a message. Never let an empty statement reach Run while filters are set. Cover a table whose hash key is `user id` in `dqlrs-web/tests/test_table_query.mjs`: a filtered Scan builds its statement, and a Query on that key reports the key name.
