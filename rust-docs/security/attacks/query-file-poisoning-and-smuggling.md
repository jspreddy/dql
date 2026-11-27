# MCP Query-File Poisoning, Statement Smuggling, and Human-Run Self-Grants

## Summary

Even if an agent is **strictly confined to `/mcp` tools**—with no shell access, no raw filesystem access, and no access to `/api/*`—the agent is explicitly permitted to create and edit `.dql` files (`create_query_file`, `update_query_file`). Because `dqlrs` treats `GRANT MCP READ ON TEMPTABLE <name>` as an ordinary DQL statement inside `.dql` scripts, and because the web UI's batch runner and statement splitter have blind spots, an agent can plant or smuggle self-granting statements into `.dql` files that execute silently when a human runs a file or folder.

## Violated boundary

- **Intended:** Agents can CRUD `.dql` files and request data access, while only deliberate human approval grants MCP read access to data.
- **Actual:** Editing `.dql` files gives the agent an indirect code-execution and privilege-escalation channel inside the human's `Session`, capable of issuing `GRANT MCP READ` (or mutating/exporting data) without going through `request_access` or the MCP review panel.

## Code paths

1. **Agent `.dql` write tools:** `rust-impl/crates/dql-cli/src/mcp.rs` (`call_tool`) exposes `create_query_file` and `update_query_file`, writing arbitrary UTF-8 DQL into any workspace `.dql` file (`web/files.rs`).
2. **`GRANT MCP READ` accepted in any script run:**
   - `rust-impl/crates/dql-parser/src/lib.rs` parses `CREATE TEMPTABLE <name> AS (<query>)` and `GRANT MCP READ ON TEMPTABLE <name>` as regular DQL statements.
   - `rust-impl/crates/dql-cli/src/web/server.rs` (`handle_run` -> `run_script` -> `execute_watched`) executes every statement in a human-triggered run against the shared `state.session`, which is the exact same `Session` read by `/mcp` (`list_granted_tables`, `read_granted_table`).
   - As soon as `execute_watched` observes a new grant, `publish_grant_changes` broadcasts `notifications/dqlrs/grants_changed` over `/mcp` (`mcp_stream.rs`), notifying the waiting agent immediately.
3. **Blind batch execution in `Run folder`:** In `dqlrs-web/static/app.js` (`openRunDialog`, `startRunDialog`), running a folder from the tree menu displays **only** file paths and statement counts (`N files · M queries`). It never displays the SQL/DQL text of the files in the folder and emits no warning when a file contains `GRANT MCP READ`, `SAVE`, `INSERT`, `UPDATE`, `DELETE`, or `DROP`.
4. **Statement-splitter vs. parser desynchronization (`split_statements` escaped-backslash bug):**
   - `rust-impl/crates/dql-cli/src/web/shape.rs` (`split_statements`) tracks string quotes with:
     ```rust
     if ch == open && chars.get(i.wrapping_sub(1)).copied() != Some('\\') {
         quote = None;
     }
     ```
     When a string literal ends with an escaped backslash (`'\\'`), `chars[i - 1]` is `'\\'`, so `split_statements` mistakenly believes the closing quote is escaped and stays inside `quote = Some('\'')` across subsequent semicolons!
   - By contrast, `rust-impl/crates/dql-parser/src/lib.rs` (`read_quoted`) consumes `\\` as a single escaped backslash character and closes the string literal at the immediately following `'`.
   - When `split_statements` hands a single combined chunk containing multiple semicolon-separated statements to `execute_for_serve` -> `FragmentEngine::execute` -> `Engine::execute` (`rust-impl/crates/dql-engine/src/engine.rs`), `Engine::execute` loops over **all** parsed statements (`for statement in &statements { last = self.run(statement)?; }`) and returns **only `last`**.

## Circumvention mechanics

### 1. Statement smuggling behind a benign `SELECT`

Because `split_statements` and `dql-parser` disagree on `'\\'`, a single line in a `.dql` file that ends with a normal `SELECT` (closing the desynchronized quote state with `'\''`) is counted and dispatched by `handle_run` as **one** statement, while `Engine::execute` runs multiple statements back-to-back:

1. Statement 1 creates a temp table from any sensitive table (`CREATE TEMPTABLE ... AS (SCAN * FROM ... WHERE attr != '\\')`).
2. Statement 2 grants MCP read on that temp table (`GRANT MCP READ ON TEMPTABLE ...`).
3. Statement 3 runs an expected `SELECT ... WHERE attr != '\''` whose rows are returned as `last`.

In `run_script`:
- Only **one** result card is emitted to the browser—showing the benign `SELECT` rows—while the earlier `Created temp table` and `Granted MCP read` messages are discarded by `Engine::execute`.
- `changed_tables(statement)` calls `dql_parser::parse_statement` on the multi-statement chunk, which returns `Err("expected exactly one statement")`, so `changed_tables` returns `[]` and suppresses the table-list SSE refresh event (`app.hub.tables`).
- Meanwhile, `execute_watched` compares `granted_tables()` before and after execution, sends `notifications/dqlrs/grants_changed` to the MCP stream, and the agent immediately reads every row via `read_granted_table`.

### 2. Folder-run planting and TOCTOU file updates

Even without quote desynchronization:
- An agent asked to scaffold or tidy `.dql` files in a folder can include `CREATE TEMPTABLE ...; GRANT MCP READ ON TEMPTABLE ...;` in one of the files (or deep below the fold of a multi-query file). When the human clicks **Run folder**, every file executes in order with no statement preview or grant confirmation dialog.
- For any `.dql` file not currently open in the editor, `runTargetDocuments` (`dqlrs-web/static/app.js`) reads `/api/file?path=...` at dialog-open time; an agent calling `update_query_file` can swap the file contents on disk between the time the human inspected the file and the time the human runs the folder—and then call `update_query_file` again after reading `read_granted_table` to restore the original `.dql` text.

## Remediation

- **Remove `GRANT MCP READ` from `.dql` script execution:**
  - Reject `Statement::GrantMcpRead` inside `POST /api/run`, CLI `-c`, and batch runs (or remove `GRANT MCP READ` from public DQL script execution entirely) so MCP grants can **only** be created by an explicit human click on the dedicated `/api/mcp-access/grant` UI control.
- **Unify statement splitting with `dql-parser`:**
  - Replace the hand-rolled quote scanner in `web/shape.rs` (`split_statements`) with `dql-parser` tokenization/span splitting, and make `run_script` execute **at most one** parsed `Statement` per step (`parse_statement`) rather than calling `parse_script` inside `Engine::execute`.
- **Surface side effects in `Run file` and `Run folder`:**
  - Inspect parsed ASTs before running a file or folder and require explicit confirmation whenever any statement mutates data (`INSERT`, `UPDATE`, `DELETE`, `CREATE`, `DROP`, `ALTER`, `LOAD`), writes files (`SAVE`), or creates virtual tables.
  - Pin each file's `revision` hash when the user opens the confirmation dialog and pass those revisions to `/api/run` so concurrent `update_query_file` writes abort the run instead of executing unreviewed edits.
