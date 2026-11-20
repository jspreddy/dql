# Push MCP grant and revoke events

**Priority:** Medium  
**Area:** `dqlrs web --mcp`, Streamable HTTP  
**Status:** Not started.

## Why

An agent learns that a temp table was granted or revoked only by calling
`list_granted_tables` again. The user already ran `GRANT` or `REVOKE` in the
editor. The server should tell connected MCP clients when that happens.

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
| What is pushed | One JSON-RPC notification per grant change. No row data |
| When | `GRANT MCP READ ON TEMPTABLE` sends `granted`. `REVOKE` (one table or `ALL`) sends `revoked`, one event per table. `DROP TEMPTABLE` sends `revoked` only if that table was granted. `CREATE TEMPTABLE` does not grant, so it sends nothing |
| Already granted | A second `GRANT` of the same table does not send another event |
| On connect | Send the current grants as `granted`, then live changes. A client that connects after the user granted does not have to poll first |
| No stream | A client that never sends `GET`, or that receives 405, keeps calling `list_granted_tables`. Stdio clients do that, because stdio has no `GET` |
| Other `GET` | A `GET` without `Accept: text/event-stream` stays 405, as today |

## Event

SSE `event: message`, data is one JSON-RPC notification:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/dqlrs/grant_changed",
  "params": {
    "table": "late_orders",
    "change": "granted",
    "at": "2026-11-20T23:46:00Z"
  }
}
```

`change` is `granted` or `revoked`. `at` is UTC, RFC 3339, the time of this
change (for a replayed grant, the original grant time). `table` is the temp
table name.

The agent still calls `read_granted_table` to read rows. The event only says
the grant changed.

## Client

An agent that can hold a stream opens it once:

```http
GET /mcp HTTP/1.1
Accept: text/event-stream
Authorization: Bearer <token>
```

That `GET` is the subscription. There is no new tool and no `resources/subscribe`.
The stream stays open. Each later `GRANT`, `REVOKE`, or dropping a granted temp
table is one event. If the stream drops, the client opens it again and receives
the current grants.

A client that does not open the stream, or whose `GET` returns 405, polls
`list_granted_tables` as it does now.

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

## Out of scope

- Pushing rows, or any event other than grant and revoke.
- `sampling`, `elicitation`, `notifications/tools/list_changed`, and
  `resources/subscribe`.
- A stream on stdio. Stdio clients keep polling.
- Changing the browser's `/api/events` stream.

## Steps

1. Hold MCP SSE subscribers and publish `notifications/dqlrs/grant_changed`.
2. `GET /mcp` with `Accept: text/event-stream`, bearer check, and origin check.
   Replay current grants, then live events. Any other `GET` stays 405.
3. Emit on `GRANT`, `REVOKE`, `REVOKE ALL`, and `DROP` of a granted temp table.
4. Tests: auth and origin failures, replay, grant, revoke, drop, and a client
   that gets 405 and still reads `list_granted_tables`.
