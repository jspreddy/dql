# 07. A granted temp table can change after the grant

**Severity:** Medium. **Needs:** a write to the granted table: the person by mistake, a
statement in an agent-written file the person runs (06), or the shared token (02).

## What happens

A grant records the temp table's *generation*. `grant_live` keeps the grant while the table's
generation matches, and recreating the table gives it a new generation. `INSERT` does not change
the generation, so rows added to a granted table after the person reviewed it are readable
through `read_granted_table`.

The person grants what they saw in the grant panel (a classification and up to 20 sample rows).
Any rows added later go to the agent under that same grant.

DQL has no `INSERT … SELECT`, so a write can only add literal values. An agent cannot use this to
copy rows it has not seen; the person has to type sensitive values into a granted table. The
severity rises to High as soon as a statement that copies rows between tables (`INSERT … SELECT`,
`UPDATE … FROM`) exists.

## Reproduced

```text
CREATE TABLE src (id STRING HASH KEY);
INSERT INTO src (id) VALUES ('public'); INSERT INTO src (id) VALUES ('secret');
CREATE TEMPTABLE t AS (SELECT * FROM src WHERE id = 'public');
GRANT MCP READ ON TEMPTABLE t;
INSERT INTO t (id) VALUES ('secret-added-after-grant');
SHOW MCP GRANTS;
```

`SHOW MCP GRANTS` still listed `t` as granted, now with `rows : 2`. The row added after the
grant is readable by MCP.

## Code

- `dql-engine/src/virtual_tables.rs:254`: a generation is assigned only when a table is created.
- `:364` `grant`, `:484` `grant_live`: the grant is checked against the generation, which writes
  do not change.

## Fix

- Freeze a temp table when it is granted: refuse `INSERT`, `UPDATE`, `DELETE`, and `ALTER` on it,
  with a message to revoke first.
- Or bump the generation on every write, which ends the grant on the first change. The person
  grants again after seeing the new rows.
- Show the row count at grant time in `list_granted_tables` and in the grant panel, so a change
  is visible.
