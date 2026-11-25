# Table reads 1: the request shape and the golden cases

**Priority:** High  
**Area:** `dqlrs-web` table browser, `dqlrs web` server  
**Status:** Done on `table-reads/1-contract`. Step 1 of [`table_reads_server.md`](table_reads_server.md).

## Goal

Fix the JSON the browser sends for a read. Record what each read builds today as golden cases,
which both the current JavaScript builder and the new Rust builder must pass.

## The request

```json
{
  "table": "posts",
  "mode": "scan",
  "source": "",
  "order": "forward",
  "keys": [
    { "attribute": "id", "op": "eq", "value": "a", "value2": "", "role": "hash", "sizeOp": "gt" }
  ],
  "filters": [
    { "attribute": "status", "op": "eq", "value": "open", "value2": "", "sizeOp": "gt" }
  ],
  "batchKeys": "a, (b, 1)",
  "extra": ["name", "tags"]
}
```

| Field | Meaning |
| --- | --- |
| `table` | The table name |
| `mode` | `scan`, `query`, or `keys` |
| `source` | `""` lets dqlrs choose (no USING), `"-"` is the table (`USING -`), anything else names an index |
| `order` | `forward` or `reverse` |
| `keys`, `filters` | The rows as the browser holds them. `op` is an id from the browser's op lists (`eq`, `between`, `begins_with`, `size`, …). `value2` is BETWEEN's second value. `sizeOp` is the comparison inside `size` |
| `batchKeys` | The Keys box text, in Keys mode |
| `extra` | Attribute names seen in rows that the table definition does not list. They have no type |

The server looks up the table's definition itself, as the `ListedTable` that `/api/tables`
returns (`keys`, `indexes`, `attributes` with types). The browser does not send it.

## Golden cases

`rust-impl/crates/dql-cli/tests/fixtures/table_reads.json`: a list of cases. Each has a `table`
(a `ListedTable`), a `read` (the request above without `table`), and what it builds: `dql` (the
statement text, `""` when none) and `error` (the message, `""` when none). The file holds every
read that `dqlrs-web/tests/test_table_query.mjs` covers today, plus:

- a string value with a `'`, and one with a `\`
- a table whose key name is not an identifier
- each mode with an index source, the table source, and `""`

Write the cases from what the JavaScript builder produces now, except where it is wrong today.
Mark those with `"fixes"` naming the bug, such as `"value with a quote"`.

## Tests

- A Node test ran every case through the JavaScript `buildReadStatement` and `readError`,
  skipping cases marked `fixes`, until step 5 removed that builder. The file is now the Rust
  builder's test data.
- The Rust builder in step 2 runs every case, including the `fixes` cases.

## Out of scope

The Rust builder, the endpoints, and any change to the browser.
