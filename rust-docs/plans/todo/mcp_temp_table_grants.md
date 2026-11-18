# Grant MCP read access to temp tables

Status: ready to build. Nothing is implemented yet. This file is a handoff: it should be enough to
do the work without the conversation that produced it.

## Goal

Let the user hand an MCP agent (Cursor, Claude) specific temp tables to read, on purpose, for as long as
the `dqlrs` process runs:

```sql
CREATE TEMPTABLE late_orders AS (SELECT * FROM orders WHERE status = 'late');
GRANT MCP READ ON TEMPTABLE late_orders;
```

The agent then lists what it was given and reads those rows through two new MCP tools. Nothing else
about MCP changes: an agent still cannot run a query, cannot grant itself anything, and cannot read a
DynamoDB table, a local (file) table, or a temp table that was not granted.

## Background: what exists today

| Piece | Where | What it does |
| --- | --- | --- |
| Temp tables | `rust-impl/crates/dql-engine/src/virtual_tables.rs` (`VirtualCatalog`, `VirtualTable`, `TableKind::Temp`, `create_temp`, `drop_kind`, `item_count`) | `CREATE TEMPTABLE name AS (SELECT\|SCAN …)` copies rows into an in-memory backend inside the engine. Never written to disk; gone when the process exits. `DROP TEMPTABLE` removes one. |
| Statements | `rust-impl/crates/dql-parser/src/lib.rs` (`enum Statement`, about line 257; `CreateTempTable`, `DropTempTable`, `ShowTables`) | Parser for every DQL statement. |
| Execution | `rust-impl/crates/dql-engine/src/engine.rs` (the `match` on `Statement` around line 620; `StatementResult` in `dql-engine/src/lib.rs`) | Runs a parsed statement and returns `Status`, `Items`, `ItemsWithNote`, and so on. |
| Help | `rust-impl/crates/dql-engine/src/help.rs` (`GROUPS`, `topic_help`), `help_body.rs` (page text) | `HELP` and `HELP <topic>`. A test runs every example in every help page in the web, CLI, and notebook paths (`dql-cli/src/help.rs`, `help_examples_run_from_web_cli_notebook_and_command`). |
| MCP server | `rust-impl/crates/dql-cli/src/mcp.rs` (`tool_definitions`, `call_tool`, tests at the bottom) | Tools: `help`, `global_search`, `lint_query`, `lint_query_file`, `list_tables`, `describe_tables`, and the `.dql` file tools. None read rows. |
| One session for UI and MCP | `rust-impl/crates/dql-cli/src/web/server.rs` (`dispatch_mcp`) | `dqlrs web --mcp` answers `/mcp` with the same `Session` (so the same engine and temp tables) as the web UI. Stdio `dqlrs --mcp` is a separate process with its own session. |
| Table list | `rust-impl/crates/dql-cli/src/meta/ls.rs` (`web_table_list`, `listed_table`); `dqlrs-web/static/app.js` (`tableKind`, `tableMarker`, `paintTables`) | Lists tables with a `kind` (`dynamodb`, `local`, `temp`); the UI shows a small kind marker. |
| Live events | `web/server.rs` (`app.hub`), `web/events.rs`; `app.js` (`startEvents`) | Server-sent events from the server to every open tab. |
| Keywords | `dqlrs-web/static/dql-tokens.js` (keyword list, line 17), `rust-impl/crates/dql-cli/src/repl/highlight.rs` (`KEYWORDS`) | Highlighting in the web editor and the REPL. |

## Decisions

These are the defaults for this work. The owner can still change any of them; the first five were open
questions at planning time and have no answer yet.

| Question | Default |
| --- | --- |
| Syntax | `GRANT MCP READ ON TEMPTABLE name` (explicit; leaves room for other rights and table kinds) |
| Scope | Temp tables only. Granting a DynamoDB or local table is an error. |
| Read limits | 100 rows per read by default, 1000 at most. No cap on the total. |
| Audit | Each MCP read shows in the web UI's History pane. |
| Expiry | None beyond "until the process exits". No `FOR 30 MINUTES`. |
| Storage | In memory only, in the engine. Never saved, cached, synced, or sealed to disk. |
| Identity | A grant belongs to one temp table, not to a name. `DROP TEMPTABLE` revokes it; a new temp table with the same name is not granted. |
| Who can grant | Only statements the user runs. MCP tools never execute statements. |

## Statements

```text
GRANT MCP READ ON TEMPTABLE name [, name …];
REVOKE MCP READ ON TEMPTABLE name [, name …];
REVOKE MCP READ ALL;
SHOW MCP GRANTS;
```

