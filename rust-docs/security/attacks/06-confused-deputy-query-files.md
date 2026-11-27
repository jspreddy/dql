# 06. Confused deputy: agent-written `.dql` files that the person runs

**Severity:** High. **Needs:** only the MCP file tools, which an agent is allowed to use, and the
person pressing Run on a file the agent wrote or changed.

## What happens

Agents may create and update `.dql` files. The person runs those files in the web editor, the
REPL, or the notebook with their own rights. Nothing distinguishes statements the agent wrote
from statements the person wrote, and nothing asks before a sensitive statement runs. An agent
can put these in a file:

- **A grant:** `CREATE TEMPTABLE notes AS (SCAN * FROM users); GRANT MCP READ ON TEMPTABLE notes;`
  Then `read_granted_table` returns the rows. The web editor shows no confirmation for `GRANT`.
- **A write into a table already granted** (see 07). DQL has no `INSERT … SELECT` today, so this
  can only add literal values. It becomes a read path if a statement that copies rows is added.
- **A `SAVE`** to a file in the workspace, for an agent that can also read files directly. The
  MCP tools refuse the binary `.dql` save format, so this needs a shell.
- **A destructive statement** (`DROP TABLE`, `DELETE`) hidden among others. That is an
  integrity problem, not a read, but it uses the same path.

The statements can sit in a long file, after many blank lines, or in a file the person already
has open. The editor reloads a file when it changes on disk, so an agent's update can land
between the person reading the file and pressing Run.

## Reproduced

From the code: `GRANT MCP READ` is an ordinary statement (`dql-engine/src/engine.rs:773`) that any
script run executes. A search of `dqlrs-web/static/` finds no confirmation for `GRANT` or for
statements written by an agent. The grant itself was reproduced through `/api/run` (02).

## Code

- `dql-engine/src/engine.rs:773` `Statement::Grant`, `:1795` `grant_mcp_read`.
- `mcp.rs:385` `create_query_file`, `:403` `update_query_file`: agents write any `.dql` text.
- `web/server.rs:1246` `/api/run`: runs whatever the editor sends.

## Impact

The grant flow assumes the person decides what an agent sees. Here the agent decides, and the
person's Run button carries it out.

## Fix

- **Grant only through the grant panel.** Remove `GRANT MCP READ` from scripts, or make a
  script's `GRANT` stop and show the panel with the table's rows, the query, and Grant/Deny
  buttons.
- **Mark agent edits.** Record which lines an MCP tool wrote (or the last MCP write time per
  file). Show them in the editor. Ask before running a file an agent changed since the person
  last saved it, and list the statements that read or write data.
- Treat 07 as part of this fix: once a table is granted, nothing may change it.

