# Unauthenticated Token Recovery from `GET /` and Terminal Streams

## Summary

Even when MCP is disabled or unconfigured—so no `.cursor/mcp.json` exists in the workspace—any local process that can make an HTTP request to `dqlrs web` on loopback (`127.0.0.1:8765` by default) or read terminal logs can recover the full human `api-token` in cleartext.

## Violated boundary

- **Intended:** A local agent without explicit configuration or human approval cannot access `dqlrs` APIs or `/mcp`.
- **Actual:** `GET /` serves the full bearer token to any unauthenticated loopback caller, and CLI startup writes the token to terminal streams.

## Code paths

1. **Static routes skip authorization:** In `rust-impl/crates/dql-cli/src/web/server.rs` (`dispatch`), only `/mcp`, `/api/*`, and `OPTIONS` check `authorized(app, request)`. Static asset routes (`GET /`, `GET /index.html`, etc.) proceed with no authentication.
2. **Token injected into HTML response:** `serve_static` calls `inject_page_token` (`web/server.rs`), replacing `</head>` in `index.html` with:
   ```html
   <meta name="dqlrs-token" content="{app.token}">
   ```
   Every unauthenticated `GET http://127.0.0.1:<port>/` receives `app.token` in the response body.
3. **Token printed on startup streams:**
   - `rust-impl/crates/dql-cli/src/web/server.rs` prints `dqlrs token <hex>` to `stderr` whenever `dqlrs web` starts, and prints the JSON client configuration containing `"Authorization": "Bearer <hex>"` to `stdout` when `--mcp` is passed.
   - `rust-impl/crates/dql-cli/src/serve/tcp.rs` prints `dqlrs token <hex>` to `stderr` when `dqlrs --serve --bind` starts.

## Circumvention mechanics

### 1. Loopback HTML bootstrap fetch

`dqlrs web` listens on `127.0.0.1:8765` by default (`--listen` if overridden, discoverable via process arguments or local socket tables). Any agent with loopback HTTP access—even one running inside a different container sharing the host network namespace, or a sandboxed process with no filesystem access at all—can fetch `GET /`, read `<meta name="dqlrs-token" ...>`, and immediately call every `/api/*` and `/mcp` endpoint (see [`shared-token-and-api-escalation.md`](shared-token-and-api-escalation.md)).

Because cross-origin browsers are blocked by the Same-Origin Policy from reading the response to `GET http://127.0.0.1:8765/`, `inject_page_token` stops remote websites (`https://evil.example`) from stealing the token. It provides **zero** protection against a local non-browser process such as an agent.

### 2. IDE terminal and background process logs

Coding agents integrated into an IDE or terminal environment routinely have read access to open terminal panes, task outputs, or `~/.cursor` / shell log files where the user launched `dqlrs web` or `dqlrs --serve --bind`. Reading the `dqlrs token <hex>` line from stderr yields the persistent per-user token (which remains valid across restarts until deleted from the keystore).

## Remediation

- **Replace `<meta name="dqlrs-token">` with a one-time browser bootstrap:**
  - Generate an ephemeral, single-use launch nonce when `dqlrs web` starts and open the user's browser to `http://127.0.0.1:<port>/?launch=<nonce>` (or require the human to open that one-time link once).
  - On first `GET /?launch=<nonce>`, consume and invalidate the nonce immediately, set an `HttpOnly; SameSite=Strict` session cookie backed by random in-memory session state, and redirect to `/`.
  - Subsequent `GET /` requests without that session cookie must **not** mint or reveal any token.
  - Pair cookie authentication on `/api/*` with `Origin` / `Sec-Fetch-Site: same-origin` validation and a CSRF header/token bound to the session cookie so cross-origin browser requests remain blocked.
- **Stop printing persistent bearer tokens to stdout/stderr:**
  - Never print long-lived tokens to stderr on normal `dqlrs web` startup.
  - Rotate session secrets per server process rather than reusing a static per-user `api-token` across sessions.
