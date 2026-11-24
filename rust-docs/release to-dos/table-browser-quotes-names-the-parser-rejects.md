# Table browser quotes names the parser rejects

Status: open. Found while reviewing the table browser in `0.7.12-alpha.1`.

## What goes wrong

A table, index, or attribute whose name is a DQL word (`size`, `from`, `key`, `count`, `order`, and the rest of the highlighter list) cannot be read from the table browser. Run builds a statement and the parser rejects it.

`SCAN * FROM posts WHERE size = 1` is valid. The browser emits `SCAN * FROM posts WHERE 'size' = 1`. The same quoting wraps a table named `order` and an index named `key`.

A name that is not an identifier at all (`odd name`) is wrapped in single quotes the same way. The parser still rejects it, and the error is "expected identifier, got a string" rather than a message about the name.

## Why

`quoteIdent` in `dqlrs-web/static/table-query.js` single-quotes anything in the highlighter word list, and anything that fails `^[A-Za-z_][A-Za-z0-9_.-]*$`. It also doubles single quotes (`''`).

The lexer in `rust-impl/crates/dql-parser/src/lib.rs` treats `'...'` as a string, and it escapes with backslash, not `''`. `expect_ident` accepts only an identifier token. Keywords are ordinary identifiers unless the parser is looking for that keyword, so `size` and `from` are legal attribute and table names when written bare. `WHERE 'size' = 1` fails in `parse_field_path`.

`dqlrs-web/tests/test_table_query.mjs` locks in the quoted form (`quoteIdent("size") === "'size'"`).

## What to change

- Leave any name that matches the parser's identifier rule bare, including words the highlighter paints as keywords.
- When a name is not an identifier, do not emit a quoted string. Block Run with a message that the name cannot be used in DQL, and do not drop that filter silently.
- Update the table-query test so `size` and `from` stay bare, and a scan or query on a reserved attribute is the statement the parser accepts.
