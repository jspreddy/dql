# Turning MCP on or off waits behind a running query, and the gate flips first

Status: open. Found on `0.7.12-alpha.1` in the MCP pill from #304 (`887fa7a`, `4ce9a86`).

## What goes wrong

Start a query that holds the web session (any `/api/run`). While it runs, open the MCP menu and flip **Serve MCP**.

- The switch stays disabled until the query finishes. `POST /api/mcp` has not returned.
- New agent calls to `/mcp` already get 404 as soon as the switch is turned off, or start succeeding as soon as it is turned on.
- Open agent event streams stay up until the query releases the session. `close_all` has not run.
- The other tabs’ pills do not update until then. The `mcp` event is published only after the lock.

So for the whole query, the HTTP gate, the open streams, and the pill disagree.

## Why

`accept` handles each connection on its own thread. `run_script` takes `lock_state` for the whole script (`dqlrs-web` server, `run_script` in `rust-impl/crates/dql-cli/src/web/server.rs`). An MCP tool call holds that same lock inside `dispatch_mcp`.

`App::set_mcp` does this, in order:

1. `self.mcp.swap(enabled)` — `/mcp` starts returning 404, or starts answering, immediately. `dispatch` reads this flag and does not take the session lock.
2. `lock_state(self).session.serves_mcp = enabled` — blocks until the query (or tool call) drops the lock. The GRANT note reads `serves_mcp`.
3. Only then, if the value changed: `mcp_hub.close_all()`, then `hub.mcp(enabled)`, then the HTTP response.

The pill disables the switch until that response arrives (`dqlrs-web/static/app.js`, the `mcp-switch` change handler).

## What to change

Make the toggle’s user-visible work not wait on a query.

- Flip the atomic flag, close streams, publish the `mcp` event, and return the HTTP response without holding `lock_state` across another request’s query.
- Update `session.serves_mcp` under the lock, but do not do the stream close or the event publish after waiting for it. If the flag and `serves_mcp` must move together, move both before any wait, or stop taking the session lock for the whole of `run_script`.
- Add a server test: start a run that blocks inside the session lock, `POST /api/mcp` with `{"enabled": false}`, and assert the response and the stream close do not wait for that run. The existing test `mcp_turns_on_and_off_from_the_web_app` only toggles while the server is idle.
