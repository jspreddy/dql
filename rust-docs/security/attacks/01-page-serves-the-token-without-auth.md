# 01. The page serves the token without auth, and the Host header is not checked

**Severity:** Critical. **Needs:** an HTTP request to the `dqlrs web` port: `curl` from any local
process, or a browser page on any site through DNS rebinding.

## What happens

`/api/*` and `/mcp` require `Authorization: Bearer <token>`. The page itself does not. `GET /`
returns `index.html` with the token added as `<meta name="dqlrs-token" content="…">`, to any
caller. The server never reads the `Host` header, so the reply is the same for
`Host: attacker.example`.

1. Any local process reads the token: `curl http://127.0.0.1:8765/` and take the `<meta>` tag.
   The default port is fixed (`8765`), and a port scan of loopback finds others.
2. With the token, `/api/run` runs any statement, `/api/rows` reads any table, and
   `/api/query-results` returns cached results.
3. A web page on another site can do the same through DNS rebinding: its hostname first resolves
   to the attacker's server, then to `127.0.0.1`. The browser treats the second answer as the
   same origin, so the page reads `/`, takes the token, and calls `/api/run`. Requiring a bearer
   header does not help once the page can read the token.

## Reproduced

Against `dqlrs web --mcp` on the memory backend (`DQL_BACKEND=memory`), with a table holding a
planted value:

```bash
curl -s -H "Host: attacker.example" http://127.0.0.1:18765/ | grep -o 'dqlrs-token" content="[0-9a-f]*'
```

That returned the 64-character token with no credentials. A `POST /api/run` with that token and
`SCAN * FROM users` returned the planted row.

## Code

- `web/server.rs:743`: the token check covers only `/api` and `/api/*` (and `/mcp` above it).
  Every other path falls through to `serve_static`.
- `web/server.rs:2249` `serve_static`, `:2266` `inject_page_token`: `index.html` gets the token
  for every caller.
- `web/server.rs` request parsing reads `Origin`, `Authorization`, and `Accept`; it never reads or
  checks `Host`.

## Impact

Every other control on the web server reduces to "can reach the port". Any agent with a shell or
an HTTP tool, and any web page the person visits while `dqlrs web` runs, can run queries.

## Fix

- Never put a long-lived secret in a page that any request can fetch. Options:
  - Mint a per-process, single-use bootstrap code. Print it in the terminal as part of the URL
    the person opens (`http://127.0.0.1:8765/#code=…`). The page trades it once for a session
    secret held in memory, as Jupyter does. A second `GET /` gets no secret.
  - Or set the session as an `HttpOnly`, `SameSite=Strict` cookie on that one exchange, and
    require a custom header as well, so a cross-site request cannot use it.
- Reject any request whose `Host` is not `127.0.0.1:<port>`, `localhost:<port>`, or
  `[::1]:<port>`, before routing. That alone stops DNS rebinding.
- Check `Origin` on `/api/*` as `/mcp` already does.
- Neither stops a same-user process that can read the terminal or the person's browser
  profile. See 03 and the README.
