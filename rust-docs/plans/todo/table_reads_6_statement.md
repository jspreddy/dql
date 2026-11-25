# Table reads 6: build the statement, not text

**Priority:** Medium  
**Area:** `dql-parser`, `web/table_read.rs`, `web/server.rs`  
**Status:** Deferred. Step 6 of [`table_reads_server.md`](table_reads_server.md). Needs step 5.

## Goal

The server builds the `dql_parser::Statement` for a read directly and runs it with
`Engine::run`. The page limit goes in `QueryOptions` instead of appended text. The preview text
comes from printing that statement.

## Why it waits

Steps 2 to 5 write DQL text on the server and parse it, as today. That removes the JavaScript side
of the round trip, which is where the bugs were, and keeps one parser as the judge of what is
valid. This step removes the remaining server-side text round trip.

## Work

- A printer from `Statement` to DQL in `dql-parser`, for at least SELECT and SCAN with conditions,
  `USING`, `KEYS IN`, `ORDER BY`, `DESC`, and `LIMIT`. Round-trip test: `parse(print(s)) == s` over
  the parser's tests and the golden cases.
- `web/table_read.rs` builds `Statement` values. The preview prints them.
- `execute_for_serve` gains a form that takes a `Statement`, or the server calls `Engine::run` and
  builds the envelope itself.

## Out of scope

Using the printer anywhere else, such as the lint fixes or the MCP request text.
