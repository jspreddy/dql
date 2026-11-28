# 01. The page serves the token without auth, and the Host header is not checked

**Severity:** Critical. **Found by:** both; DNS rebinding by Claude. **Needs:** an HTTP request
to the `dqlrs web` port, from any local process, or from a web page through DNS rebinding.
**Under `aws-vault exec`:** unchanged. The token drives the running server, which holds the
credentials.

## What happens

`/api/*` and `/mcp` require `Authorization: Bearer <token>`. The page does not.

1. **`GET /` returns the token** (both). `serve_static` adds
   `<meta name="dqlrs-token" content="…">` to `index.html` for every caller. Any local process
   runs `curl http://127.0.0.1:8765/`, takes the tag, and calls every `/api/*` route (see 02).
   The port is fixed by default (`8765`), and `--listen` values show up in process arguments and
   socket tables. A process with no file access at all, or in another container sharing the
   host network, can do this.
2. **DNS rebinding** (Claude). The server never reads `Host`. A page on `attacker.example`
   rebinds its name to `127.0.0.1`. The browser then treats `http://attacker.example:8765/` as
   the same origin, so the page reads `/`, takes the token, and calls `/api/run`.
3. **The token on startup streams** (both). `dqlrs web` prints `dqlrs token <hex>` and, with
   `--mcp`, the client config with the bearer value. `dqlrs --serve --bind` prints the token too.
   Terminal panes, task output, and logs that an agent can read carry a token that never
   rotates (03).

## Reconciled

Cursor's review says the same-origin policy keeps websites from reading `GET /`. That holds only
while the `Host` header is ignored by the attacker too. DNS rebinding makes the page same-origin,
and the server answers any `Host`. Claude reproduced the request with `Host: attacker.example`.

## Reproduced

Claude, against `dqlrs web --mcp` on the memory backend with a planted value:

```bash
curl -s -H "Host: attacker.example" http://127.0.0.1:18765/ | grep -o 'dqlrs-token" content="[0-9a-f]*'
```

That returned the 64-character token with no credentials, and `POST /api/run` with it returned
the planted row from `SCAN * FROM users`.

## Code

- `web/server.rs:743`: the token check covers `/api` and `/api/*` (and `/mcp` above it); other
  paths go to `serve_static`.
- `web/server.rs:2249` `serve_static`, `:2266` `inject_page_token`.
- `web/server.rs` request parsing reads `Origin`, `Authorization`, and `Accept`, never `Host`.
- `api_token.rs:30` `stderr_line`; `serve/tcp.rs` prints the same line.

## Fix

- A one-time bootstrap instead of the `<meta>` tag (both): mint a single-use launch nonce per
  process, open `http://127.0.0.1:<port>/?launch=<nonce>`, trade it once for an in-memory session
  (an `HttpOnly`, `SameSite=Strict` cookie plus a CSRF header). A later `GET /` without the session
  reveals nothing.
- Reject any `Host` other than `127.0.0.1:<port>`, `localhost:<port>`, or `[::1]:<port>` before
  routing (Claude). Check `Origin` and `Sec-Fetch-Site` on `/api/*` (both).
- Stop printing long-lived tokens; rotate per process (both).
- None of this stops a same-user process that reads the terminal or the browser profile. See 03
  and the README.
