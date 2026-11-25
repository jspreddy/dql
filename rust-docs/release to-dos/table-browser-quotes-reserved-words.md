# Table browser quotes reserved words the parser rejects

Status: partly fixed in #314 (`a93e9ff`). Most keywords now stay bare and parse. `size`, `not`, and the condition function names still fail: [`table-browser-function-words-still-fail-as-attributes.md`](table-browser-function-words-still-fail-as-attributes.md).

## What goes wrong

A table, index, or attribute whose name is a DQL word cannot be read from the table browser. Run builds a statement and the parser rejects it.

`SCAN * FROM posts WHERE size = 1` is valid. The browser emits `SCAN * FROM posts WHERE 'size' = 1`. The same quotes wrap a table named `order` and an index named `key`. The words are the highlighter list: `size`, `from`, `key`, `count`, `order`, and the rest in `dqlrs-web/static/dql-tokens.js`.

## Why

`quoteIdent` in `dqlrs-web/static/table-query.js` single-quotes any name in that list. `buildReadStatement` and `formatCondition` use it for the table, the key, and the filter.

The lexer in `rust-impl/crates/dql-parser/src/lib.rs` treats `'...'` as a string. `expect_ident` accepts only an identifier token. A keyword is an ordinary identifier unless the parser is looking for that keyword, so `size` and `from` are legal names when written bare. `WHERE 'size' = 1` fails in `parse_field_path`.

`dqlrs-web/tests/test_table_query.mjs` locks in the quoted form (`quoteIdent("size") === "'size'"`).

## What to change

Leave any name that matches the parser's identifier rule bare, including words the highlighter paints as keywords. Update the table-query test so `size` and `from` stay bare, and a scan or query on one of those attributes is the statement the parser accepts.
