# 11. Virtual-table metadata comes from the rows

**Severity:** Low. **Found by:** Cursor. **Needs:** only MCP (`list_tables`,
`describe_tables`). **Under `aws-vault exec`:** unchanged.

## What happens

For DynamoDB tables, `DescribeTable` returns keys and indexes, not the attributes on items. For
temp and local tables, `VirtualCatalog` infers columns by reading every attribute name across all
rows (`columns_for_saved_rows`, `infer_columns`). So `list_tables` and `describe_tables` on `/mcp`
show:

- the names of the person's ungranted temp tables (often descriptive: `late_orders_for_acme`);
- every attribute name present in their rows, and in local CSV and JSON tables;

and `describe_tables` on a local table reloads its file (`reload_local`).

Attribute names are metadata in the model, so this is within the letter of it. It is listed
because the names come from data the person has not granted, and names can carry data (a pivot
that turns values into columns).

## Code

- `dql-engine/src/virtual_tables.rs` `columns_for_saved_rows`, `infer_columns`, `reload_local`.
- `mcp.rs:215` `list_tables`, `:222` `describe_tables`.

## Status

Cursor's review, from the code.

## Fix

- Leave ungranted temp tables out of MCP's `list_tables` and `describe_tables`, or show their keys
  only.
- Decide in the model whether row-derived attribute names count as metadata, and document it.
