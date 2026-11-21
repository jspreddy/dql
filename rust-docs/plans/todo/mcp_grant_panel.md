# MCP grant panel on the header chip

**Priority:** Medium  
**Area:** `dqlrs-web` header, `dqlrs web --mcp`  
**Status:** Not started. This file owns request, inspect, run, and the
grant decision. Classify is [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
Telling other MCP clients is [`mcp_grant_events.md`](mcp_grant_events.md).

## Flow

1. **Request.** The agent calls `request_access` with one read query and waits. A browser event opens this panel on the `M1` chip. The call stays open until Grant, Deny, or the client disconnects.
2. **Inspect.** The panel shows that query. The user reads it before anything runs.
3. **Run.** The user runs it. The server accepts only one `SELECT` or `SCAN`, stores the result as a new temp table, and hands that table to the classifiers.
4. **Classify.** The panel shows the result rows and the attribute name matches and value matches from [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
5. **Grant.** The user grants or denies. Grant runs `GRANT MCP READ ON TEMPTABLE` on the table from the last Run. The waiting call returns. [`mcp_grant_events.md`](mcp_grant_events.md) emits `grant_changed` from that `GRANT`.

## Why

An agent that needs data can submit a query, and today nothing in the header
asks the user about it. The person who can grant is in this web app. The
request pops up with the query. The user runs it, sees the rows and the
classifier matches, and only then sends that result back to the agent.

## Decisions

| Question | Decision |
| --- | --- |
| What opens it | The panel pops open when a request arrives. A click on the MCP chip (`M1`, `M2`, …) opens the same panel. Hover still shows the short client tip |
| The tool | `request_access`, with the query text. The server does not run it. The tool cannot grant. This is not `elicitation/create`: that prompt would show in the agent host |
| Inspect | The query is shown as submitted. The user does not edit it here |
| Run | One `SELECT` or `SCAN`. The server parses it and runs `CREATE TEMPTABLE mcp_<id> AS ( … )` through the existing run path. Anything else is refused, and Grant stays off. Run again replaces that temp table |
| What Run shows | The result rows, in the same result table the editor uses, plus the classifier lines from the sensitivity plan. Both come back from the one Run |
| Grant | Enabled only after that Run succeeded and classify finished for that result. It runs `GRANT MCP READ ON TEMPTABLE mcp_<id>`, the same statement as the editor. It does not run the query again. The tool returns the table name |
| Deny | No `GRANT`. Drop the temp table this request created. The tool returns that the user denied it |
| Disconnect | Same as Deny for a request that was not granted |
| Several agents | Each request has its own query, its own temp table, and its own Grant. Granting one does not grant the others |
| Already granted | The panel still lists grants the user made: name, `MCP read`, and when |

## What the panel shows

A pending request:

| Agent | Query | Result |
| --- | --- | --- |
| `M1` | the query text | Run, then the rows, the attribute matches, the value matches, Grant, Deny |

Granted, from `SHOW MCP GRANTS`:

| Table | Access | Granted |
| --- | --- | --- |
| `mcp_1` | MCP read | the `granted_at` time |

An empty half says so.

## Sync

Opening the panel loads `GET /api/mcp-access`: current grants and pending
requests, including the query and, once Run has finished, the classifier
lines. While it is open, the browser event stream updates it. `grant-request`
adds or removes a request and is what pops the panel open. `grants` refreshes
the granted list after Grant, editor `GRANT` / `REVOKE` / `DROP`. The panel
does not poll, and it does not read the MCP stream.

Grant and a `GRANT` typed in the editor share one path, so the `mcp` marker
and the granted list change together. A `GRANT` of this request's temp table
from the editor completes the waiting call as granted. Revoke stays a statement.

## Auth

`GET /api/mcp-access`, Run, Grant, and Deny use the same bearer token as the
rest of `/api`. The page already sends it. No new token.

## Out of scope

- Granting a DynamoDB or local table. The granted object is the temp table Run created.
- Letting the agent run the query, name the temp table, or grant it.
- A query that is not one `SELECT` or `SCAN`.
- `elicitation/create` inside the agent host.
- Classifier rules. Those are [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
- The MCP event stream. That is [`mcp_grant_events.md`](mcp_grant_events.md).

## Steps

1. Remember pending `request_access` calls: agent chip, query, and the temp table once Run has created one.
2. Pop the panel on `grant-request`. Show the query.
3. Run parses one read, creates `mcp_<id>`, then classifies that table. Show the rows and both match lists. Grant stays off until that returns.
4. Grant runs `GRANT`. Deny and disconnect drop the ungranted temp table. The tool returns either way.
5. A Playwright test walks the path: the request pops up, Run shows rows and both classifier lists, Grant grants that table, Deny does not.
