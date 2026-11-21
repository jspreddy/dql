# Push MCP grant and revoke events

**Priority:** Medium  
**Area:** `dqlrs web --mcp`, Streamable HTTP  
**Status:** Not started. Step 5 of [`mcp_grant_flow.md`](mcp_grant_flow.md),
after the user decides in [`mcp_grant_panel.md`](mcp_grant_panel.md).

## Flow

One path. The other two files own the earlier steps.

1. **Request.** The agent calls `request_access` with one read query. The panel pops it up. [`mcp_grant_panel.md`](mcp_grant_panel.md)
2. **Inspect.** The user reads that query in the panel.
3. **Run.** The user runs it. The result is stored as a temp table.
4. **Classify.** Attribute name and value classifiers run on that temp table. The panel shows the rows and both match lists. [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md)
5. **Grant.** The user grants or denies. Grant is `GRANT MCP READ ON TEMPTABLE` on that temp table. The agent's `request_access` returns the answer. This file emits `grant_changed` so every connected MCP client hears the decision without polling.

## Why

After the user grants, an agent learns about it only by calling
`request_access` or `list_granted_tables` again. The same gap exists when the
user revokes, drops, or replaces a granted temp table from the editor. The
server should say so on a stream the client already holds.

MCP's Streamable HTTP transport (`2025-06-18`, `transports.mdx`) lets a client
open a standing stream with `GET` and `Accept: text/event-stream`. The server
sends JSON-RPC notifications on that stream. The older HTTP+SSE transport
(`2024-11-05`) used the SSE stream as the standing server-to-client channel.
This plan uses the Streamable HTTP `GET` stream.

## Decisions

| Question | Decision |
| --- | --- |
| Endpoint | The existing `/mcp` endpoint. `GET` with `Accept: text/event-stream` holds the stream. `POST` stays the way a client sends a message |
| Who may connect | Same rules as `POST /mcp`: `Authorization: Bearer` with the server token, and a non-loopback `Origin` is rejected |
| What is pushed | One JSON-RPC notification per grant change. No row data, no query text, no classifier matches |
| How changes are found | Compare the live grants (`granted()`) before and after each statement that succeeds, on every run path: the editor, Run, Grant, and Deny. A name that appears is `granted`. A name that disappears is `revoked`. This covers `GRANT`, `REVOKE` (one table or `ALL`), `DROP TEMPTABLE`, and a `CREATE TEMPTABLE` that replaces a granted table, which ends the grant because the generation changed |
| Nothing changed | Run's `CREATE TEMPTABLE` of a new name changes no grant, so it sends nothing. A second `GRANT` of the same table keeps its grant, so it sends nothing |
| Denied | Deny changes no grant, but the agent that asked waits for it. Send `denied` with the request id and no table |
| On connect | Send the current grants as `granted`, then live changes. A client that connects after the user granted does not have to poll first. Past denials are not replayed; `request_access` still returns them |
| The waiting agent | `request_access` returns the table name or the denial. That is the answer for the agent who asked. This stream is the same fact for every client, including one that reconnects |
| No stream | A client that never sends `GET`, or that receives 405, calls `request_access` again with its request id, or `list_granted_tables` |
| Stdio | Out of scope. A stdio server has its own engine and no grants from the browser (see [`mcp_grant_flow.md`](mcp_grant_flow.md)) |
| Other `GET` | A `GET` without `Accept: text/event-stream` stays 405, as today |
| Browser stream | The same comparison sends the browser's `grants` event, which today follows only `GRANT` and `REVOKE`. A drop or replace of a granted table then refreshes the panel and the `mcp` marker too |

## Event

SSE `event: message`, data is one JSON-RPC notification:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/dqlrs/grant_changed",
  "params": {
    "table": "mcp_r1",
    "change": "granted",
    "request": "r1",
    "at": "2026-11-20T23:46:00Z"
  }
}
```

`change` is `granted`, `revoked`, or `denied`. `table` is the temp table and is
absent for `denied`. `request` is the `request_access` id when the change came
from the panel, and absent for a table the user granted from the editor. `at`
is UTC, RFC 3339, the time of this change (for a replayed grant, the original
`granted_at`).

After `granted`, the agent calls `read_granted_table`. The event only says the
grant changed.

## Client

An agent that can hold a stream opens it once, before it requests access:

```http
GET /mcp HTTP/1.1
Accept: text/event-stream
Authorization: Bearer <token>
```

That `GET` is the subscription. There is no new tool and no `resources/subscribe`.
The stream stays open. Each later grant, denial, revoke, drop, or replace of a
granted temp table is one event. If the stream drops, the client opens it again
and receives the current grants.

A client that does not open the stream, or whose `GET` returns 405, keeps
calling `request_access` with its request id, as the panel plan describes.

## Auth

- Missing or wrong bearer token: HTTP 401, the same JSON-RPC unauthorized error
  as `POST /mcp`. No event stream.
- `Origin` present and not loopback: HTTP 403, same as `POST /mcp`.
- `/mcp` is still off unless the process was started with `--mcp`.

## Where it hooks

- `rust-impl/crates/dql-cli/src/web/server.rs` (`dispatch_mcp`): accept the
  standing `GET`, after the auth and origin checks that already run, and keep
  one subscriber per stream. Each stream holds its own connection thread, as
  `/api/events` does.
- `rust-impl/crates/dql-cli/src/web/events.rs`: the subscriber list next to the
  browser's. A subscriber whose write fails is dropped.
- The run loop in `server.rs` that calls `changes_mcp_grants` today: replace
  that check with the before and after comparison, and publish to both
  streams from there. The engine crate does not know about streams, so nothing
  is published from `virtual_tables.rs`.
- Deny publishes `denied` from the panel's Deny path.

## Out of scope

- The request, the query, Run, and the classifiers. Those are the panel and
  [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
- Pushing rows, or any event other than grant changes and denials.
- `sampling`, `elicitation`, `notifications/tools/list_changed`, and
  `resources/subscribe`.
- A stream on stdio.
- Resuming a dropped stream with `Last-Event-ID`. Reconnect replays the
  current grants instead.

## Steps

1. Compare grants before and after each statement. Publish the browser's
   `grants` from that comparison. Tests: drop and replace of a granted table
   now send `grants`.
2. Hold MCP SSE subscribers and publish `notifications/dqlrs/grant_changed`.
3. `GET /mcp` with `Accept: text/event-stream`, bearer check, and origin check.
   Replay current grants, then live events. Any other `GET` stays 405.
4. Send `denied` from Deny.
5. Tests: auth and origin failures, replay, grant, revoke, revoke all, drop,
   replace, deny, a Run that emits nothing, and a client that gets 405 and
   still gets its answer from `request_access`.
