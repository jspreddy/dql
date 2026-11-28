# 10. Grants are shared by every MCP client, and the agent label is self-declared

**Severity:** Medium. **Found by:** Cursor. **Needs:** a second MCP client on the same server.
**Under `aws-vault exec`:** unchanged.

## What happens

- **One pool of grants.** `App` holds one `Session` and one `access::Store` for every `/mcp`
  caller. `list_granted_tables`, `read_granted_table`, and `request_access` with
  `request: "r1"` are not tied to the MCP session or agent that asked. A second agent lists the
  grants and reads data the person granted for a different task, or polls `r1`, `r2`, … to watch
  requests.
- **The agent name is the agent's word.** The name in the grant panel comes from the tool call's
  optional `agent` argument, or from the self-declared `clientInfo.name` in `initialize`. An agent
  can present itself as a trusted one.

## Code

- `web/server.rs` `request_access`, `App`'s shared `WebState`.
- `web/access.rs` `Store::open(query, agent)`.

## Status

Cursor's review, from the code. Not run in this reconciliation.

## Fix

- Issue a distinct MCP token per client, and bind each request id and each grant to the token
  that asked. `list_granted_tables` and `read_granted_table` show only that client's grants.
- Take the agent's name from the server-side token registration, not from the call.
