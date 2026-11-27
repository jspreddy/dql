# 02. The MCP token is the API token: an agent can run queries and grant itself access

**Severity:** Critical. **Needs:** only what the person gives the agent to use MCP: the
`.cursor/mcp.json` or `.mcp.json` snippet, or the config copied from the MCP pill.

## What happens

To connect an agent to HTTP `/mcp`, `dqlrs web` prints (and the MCP pill copies) a config with
`"Authorization": "Bearer <token>"`. That is the same token the browser uses for `/api/*`. The
server has one token and one check (`authorized`) for both. So the agent's MCP credential is a
full human credential. With it, an agent can:

1. **Run any query:** `POST /api/run` with `{"dql": "SCAN * FROM users"}`.
2. **Grant itself access without a request:**
   `CREATE TEMPTABLE loot AS (SCAN * FROM users); GRANT MCP READ ON TEMPTABLE loot;` through
   `/api/run`, then `read_granted_table` through `/mcp`.
3. **Approve its own request:** call `request_access` on `/mcp`, then
   `POST /api/mcp-access/run` and `POST /api/mcp-access/grant` with `{"request":"r1"}`. The
   waiting `request_access` call returns `granted`, and `read_granted_table` returns the rows.
   The person sees a request appear and disappear.
4. Turn `/mcp` on (`POST /api/mcp`), read workspace files that MCP refuses (`GET /api/file`
   serves any viewable file, not only `.dql`), and read cached results (`/api/query-results`).

## Reproduced

On `dqlrs web --mcp` with the memory backend, using only the token from the MCP snippet:

- `request_access` with `SCAN * FROM users`, then `/api/mcp-access/run` and
  `/api/mcp-access/grant` for `r1`: the request answered `granted`, and `read_granted_table`
  on `mcp_r1` returned the planted value.
- `/api/run` with `CREATE TEMPTABLE loot …; GRANT MCP READ ON TEMPTABLE loot;` answered
  `Granted MCP read on temp table loot (1 row)`.

## Code

- `mcp.rs:437` `http_client_snippets`: the MCP config carries the API token.
- `web/server.rs:732-745`: `/mcp` and `/api/*` both pass `authorized(app, request)`, one token.
- `web/server.rs:1237-1245`, `:1436` `run_access`, `:1500` `grant_access`: the approval routes
  accept that same token, so the requester can approve.
- `dql-engine/src/engine.rs:773`: `GRANT MCP READ` is an ordinary statement, so anything that can
  reach `/api/run` can grant.

## Impact

The grant flow protects nothing from the agent it was built for. No shell, no key store, and no
second process is needed: the agent uses the credential the person handed it.

## Fix

- Two credentials. An **MCP token**, minted for agents, accepted only on `/mcp`, and rejected
  by every `/api/*` route. A **browser session** secret that is never printed or copied into a
  config (see 01).
- Grant only from a human action. Accept `/api/mcp-access/run`, `/grant`, and `/deny` only
  with the browser session, never with the MCP token. Better, tie them to a click that
  carries a fresh, single-use nonce the server rendered into the grant panel.
- Do not let `GRANT MCP READ` run from `/api/run` scripts; see 06.
- Give each MCP token a scope and a lifetime the person can see and revoke in the MCP menu.
