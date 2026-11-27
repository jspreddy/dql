# 06. Query-file poisoning: agent-written `.dql` files the person runs

**Severity:** High. **Found by:** both; Run folder and the revision race by Cursor. **Needs:** only
the MCP file tools, which agents are allowed, and the person pressing Run. **Under `aws-vault
exec`:** unchanged; the person's `dqlrs` runs the file with the person's credentials.

## What happens

Agents may create and update `.dql` files (`create_query_file`, `update_query_file`). The person
runs them in the web editor, the REPL, or the notebook against the same `Session` that `/mcp`
reads. Nothing marks agent-written statements and nothing asks before a sensitive one runs.

- **A grant** (both): `CREATE TEMPTABLE notes AS (SCAN * FROM users); GRANT MCP READ ON
  TEMPTABLE notes;`. When it runs, `execute_watched` sees the new grant and
  `publish_grant_changes` sends `notifications/dqlrs/grants_changed` to the waiting agent
  (Cursor), which reads the rows with `read_granted_table`.
- **Run folder shows no statements** (Cursor): the folder dialog lists file paths and counts
  (`N files · M queries`), never the text, and warns about nothing.
- **Hidden placement** (both): far down a long file, or in a file the person already has open.
  A clean editor reloads a file when it changes on disk (`file-watch.spec.js`).
- **Swap between review and run** (Cursor): for files not open in the editor, the folder run reads
  `/api/file` when the dialog opens. An agent can update the file after the person looked, then
  restore it after reading the grant.
- **A `SAVE`** (Claude): rows to a workspace file, for an agent that can read files directly. MCP's
  file tools refuse the binary `.dql` save format, so this needs a shell.
- **A write into a granted table** (Claude): see 08.
- **Destructive statements** (both): `DROP`, `DELETE`, `UPDATE` ride the same path.

07 is a sharper version: the grant hides behind a statement the person can see.

## Code

- `mcp.rs:385` `create_query_file`, `:403` `update_query_file`.
- `dql-engine/src/engine.rs:773` `Statement::Grant`, `:1795` `grant_mcp_read`.
- `web/server.rs:1246` `/api/run` → `handle_run` → `run_script` → `execute_watched`.
- `dqlrs-web/static/app.js` `openRunDialog`, `startRunDialog`, `runTargetDocuments`.

## Fix

- Grants only from the grant panel (both): reject `GRANT MCP READ` in `/api/run`, `-c`, and
  folder runs, or stop the run and open the panel with the rows and Grant/Deny.
- Show side effects before a run (Cursor): parse the files and ask whenever a statement writes
  data, writes files (`SAVE`), creates virtual tables, or grants.
- Pin revisions (Cursor): send each file's `revision` from the dialog to `/api/run`, and abort if
  the file changed.
- Mark agent edits (Claude): record MCP writes per file, show them, and ask before running a file
  an agent changed since the person last saved it.
