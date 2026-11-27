# Table reads 3: the endpoints

**Priority:** High  
**Area:** `rust-impl/crates/dql-cli/src/web/server.rs`  
**Status:** Done on `table-reads/3-endpoints`. Step 3 of [`table_reads_server.md`](table_reads_server.md). Needs step 2.

## Goal

The server answers the browser's read requests from the step 2 builder.

## Endpoints

| Endpoint | Body | Answer |
| --- | --- | --- |
| `POST /api/table-read` | a read | `{ "ok": true, "dql": "...", "error": "" }`. `dql` is the statements joined with `; `. With an error, `dql` is `""` |
| `POST /api/rows` | `{ "read": {...}, "page": 0 }` | as today, with `dql` set to what ran |
| `POST /api/explain` | `{ "read": {...} }` | as today |

- `/api/table-read` is the preview. It runs nothing and does not take the session lock longer
  than the table lookup.
- `/api/rows` and `/api/explain` build the statements with the builder, then append the page
  `LIMIT` and run them as `table_rows` and `table_explain` do now. A builder error is a 400 with the
  message.
- The `GET` forms with `dql=` stay until step 5, so the browser can move over in step 4.
- The table definition comes from the same lookup as `/api/tables?name=`.

## Tests

Server tests for each endpoint: a scan with a filter, a query with an index, a Keys read on an
index (several statements), a value with a quote that now reads its row, and an error that names
the bad key.

## Out of scope

The browser change, and removing the `dql` forms.
