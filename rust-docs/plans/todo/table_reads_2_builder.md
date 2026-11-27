# Table reads 2: the Rust builder and validator

**Priority:** High  
**Area:** `rust-impl/crates/dql-cli/src/web/`  
**Status:** Done on `table-reads/2-builder`. Step 2 of [`table_reads_server.md`](table_reads_server.md). Needs step 1.

## Goal

One Rust module turns a read request and a `ListedTable` into the statements to run and the
message to show. It does what `buildReadStatement` and `readError` do in `table-query.js` today,
with DQL quoting done the way the lexer reads it.

## Module

`web/table_read.rs`:

- `TableRead`, `ReadRow`: the request types from step 1 (`serde`).
- `fn plan(table: &ListedTable, read: &TableRead) -> Result<Vec<String>, String>`: the statements
  in run order (Keys mode on an index gives one SELECT per key), or the message. It never returns
  an empty list without a message.
- Helpers ported from `table-query.js`: the op lists, `sourceOf`, `sourceKeys`, `ensureQueryKeys`,
  the Keys box parser, key dedupe, `keyValueError`, `keysModeError`, and the name check that lists
  only the names the statement contains.

## Quoting

- A name is written bare when it matches the parser's identifier rule (`[A-Za-z_][A-Za-z0-9_.-]*`),
  including keywords. Otherwise it is an error that names it: "<name> cannot be used as a key name".
- A string value is written `'...'` with `\` written `\\` and `'` written `\'`, which is how
  `read_quoted` in `dql-parser` reads it.
- A NUMBER is written as typed when it is a number, and is an error otherwise. A value of an
  attribute with no type follows today's rule: a plain number or `true`/`false`/`null` is written
  bare, anything else is a string.

## Tests

- Every golden case from step 1, including the `fixes` cases.
- Every statement the builder returns parses with `parse_statement` and reads the same table. A
  property-style test runs this over all the golden cases.

## Out of scope

The endpoints and the browser. Building a `Statement` directly is step 6.
