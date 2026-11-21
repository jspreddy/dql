# Push MCP grant and revoke events

**Priority:** Medium  
**Area:** `dqlrs web --mcp`, Streamable HTTP  
**Status:** Not started. This is step 5 of the grant flow, after the user
decides in [`mcp_grant_panel.md`](mcp_grant_panel.md).

## Flow

One path. The other two files own the earlier steps.

1. **Request.** The agent submits one read query and waits. The panel pops it up. [`mcp_grant_panel.md`](mcp_grant_panel.md)
2. **Inspect.** The user reads that query in the panel.
3. **Run.** The user runs it. The result is stored as a temp table.
4. **Classify.** Attribute name and value classifiers run on that temp table. The panel shows the rows and both match lists. [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md)
5. **Grant.** The user grants or denies. Grant is `GRANT MCP READ` on that temp table. The waiting call returns. This file emits `grant_changed` so every connected MCP client hears the decision without polling.

## Why

After the user grants, the agent still only learns about it by calling
`list_granted_tables` again. The same gap exists when the user revokes or
drops a granted temp table from the editor. The server should say so on the
stream the client already holds.

MCP's Streamable HTTP transport (`2025-06-18`, `transports.mdx`) lets a client
open a standing stream with `GET` and `Accept: text/event-stream`. The server
sends JSON-RPC notifications on that stream. The older HTTP+SSE transport
(`2024-11-05`) used the SSE stream as the standing server-to-client channel.
This plan uses the Streamable HTTP `GET` stream. Stdio has no such request.

## Decisions

| Question | Decision |
| --- | --- |
| Endpoint | The existing `/mcp` endpoint. `GET` with `Accept: text/event-stream` holds the stream. `POST` stays the way a client sends a message |
| Who may connect | Same rules as `POST /mcp`: `Authorization: Bearer` with the server token, and a non-loopback `Origin` is rejected |
| What is pushed | One JSON-RPC notification per grant change. No row data, no query text, no classifier matches |
| When | The panel's Grant runs `GRANT MCP READ ON TEMPTABLE` and this stream sends `granted`. `REVOKE` (one table or `ALL`) sends `revoked`, one event per table. `DROP TEMPTABLE` sends `revoked` only if that table was granted. Run's `CREATE TEMPTABLE` does not grant, so it sends nothing. Deny sends nothing |
| Already granted | A second `GRANT` of the same table does not send another event |
| On connect | Send the current grants as `granted`, then live changes. A client that connects after the user granted does not have to poll first |
| The waiting agent | `request_access` returns the table name or the denial. That return is the answer for the agent who asked. This stream is the same fact for every client, including one that reconnects |
| No stream | A client that never sends `GET`, or that receives 405, keeps calling `list_granted_tables`. Stdio clients do that, because stdio has no `GET` |
| Other `GET` | A `GET` without `Accept: text/event-stream` stays 405, as today |

## Event

SSE `event: message`, data is one JSON-RPC notification:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/dqlrs/grant_changed",
  "params": {
    "table": "mcp_1",
    "change": "granted",
    "at": "2026-11-20T23:46:00Z"
  }
}
```

`change` is `granted` or `revoked`. `at` is UTC, RFC 3339, the time of this
change (for a replayed grant, the original grant time). `table` is the temp
table Run created and Grant just allowed.

The agent then calls `read_granted_table`. The event only says the grant changed.

## Client

An agent that can hold a stream opens it once, before it requests access:

```http
GET /mcp HTTP/1.1
Accept: text/event-stream
Authorization: Bearer <token>
```

That `GET` is the subscription. There is no new tool and no `resources/subscribe`.
The stream stays open. Each later grant, revoke, or drop of a granted temp
table is one event. If the stream drops, the client opens it again and receives
the current grants.

A client that does not open the stream, or whose `GET` returns 405, polls
`list_granted_tables` as it does now. Its `request_access` call still waits
and still returns.

## Auth

- Missing or wrong bearer token: HTTP 401, the same JSON-RPC unauthorized error
  as `POST /mcp`. No event stream.
- `Origin` present and not loopback: HTTP 403, same as `POST /mcp`.
- `/mcp` is still off unless the process was started with `--mcp`.

## Where it hooks

- `rust-impl/crates/dql-cli/src/web/server.rs` (`dispatch_mcp`): accept the
  standing `GET` and keep one subscriber per stream.
- Grant and revoke already run in `virtual_tables.rs` (`grant`, `revoke`,
  `revoke_all`) from `GRANT`, `REVOKE`, and `DROP TEMPTABLE`. Publish from
  there, after the catalog change, and only when the process serves MCP.
  The panel's Grant and Deny go through those statements, so this file does
  not add a second publisher.

## Out of scope

- The request, the query, Run, and the classifiers. Those are the panel and
  [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md).
- Pushing rows, or any event other than grant and revoke.
- `sampling`, `elicitation`, `notifications/tools/list_changed`, and
  `resources/subscribe`.
- A stream on stdio. Stdio clients keep polling.
- Changing the browser's `/api/events` stream. The popup uses that stream.

## Steps

1. Hold MCP SSE subscribers and publish `notifications/dqlrs/grant_changed`.
2. `GET /mcp` with `Accept: text/event-stream`, bearer check, and origin check.
   Replay current grants, then live events. Any other `GET` stays 405.
3. Emit on `GRANT`, `REVOKE`, `REVOKE ALL`, and `DROP` of a granted temp table.
   A temp table created by Run emits nothing until Grant.
4. Tests: auth and origin failures, replay, grant, revoke, drop, a Run that
   emits nothing, and a client that gets 405 and still reads
   `list_granted_tables`.
