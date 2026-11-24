# Invalid identifiers corrupt table, key, and filter queries

Status: open. Found on `0.7.12-alpha.1` while reviewing the table browser.

## What goes wrong

A table, key, or filter whose name is not an identifier is still spliced into the statement. The read that runs is not a read of that name.

A space in the name becomes a string where the parser expects an identifier: `SCAN * FROM 'odd name'`, `WHERE 'odd name' = 'open'`. A name that contains `'` is worse. `quoteIdent` turns `o'brien` into `'o''brien'`. The lexer does not treat `''` as an escaped quote, so the name is split into the tokens `o` and `brien`. That happens in all three places the browser inserts a name:

- the table, after `FROM` (`buildReadStatement`, `formatIndexKeySelect`)
- a key condition (`formatCondition` for a key row, and the hash and range names in `formatIndexKeySelect`)
- a filter (`formatCondition` for a filter row)

The tail of the name is then more of the statement. Run reports a parse error, or the condition is no longer the attribute the user chose.

## Why

`quoteIdent` in `dqlrs-web/static/table-query.js` single-quotes anything that fails `^[A-Za-z_][A-Za-z0-9_.-]*$`, and it doubles single quotes (`''`).

`read_quoted` in `rust-impl/crates/dql-parser/src/lib.rs` ends a string at the next `'`. It escapes with a backslash, not `''`. `parse_field_path` and the table name both go through `expect_ident`, which rejects a string token. Nothing in the browser refuses the name before it is inserted, so the bad text is part of the query for the table, the keys, and the filters.

## What to change

Do not emit a quoted string for a name the parser cannot accept as an identifier. Block Run with a message that names the table, key, or filter, and do not drop that part of the statement silently. Cover a space and an embedded `'` in `dqlrs-web/tests/test_table_query.mjs` for a table name, a key, and a filter.
