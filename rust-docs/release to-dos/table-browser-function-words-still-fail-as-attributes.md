# The table browser cannot filter on `size`, `not`, or a condition function name

Status: fixed on `fix/release-review-followups`. Found on `v-rust` at `90c492c` while reviewing #314 (`a93e9ff`).

## What goes wrong

An attribute named `size`, `not`, `contains`, `begins_with`, `attribute_exists`, `attribute_not_exists`, or `attribute_type` cannot be used as a key or a filter. The browser now emits it bare, `SCAN * FROM posts WHERE size = 1`, and the parser rejects that statement, as it rejected the quoted form before #314.

[`table-browser-quotes-reserved-words.md`](table-browser-quotes-reserved-words.md) gave `WHERE size = 1` as a valid statement. It is not. Checked with `parse_statement`:

| Attribute | `WHERE <name> = 1` | `ORDER BY <name>` |
| --- | --- | --- |
| `size`, `not`, `contains`, `begins_with`, `attribute_exists`, `attribute_not_exists`, `attribute_type` | rejected | accepted |
| `from`, `order`, `key`, `count`, `and`, `or`, `where`, `in`, `between`, `limit`, `using`, `desc`, `select`, `scan`, `keys` | accepted | accepted |

`dqlrs-web/tests/test_table_query.mjs` asserts the browser builds `SCAN * FROM posts WHERE size = 1`, which only checks the string, not that it parses.

## Why

In `rust-impl/crates/dql-parser/src/lib.rs`, the condition parser claims these words before it tries a field path:

- `parse_primary_condition` takes `NOT` as a negation whatever follows it.
- `parse_comparison` takes `SIZE`, then requires `(`.
- `peek_function_condition` matches `BEGINS_WITH`, `ATTRIBUTE_EXISTS`, `ATTRIBUTE_NOT_EXISTS`, `ATTRIBUTE_TYPE`, and `CONTAINS` by name alone, without looking for `(`.

So `size = 1` fails at the missing `(`, instead of falling back to a field named `size`.

## What to change

In the parser, treat `SIZE` and the function names as calls only when the next token is `(`, and `NOT` as a negation only when the next token is not a comparison operator. Otherwise read the word as a field path. Then `WHERE size = 1` and `WHERE contains = 'x'` parse as plain comparisons, and `size(tags) > 2` keeps working. Add parser tests for each word in both forms. Have the table-query test check that its statement parses, not only its text.
