# MCP grant panel on the header chip

**Priority:** Medium  
**Area:** `dqlrs-web` header, `dqlrs web --mcp`  
**Status:** Not started. Steps 1, 2, 3, and 5 of
[`mcp_grant_flow.md`](mcp_grant_flow.md). Classify is
[`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md). Telling other MCP
clients is [`mcp_grant_events.md`](mcp_grant_events.md).

## Flow

1. **Request.** The agent calls `request_access` with one read query. The server parses it, refuses anything that is not one `SELECT` or `SCAN`, and stores a pending request with an id (`r1`, `r2`, …). A browser event opens this panel.
2. **Inspect.** The panel shows that query. The user reads it before anything runs.
3. **Run.** The user runs it. The server stores the result as a new temp table and hands that table to the classifiers.
4. **Classify.** The panel shows the result rows and the attribute name matches and value matches from [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
5. **Grant.** The user grants or denies. Grant runs `GRANT MCP READ ON TEMPTABLE` on the table from the last Run. The agent's next `request_access` for that id returns the answer. [`mcp_grant_events.md`](mcp_grant_events.md) emits `grant_changed` from that `GRANT`.

## Why

An agent that needs data can submit a query, and today nothing in the header
asks the user about it. The person who can grant is in this web app. The
request pops up with the query. The user runs it, sees the rows and the
classifier matches, and only then sends that result back to the agent.

## Decisions

| Question | Decision |
| --- | --- |
| Transport | `dqlrs web --mcp` only. Grants live in the web process (see [`mcp_grant_flow.md`](mcp_grant_flow.md)). On stdio, `request_access` returns a tool error that says to use `dqlrs web --mcp` |
| What opens it | The panel pops open when a request arrives. A click on the MCP chip (`M1`, `M2`, …) opens the same panel. Hover still shows the short client tip |
| The tool | `request_access`. Arguments: `query` for a new request, or `request` (an id) to wait on one already made. The server does not run the query. The tool cannot grant. This is not `elicitation/create`: that prompt would show in the agent host |
| Check at request time | The query is parsed when the agent sends it. A parse error, more than one statement, or anything other than `SELECT` or `SCAN` is a tool error at once, and nothing pops up |
| Waiting | The call waits up to 30 seconds for the decision, then returns `{"request": "r1", "status": "pending"}`. The agent calls `request_access` with `request: "r1"` to keep waiting. 30 seconds stays under the 60-second default timeout of common clients. The wait does not hold the session lock |
| Answer | `{"request": "r1", "status": "granted", "table": "mcp_r1"}`, or `{"request": "r1", "status": "denied"}`. The answer stays readable until the server stops, so a retry after a timeout gets the same answer |
| Inspect | The query is shown as submitted. The user does not edit it here |
| Run | The server runs `CREATE TEMPTABLE mcp_<request id> AS ( … )` through the existing run path, so `/api/stop` cancels it. If that name is already a table, Run refuses and says so. Run again replaces that temp table and classifies it again |
| What Run shows | The result rows, in the same result table the editor uses, plus the classifier lines from the sensitivity plan. Both come back from the one Run |
| Grant | Enabled only after that Run succeeded and classify finished for that result. It runs `GRANT MCP READ ON TEMPTABLE mcp_<id>`, the same statement as the editor. It does not run the query again. The request is then done: Run and Grant go away for it |
| Deny | No `GRANT`. Drop the temp table this request created. The request is done |
| Agent gone | The server cannot tell that an agent left; it only sees that the agent stopped calling. The request stays until Grant, Deny, or the server stops. When the agent's chip has dropped off, the panel marks the request as from an agent that is no longer connected |
| Several agents | Each request has its own id, query, temp table, and Grant. Granting one does not grant the others. Requests are keyed by id, because agents without a session id share one chip today |
| Editor statements | A `GRANT` of this request's temp table typed in the editor completes the request as granted. A `DROP TEMPTABLE` of it before Grant clears the Run result, and Grant stays off until the user runs again. Revoke stays a statement |
| Already granted | The panel still lists grants the user made: name, `MCP read`, and when |

## What the panel shows

A pending request:

| Agent | Query | Result |
| --- | --- | --- |
| `M1`, `r1` | the query text | Run, then the rows, the attribute matches, the value matches, Grant, Deny |

Granted, from `SHOW MCP GRANTS`:

| Table | Access | Granted |
| --- | --- | --- |
| `mcp_r1` | MCP read | the `granted_at` time |

An empty half says so.

## Sync

Opening the panel loads `GET /api/mcp-access`: current grants and pending
requests, including the query and, once Run has finished, the classifier
lines. While it is open, the browser event stream (`/api/events`) updates it.
`grant-request` adds or removes a request and is what pops the panel open.
`grants` refreshes the granted list after Grant and after editor `GRANT`,
`REVOKE`, or `DROP`. The panel does not poll, and it does not read the MCP
stream.

Grant and a `GRANT` typed in the editor share one path, so the `mcp` marker
and the granted list change together.

## Auth

`GET /api/mcp-access`, Run, Grant, and Deny use the same bearer token as the
rest of `/api`. The page already sends it. No new token.

## Where it hooks

- `rust-impl/crates/dql-cli/src/mcp.rs`: the `request_access` tool, its
  schema, and the server instructions, which today say the agent cannot ask.
  The pending store is passed in from the web server; stdio has none.
- `rust-impl/crates/dql-cli/src/web/server.rs`: the pending store with a
  `Condvar` for the wait, `GET /api/mcp-access`, and Run, Grant, and Deny.
  `dispatch_mcp` must release `lock_state` before it waits.
- `rust-impl/crates/dql-cli/src/web/events.rs`: the `grant-request` event.
- `dqlrs-web/static/`: the panel on the presence chip. Follow
  `dqlrs-web/DESIGN.md`.

## Out of scope

- Granting a DynamoDB or local table. The granted object is the temp table Run created.
- Letting the agent run the query, name the temp table, or grant it.
- A query that is not one `SELECT` or `SCAN`.
- `elicitation/create` inside the agent host.
- The stdio transport.
- Classifier rules. Those are [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
- The MCP event stream. That is [`mcp_grant_events.md`](mcp_grant_events.md).

## Steps

1. `request_access`: parse and check the query, store the pending request, wait up to 30 seconds, return `pending` or the answer. A tool error on stdio. Unit tests for the check, the wait, and a retry that gets the stored answer.
2. Send `Mcp-Session-Id` on `initialize` so each agent has its own chip, and keep accepting clients that send none.
3. Pop the panel on `grant-request`. Show the query.
4. Run creates `mcp_<id>`, then classifies that table. Show the rows and both match lists. Grant stays off until that returns.
5. Grant runs `GRANT`. Deny drops the ungranted temp table. Either one completes the request.
6. A Playwright test walks the path: the request pops up, Run shows rows and both classifier lists, Grant grants that table and the agent's `request_access` returns it, Deny does not.
