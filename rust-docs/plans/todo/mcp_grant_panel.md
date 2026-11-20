# MCP grant panel on the header chip

**Priority:** Medium  
**Area:** `dqlrs-web` header, `dqlrs web --mcp`  
**Status:** Not started. Depends on the grant stream in
[`mcp_grant_events.md`](mcp_grant_events.md) only for telling other MCP clients.
This panel uses the browser event stream.

## Why

The header shows a colored `M1` chip while an agent is connected. Hovering it
lists clients. It does not say which temp tables that agent can read, or that
it is asking for one. The person who can grant is in this web app. Clicking
the chip should open that list and the yes/no for a new request.

## Decisions

| Question | Decision |
| --- | --- |
| What opens it | A click on an MCP presence chip (`M1`, `M2`, …). Hover still shows the short client tip. Other chips do not open this panel |
| Granted rows | Temp tables the user already granted: name, access (`MCP read`), and when. One access level exists today |
| Pending rows | Requests an agent has made and the user has not answered. Each row has Approve and Deny |
| How a request arrives | The agent calls a new tool, `request_grant`, with a temp table name. The server keeps that call open until Approve, Deny, or the MCP client disconnects. This is the grant-request flow. It is not `elicitation/create`: that prompt would show in the agent host, and the person who can grant is here |
| Approve | The server runs `GRANT MCP READ ON TEMPTABLE name`, the same statement as the editor. The tool call then returns success. If the temp table does not exist, `GRANT` fails as it does today, the error shows on the row, and the request stays pending |
| Deny | No `GRANT`. The tool call returns that the user denied it, and the row goes away |
| Who can request | Only an MCP client. The tool cannot grant. Approving does not run `CREATE TEMPTABLE` and does not take a query from the agent |
| Several agents | Each pending row names the chip (`M1`) that asked. Approving grants the table, not that one client |

## What the panel shows

Granted, from the engine's grant list (`SHOW MCP GRANTS` shape):

| Table | Access | Granted |
| --- | --- | --- |
| `late_orders` | MCP read | the `granted_at` time |

Pending:

| Agent | Table | Access | Asked | |
| --- | --- | --- | --- | --- |
| `M1` | `late_orders` | MCP read | the request time | Approve, Deny |

An empty half says so. The panel does not show rows.

## Sync

Opening the panel loads `GET /api/mcp-access`: current grants and pending
requests. While it is open, the browser event stream updates it. A `grants`
event refreshes the granted list (editor `GRANT` / `REVOKE` / `DROP`, or
Approve). A `grant-request` event adds, updates, or removes a pending row.
The panel does not poll.

Approve and a `GRANT` typed in the editor share one path, so the `mcp` marker
on the table and the granted list change together. A grant that lands for a
table with a pending request completes that request as approved. Revoking or
dropping a granted table takes it off the granted list and does not invent a
pending request.

`request_grant` returning is what the agent waits on. The standing MCP event
stream from `mcp_grant_events.md` still tells other clients that the grant
changed. This panel does not read that stream.

## Auth

`GET /api/mcp-access` and the approve and deny posts use the same bearer token
as the rest of `/api`. The page already sends it. No new token.

## Out of scope

- Granting a DynamoDB or local table.
- Letting the agent supply the `CREATE TEMPTABLE` query.
- `elicitation/create` inside the agent host.
- Editing a grant from the panel beyond Approve and Deny. Revoke stays a statement.

## Steps

1. Remember pending `request_grant` calls, keyed by agent chip and table.
2. `GET /api/mcp-access`, plus approve and deny. Approve runs `GRANT`.
3. Push `grant-request` on the browser event stream. Reuse `grants` for the granted list.
4. Click handler on the MCP chip and the panel. A Playwright test: chip opens, a granted table is listed, a `request_grant` appears, Approve grants it, Deny does not.