| Statement | Result |
| --- | --- |
| `GRANT …` | `Status`: "Granted MCP read on temp table late_orders (3 rows) until this server stops." One line per table. Granting an already granted table succeeds and says so. |
| `REVOKE … ON TEMPTABLE …` | `Status`: "Revoked MCP read on temp table late_orders." Revoking one that is not granted is not an error ("…was not granted"). |
| `REVOKE MCP READ ALL` | `Status`: "Revoked MCP read on 2 temp tables." (or "No MCP grants.") |
| `SHOW MCP GRANTS` | `Items`, one per grant: `table`, `rows`, `columns` (comma-separated `name type` with the key role), `granted_at` (RFC 3339), `query` (the temp table's source query). `ItemsWithNote` with "No MCP grants." when empty. |

Errors:

- Unknown name: "No temp table named 'x'."
- A DynamoDB or local table: "'orders' is a DynamoDB table. Only temp tables can be granted: CREATE TEMPTABLE name AS (SELECT …), then GRANT MCP READ ON TEMPTABLE name."
- `GRANT` without `MCP READ ON TEMPTABLE`: the usual parse error, with the syntax in it.

Where it applies: the engine runs these statements anywhere (web, REPL, `-c`, notebook, `--serve`). Only a
process that serves MCP lets an agent read the grant, which today means `dqlrs web --mcp`. In any other
process the `GRANT` note adds: "No agent can read it here: only `dqlrs web --mcp` serves grants." The
`Session` needs a flag for "this process serves MCP" (set in `web::run` when `--mcp` is on) so the CLI
layer can add that sentence; the engine itself does not know about MCP.

## MCP tools

### `list_granted_tables`

No arguments. Returns:

```json
{
  "tables": [
    {
      "name": "late_orders",
      "rows": 3,
      "columns": [{ "name": "id", "type": "STRING", "key": "hash" }, { "name": "status", "type": "STRING" }],
      "granted_at": "2026-11-18T10:02:11Z",
      "query": "SELECT * FROM orders WHERE status = 'late'"
    }
  ]
}
```

With no grants: `{"tables": [], "hint": "Nothing is granted. Ask the user to run CREATE TEMPTABLE … AS (SELECT …) and GRANT MCP READ ON TEMPTABLE name in dqlrs web."}`

### `read_granted_table`

Arguments: `table` (required), `limit` (default 100, 1 to 1000), `offset` (default 0), `attributes`
(optional list of attribute names to return).

Returns `{"table": "late_orders", "rows": [ … ], "total": 3, "offset": 0, "next_offset": null}`.
Rows are plain JSON with DynamoDB types unwrapped, the same shape `dqlrs -c … --json` prints. A missing
attribute is left out of that row.

Errors (as MCP tool errors, `isError: true`): the table is not granted, was revoked, or was dropped
("'x' is not granted. Call list_granted_tables to see what is."); `limit` out of range; `attributes` not
a list of strings.

### Rules for both

- Read only from the engine's in-memory temp table store (`VirtualCatalog`). Never call DynamoDB, so no
  capacity is used and the agent cannot reach past the granted rows.
- Hold the session lock only while copying the page out, as other tools do.
- `list_tables` and `describe_tables` add `"granted": true` to a granted temp table.
- Update the server's MCP instructions text and each tool description so an agent knows grants exist and
  that the user makes them.

## Web UI

- **Table list:** a granted temp table gets an "MCP" marker after its kind marker (reuse the
  `.table-kind` style, `data-kind="mcp"`). Its tooltip, medium delay: "An agent can read these rows until
  the server stops." See `dqlrs-web/DESIGN.md`, Tooltips, for the tiers.
- **Statements:** `GRANT`, `REVOKE`, and `SHOW MCP GRANTS` show in the result pane like other statements.
  Add `GRANT`, `REVOKE`, `MCP`, `READ`, `GRANTS` to the keyword lists for highlighting, and make sure lint
  accepts them (the web linter calls the parser).
- **Refresh:** after a grant or revoke, the table list repaints so the marker appears or goes. Use the
  existing hub: send a small event and have `app.js` re-fetch the table list.
- **Audit:** each `read_granted_table` call sends a hub event (table, rows returned, offset). Each open
  tab adds a History entry such as "MCP read late_orders: 3 rows" with the time. These entries are not
  saved.

## Steps

Do these in order. Each step should leave `just fix` and `just check` passing.

1. **Parser.** Add `Statement::Grant { tables }`, `Statement::Revoke { tables: Option<Vec<String>> }`
   (`None` for `ALL`), and `Statement::ShowMcpGrants`. Parse `GRANT MCP READ ON TEMPTABLE a, b`,
   `REVOKE MCP READ ON TEMPTABLE a`, `REVOKE MCP READ ALL`, `SHOW MCP GRANTS`. Tests: each form, case
   insensitivity, missing parts give parse errors, and `SHOW TABLES` still parses.
2. **Engine.** In `VirtualCatalog`, give each temp table a generation number (incremented on create) and
   keep `grants: HashMap<String, Grant { generation, granted_at }>`. A grant is live only while the name's
   current generation matches. `drop_kind` removes the grant. Add `grant`, `revoke`, `revoke_all`,
   `granted() -> Vec<GrantInfo>`, and `read_granted(name, limit, offset, attributes) -> Result<Page>`. Wire
   the statements into `engine.rs` with the results and errors above. Tests: grant, re-grant, revoke,
   revoke all, drop revokes, re-create is not granted, DynamoDB and local tables refused, pages and
   projection, `SHOW MCP GRANTS` rows.
3. **Help.** `HELP GRANT` and `HELP REVOKE` pages in `help_body.rs`, listed in `GROUPS` under Queries.
   Their examples must run: each one creates its own temp table first (for example
   `CREATE TEMPTABLE g AS (SCAN * FROM foobars); GRANT MCP READ ON TEMPTABLE g;`). Run the help-examples
   test.
4. **Session flag.** `Session::serves_mcp` (or similar), set by `web::run` when `--mcp` is on; the CLI
   adds the "No agent can read it here" sentence when it is false.
5. **MCP tools.** In `mcp.rs`: `list_granted_tables`, `read_granted_table`, the `granted` field in
   `list_tables` and `describe_tables`, and updated descriptions. Tests against a memory session (see
   the existing tests at the bottom of `mcp.rs`): nothing granted, grant then list, read pages
   (`next_offset`), projection, limit bounds, ungranted table, revoke, drop, re-create.
6. **Web.** The "MCP" marker, the refresh and audit hub events, keywords. Add a Playwright test under
   `dqlrs-web/playwright/tests/` (for example `mcp-grants.spec.js`):
   - create a temp table and run `GRANT` in the editor, check the result note and the table-list marker;
   - call `/mcp` (`tools/call` `read_granted_table`) with the API token from the page, check the rows;
   - check the History entry for that read;
   - `REVOKE`, check the marker is gone and the tool call now fails.
   Save screenshots with `playwrightArtifact`, light and dark.
7. **Docs.**
   - `rust-docs/README.md`: an "MCP grants" part in the MCP section with the statements and both tools.
   - Top-level `README.md`: one or two sentences where MCP is described, linking to rust-docs.
   - `dqlrs-web/examples/local-and-temp-tables/`: a grant example in `README.md` and `local-tables.dql`.
   - When done, `git mv` this file to `rust-docs/plans/archive/` and move its row in
     `rust-docs/plans/README.md`.

## Done when

- The statements, results, and errors above work in the web editor and in `dqlrs -c`.
- In `dqlrs web --mcp`, an agent can list and read exactly the granted temp tables, and nothing else.
- A dropped or revoked table cannot be read, and a re-created one is not granted.
- No grant survives a restart, and nothing about grants is written to disk.
- `just fix`, `just check`, and `just web-test` pass; the new Playwright test has screenshots.

## Out of scope

- Granting DynamoDB or local tables, write access, or anything other than read.
- Grants in stdio `dqlrs --mcp` (a separate process cannot see the web session's temp tables).
- Expiry times, per-agent grants, or persistent grants.

## Working in this repo

- Read `AGENTS.md` first. The target is Rust (`rust-impl/`); do not add `dqlrs` behavior in Python.
- Web UI evidence comes from Playwright tests run with `just web-test`, screenshots saved with
  `playwrightArtifact`. Many tests need DynamoDB Local (`just dynamo` starts it; `just dd-fresh` restarts
  it clean).
- Before each commit: `just fix`, then `just check`.
- Branch from `origin/v-rust` (never commit to `v-rust` itself); prefer a stack of small pull requests,
  for example one for steps 1 to 3, one for 4 and 5, one for 6 and 7.
- Commit messages use conventional commits (`feat(engine): …`, `feat(mcp): …`, `test(web): …`).
- `CHANGELOG.md` is generated (`just changelog`); never edit it by hand.
- UI components and tooltip rules: `dqlrs-web/DESIGN.md`.
