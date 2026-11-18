# Grant MCP read access to temp tables

Status: plan, for review. Branch `plan/mcp-temp-table-grants`.

## Why

MCP agents can search tables, lint, and write `.dql` files, but they never run queries and never see
data. Sometimes you want an agent to look at a result: a handful of rows to explain, a sample to write
a query against, a set to summarize. Today you have to copy and paste it.

This plan lets you hand an agent specific temp tables, on purpose, for as long as the server runs:

```sql
CREATE TEMPTABLE late_orders AS (SELECT * FROM orders WHERE status = 'late');
GRANT MCP READ ON TEMPTABLE late_orders;
```

The agent can then list what it was given and read those rows. Nothing else changes: it still cannot
run a query, and it still cannot read a DynamoDB table, a local table, or a temp table you did not grant.

## What exists

- **Temp tables** (`dql-engine/src/virtual_tables.rs`): `CREATE TEMPTABLE name AS (SELECT|SCAN …)` keeps
  the rows in the process's memory. They are never written to disk and are gone when the process exits.
  `DROP TEMPTABLE` removes one.
- **One session for the UI and MCP**: `dqlrs web --mcp` serves `/mcp` from the same `Session` (and so the
  same engine and temp tables) as the web UI (`web/server.rs`, `dispatch_mcp`).
- **MCP tools** (`dql-cli/src/mcp.rs`): `help`, `global_search`, `lint_query`, `lint_query_file`,
  `list_tables`, `describe_tables`, and the query-file tools. None of them read rows.

## Statements

```text
GRANT MCP READ ON TEMPTABLE name [, name …];
REVOKE MCP READ ON TEMPTABLE name [, name …];
REVOKE MCP READ ALL;
SHOW MCP GRANTS;
```

- **Only temp tables.** Granting a DynamoDB or local table is an error that says to make a temp table
  first (`CREATE TEMPTABLE … AS (SELECT …)`). That keeps the agent's view to exactly the rows you
  picked, as a snapshot.
- **In memory only.** Grants live in the engine next to the temp tables. They are never saved, cached,
  or synced, and end when the process exits.
- **Tied to the table, not the name.** `DROP TEMPTABLE` revokes its grant. A new temp table with the
  same name is not granted until you grant it again (each temp table gets a generation number; the
  grant stores it).
- **Results.** `GRANT` and `REVOKE` return a note, for example "Granted MCP read on temp table
  late_orders (3 rows) until this server stops." `SHOW MCP GRANTS` returns rows: table, rows, columns,
  granted at, and the query that made the table.
- **Where it applies.** The statements run anywhere the engine runs, but only a process that serves MCP
  shows grants to an agent: `dqlrs web --mcp`. In a process without MCP (the REPL, `-c`, the notebook,
  stdio `dqlrs --mcp`, which is its own process), `GRANT` still succeeds and its note says no agent can
  read it here.
- **Agents cannot grant.** MCP tools do not run statements, so an agent cannot grant itself access. It
  can write a `GRANT` into a `.dql` file with `create_query_file`, but you have to run it.

## MCP tools

| Tool | Arguments | Returns |
| --- | --- | --- |
| `list_granted_tables` | none | Each granted temp table: name, row count, columns (name, type, key role), granted at, and the query that made it. An empty list says how to ask the user for a grant. |
| `read_granted_table` | `table`; `limit` (default 100, at most 1000); `offset` (default 0); `attributes` (optional list to project) | `rows` as plain JSON (DynamoDB types unwrapped, like `--json`), `total`, `next_offset` (null at the end) |

- Reads come only from the in-memory temp table store. They never call DynamoDB, so no capacity is used
  and the agent's reach cannot grow past the granted rows.
- An ungranted, revoked, or dropped table gives a tool error that names `list_granted_tables`.
- `list_tables` and `describe_tables` mark a granted temp table (`"granted": true`), so an agent that
  finds it there knows it can read it.

## Web UI

- **Table list:** a granted temp table gets a small "MCP" badge beside its name, with a tip: "An agent
  can read these rows until the server stops."
- **Statements:** `GRANT`, `REVOKE`, and `SHOW MCP GRANTS` show their note or rows in the result pane like
  other statements. Lint and highlighting know the new keywords.
- **Reads are visible:** each `read_granted_table` call is listed in the History pane as an MCP read
  (table, rows returned, time), next to the user's own runs.

## Help and docs

- `HELP GRANT` and `HELP REVOKE` pages with examples; `HELP` lists them under Queries.
- `rust-docs/README.md`: an MCP section on grants, with the two tools.
- `dqlrs-web/examples/local-and-temp-tables/`: a grant example in the README and the `.dql` file.

## Steps

1. **Parser** (`dql-parser`): `GRANT`, `REVOKE`, and `SHOW MCP GRANTS` statements; round-trip and error tests.
2. **Engine** (`dql-engine`): grants in `VirtualTables` (name to generation and time), revoke on drop,
   `granted_tables()` and `read_granted(name, limit, offset, attributes)`; statement results; tests.
3. **Help**: `GRANT` and `REVOKE` topics; the help-examples test runs their examples.
4. **MCP** (`dql-cli/src/mcp.rs`): `list_granted_tables`, `read_granted_table`, and `granted` in
   `list_tables` and `describe_tables`; tests against a memory session (create, grant, list, read pages,
   project, revoke, drop, re-create).
5. **Web**: the table-list badge, History entries for MCP reads, lint and highlighting; a Playwright test
   with screenshots.
6. **Docs and examples.**

## Open questions

1. **Syntax.** `GRANT MCP READ ON TEMPTABLE name` (explicit, room to grow) or a shorter `GRANT MCP ON name`?
2. **Scope.** Temp tables only (this plan), or local (file) tables too?
3. **Limits.** 100 rows per read by default and 1000 at most; also a cap on the total an agent can read?
4. **Audit.** List MCP reads in the History pane (this plan), or only in the server log?
5. **Expiry.** Only "until the server stops", or also an optional `FOR 30 MINUTES`?
