# An agent stream can stay open after MCP is turned off

Status: fixed on `fix/release-review-followups`. Found on `v-rust` at `90c492c` while reviewing #308 (`b97b3b8`).

## What goes wrong

1. A query holds the web session.
2. An agent opens its `/mcp` event stream. MCP is on, so it gets `200 OK`, then waits on the session to replay the current grants.
3. The user turns MCP off. The switch answers at once.
4. The query ends.

The agent's stream is now subscribed and stays open, with MCP off. It receives every later grant, revoke, and deny, though `/mcp` answers 404 to everything else. A server test that holds the session from a thread, opens the stream, calls `set_mcp(false)`, and releases the session finds the stream still open three seconds later.

A narrower case needs no query: an agent that reconnects as soon as its stream closes can land between `close_all` and the flag turning off, and subscribe after the close.

## Why

`write_mcp_events` in `rust-impl/crates/dql-cli/src/web/server.rs` writes the headers, then takes `lock_state` to read the current grants, then calls `mcp_hub.subscribe`. The on/off check happened earlier, in `dispatch`. Nothing checks again after the wait.

`App::set_mcp` closes the streams before it stores the new flag:

```rust
if !enabled {
    app.mcp_hub.close_all();
}
app.hub.mcp(enabled);
app.mcp.store(enabled, Ordering::SeqCst);
```

A subscriber that arrives after `close_all` is never closed.

## What to change

Store the flag first, then close the streams. In `write_mcp_events`, check `mcp_enabled()` after `subscribe` and end the stream if MCP is off, so a subscriber that raced the close leaves on its own. Add the server test above.
