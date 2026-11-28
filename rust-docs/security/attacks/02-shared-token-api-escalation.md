# 02. The MCP token is the API token: run queries, self-grant, turn MCP back on

**Severity:** Critical. **Found by:** both; extra routes as marked. **Needs:** only the MCP
config the person gave the agent, or a token from 01 or 03. **Under `aws-vault exec`:**
unchanged. The token drives the running, credentialed server.

## What happens

`dqlrs` has one per-user token (`api_token::load`). It guards `/mcp`, every `/api/*` route, and
TCP `dqlrs --serve --bind`. The MCP config printed by `dqlrs web` and copied by the MCP pill
carries it as `"Authorization": "Bearer <token>"`. So an agent's MCP credential is a full human
credential:

1. **Run any statement** (both): `POST /api/run` with `{"dql": "SCAN * FROM users"}`.
2. **Browse any table** (Cursor): `GET`/`POST /api/rows` pages any DynamoDB, local, or ungranted
   temp table.
3. **Approve its own request** (both): `request_access` on `/mcp`, then
   `POST /api/mcp-access/run` and `/grant` with `{"request":"r1"}`, then `read_granted_table`.
4. **Grant without a request** (Claude): `CREATE TEMPTABLE loot AS (SCAN * FROM users);
   GRANT MCP READ ON TEMPTABLE loot;` through `/api/run`.
5. **Undo the kill switch** (Cursor): `/api/*` is served whether or not MCP is on, so
   `POST /api/mcp` with `{"enabled": true}` turns `/mcp` back on after the person turned it off.
6. **Read cached results** (both): `GET /api/query-results?path=<file>.dql` and
   `GET /api/structure` return decrypted history (see 05).
7. **Read files MCP refuses** (both): `GET /api/file` and `/api/file-rows` serve any viewable
   file, not only `.dql`, including dot-folders such as `.cursor/mcp.json` (Cursor:
   `read_opened` uses `resolve_inside` without rejecting dot segments).
8. **Drive TCP workers** (Cursor): `{"op":"exec","token":"<token>","dql":"…"}` to any
   `dqlrs --serve --bind` listener.

## Reconciled

Cursor's review says `dqlrs` writes the token into `.cursor/mcp.json` and `.mcp.json`. No code
writes those files; `dqlrs` prints the snippet and the pill copies it. The token reaches the
workspace when the person pastes it there, which the setup instructions invite. The risk is the
same; the wording is corrected.

## Reproduced

Claude, on `dqlrs web --mcp` with the memory backend, using only the token from the snippet:
the self-approved request returned `granted` and `read_granted_table` returned the planted value;
the `/api/run` grant answered `Granted MCP read on temp table loot (1 row)`.

## Code

- `mcp.rs:437` `http_client_snippets`: the config carries the API token. `web/server.rs`
  `mcp_info` returns it from `GET /api/mcp`.
- `web/server.rs:732-745`: `/mcp` and `/api/*` both pass `authorized(app, request)`.
- `web/server.rs:1237-1245`, `:1436` `run_access`, `:1500` `grant_access`: the approval routes
  take the same token.
- `serve/tcp.rs`: TCP `--serve` loads the same `api_token::load()`.
- `dql-engine/src/engine.rs:773`: `GRANT MCP READ` is an ordinary statement.

## Fix

- Split credentials by role (both): a browser session secret that is never printed or copied
  (01); MCP tokens accepted only on `/mcp`, only while MCP is on, and rejected by `/api/*` and
  TCP `--serve`; a per-listener secret for `--serve --bind`.
- Accept `/api/mcp-access/run`, `/grant`, and `/deny` only with the browser session, tied to a
  click that carries a single-use nonce (Claude).
- Keep `GRANT MCP READ` out of scripts (06).
- Reject dot-folder segments in `read_opened`, as `clean_relative` does (Cursor).
- Give each MCP token a scope and lifetime the person can see and revoke (both).
