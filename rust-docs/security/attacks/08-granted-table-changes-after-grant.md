# 08. A granted temp table can change after the grant

**Severity:** Medium; High once rows can be copied between tables. **Found by:** Claude.
**Needs:** a write to the granted table: the person by mistake, an agent-written file the person
runs (06, 07), or the shared token (02). **Under `aws-vault exec`:** unchanged.

## What happens

A grant records the temp table's generation, and `grant_live` keeps it while the generation
matches. Recreating the table changes the generation; `INSERT` does not. Rows added after the
person reviewed the table go to the agent under the same grant.

DQL has no `INSERT … SELECT` today, so a write can only add literal values. An agent cannot copy
rows it has not seen this way. The severity rises when a statement that copies rows
(`INSERT … SELECT`, `UPDATE … FROM`) is added.

## Reproduced

```text
CREATE TABLE src (id STRING HASH KEY);
INSERT INTO src (id) VALUES ('public'); INSERT INTO src (id) VALUES ('secret');
CREATE TEMPTABLE t AS (SELECT * FROM src WHERE id = 'public');
GRANT MCP READ ON TEMPTABLE t;
INSERT INTO t (id) VALUES ('secret-added-after-grant');
SHOW MCP GRANTS;
```

`SHOW MCP GRANTS` still listed `t`, now with `rows : 2`.

## Code

- `dql-engine/src/virtual_tables.rs:254`: generations are assigned only at creation.
- `:364` `grant`, `:484` `grant_live`.

## Fix

- Freeze a granted temp table: refuse writes until the grant is revoked.
- Or bump the generation on every write, which ends the grant on the first change.
- Show the row count at grant time in the panel and in `list_granted_tables`.
