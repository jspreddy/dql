# Let the user grant an agent's query

**Priority:** Medium  
**Area:** `dqlrs web --mcp`  
**Status:** Not started. This is the workflow. The three plans below own the steps.

## Goal

An agent that needs data asks by writing a query and requesting access. The
person at the web app sees that request, inspects the query, runs it, and sees
the resulting data together with the classifier responses. Only then do they
decide whether that data goes back to the agent.

The agent does not receive the data with the request. The user sees the query
before it runs, and sees the rows and the classifier responses before deciding.
A later change of mind is visible to connected agents.

## Workflow

1. **Request.** The agent writes a query and calls `request_access`. The server checks that it is one read and holds it as a pending request. [`mcp_grant_panel.md`](mcp_grant_panel.md)
2. **Inspect.** The request pops up. The user reads the query. [`mcp_grant_panel.md`](mcp_grant_panel.md)
3. **Run.** The user runs the query. The result becomes a new temp table, and the user sees its rows. [`mcp_grant_panel.md`](mcp_grant_panel.md)
4. **Classify.** With that data, the user sees which attribute names matched and which values matched. [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md)
5. **Decide.** The user grants or denies. Grant runs `GRANT MCP READ ON TEMPTABLE` on that temp table. The agent learns the answer from `request_access`, and connected agents hear the grant, and a later revoke, on the `/mcp` stream. [`mcp_grant_panel.md`](mcp_grant_panel.md), [`mcp_grant_events.md`](mcp_grant_events.md)

## Plans

| Plan | What it owns |
| --- | --- |
| [`mcp_grant_panel.md`](mcp_grant_panel.md) | The `request_access` tool, the pending request, the panel, Run, Grant, and Deny |
| [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md) | The classifier responses shown with that data before the decision |
| [`mcp_grant_events.md`](mcp_grant_events.md) | After the decision, connected agents hear the grant, the denial, and a later revoke |

Build in that order. The panel works without the other two: Grant stays off
until classify exists, so ship sensitivity with it or right after. Events are
an addition: without them, agents still get the answer from `request_access`.

## What the code does today

These facts shape all three plans. Each plan says where it depends on one.

| Fact | Where | What it means for this flow |
| --- | --- | --- |
| Grants live in the `dqlrs web` process only | `virtual_tables.rs` `grants`, per engine | `dqlrs --mcp` on stdio has its own engine and never sees a grant made in the browser. The flow is HTTP only (`dqlrs web --mcp`) |
| A grant is tied to the temp table's generation | `grant_live` in `virtual_tables.rs` | `CREATE TEMPTABLE` of a granted name, or `DROP TEMPTABLE`, ends the grant without any `REVOKE` |
| The browser hears `grants` only after `GRANT` or `REVOKE` | `changes_mcp_grants` in `web/server.rs` | A drop or replace of a granted table does not refresh the browser today. The events plan fixes both streams the same way |
| `/mcp` answers `POST` only, with one JSON reply | `dispatch_mcp` in `web/server.rs` | No standing stream yet. A tool call cannot stream progress |
| The server never sends `Mcp-Session-Id` | `dispatch_mcp`, `presence.rs` | Clients that send none all share one presence id, so they all show as `M1`. Requests are keyed by request id, not by chip |
| One thread per connection, one lock for the session | `accept`, `lock_state` in `web/server.rs` | A tool call may wait, but never while holding the session lock |
| MCP clients time out a tool call | The TypeScript SDK's default request timeout is 60 seconds | `request_access` cannot wait for the user without a bound. It waits a short time and then returns `pending` |
