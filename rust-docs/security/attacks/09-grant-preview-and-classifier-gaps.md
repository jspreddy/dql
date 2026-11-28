# 09. The grant panel shows 20 rows and classifies 1,000; the grant covers all

**Severity:** High. **Found by:** Cursor. **Needs:** only MCP (`request_access`,
`read_granted_table`) and a person who grants what the panel shows. **Under `aws-vault exec`:**
unchanged.

## What happens

`run_access` creates `mcp_r<N>` with every row the agent's query returns. It then:

- classifies only the first 1,000 rows (`temp_table_page(&table, 0, 1000)`, `SCAN_CAP` in
  `web/classify.rs`);
- shows only the first 20 (`.take(20)`, `renderGrantRows`).

**Grant** exposes all `total` rows, and `read_granted_table` takes any `offset`.

1. **Rows past the preview.** The agent orders its query so the first 20 (or 1,000) rows are
   harmless and the sensitive ones come after. The panel says "No sensitive attributes or values
   found" and shows clean rows. After the grant, `read_granted_table` with `offset: 1000` returns
   the tail.
2. **Values the classifier skips.** `walk_value` ignores `Value::Binary`, `Timestamp`,
   `Interval`, and `Null`. Secrets stored in a binary attribute (JSON, PEM keys, JWTs) raise no
   warning. Attribute-name checks match a fixed list, so `payload`, `data`, or `notes` pass, and
   value checks match fixed patterns (email, SSN, card numbers, phone numbers, AWS keys, bearer
   tokens, PEM, JWT).

## Code

- `web/server.rs:1436` `run_access`: `CREATE TEMPTABLE`, `temp_table_page(…, 0, 1000)`,
  `.take(20)`.
- `web/classify.rs` `SCAN_CAP`, `walk_value`, `name_pattern`, `NAME_PATTERNS`.
- `mcp.rs:313` `read_granted_table`, `virtual_tables.rs:439` `read_granted`.

## Status

Cursor's review, from the code. Not run in this reconciliation.

## Fix

- Grant only what was classified and shown: cap `request_access` results (for example 1,000
  rows), classify every stored row, and let the person page through all of them before Grant. Or
  truncate `mcp_r<N>` to the classified slice.
- Classify binary values: scan their bytes as text and as base64, and say in the panel when binary
  or opaque attributes are present.
