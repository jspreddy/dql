# Plan: local API

**Group:** API. **Status:** implemented.

Loopback is already required. Each web server and each TCP `--serve` listener mints a token so a client that does not have it, including a page on another origin, cannot call the server just by knowing the port.

## Surfaces

| Surface | Bind | Auth today |
| --- | --- | --- |
| `dqlrs --web` | `--listen`, default `127.0.0.1:8765`. Non-loopback is refused | None on `/api/*` |
| `/mcp` on that same port | Same listener. Off unless `--mcp` | Non-loopback `Origin` is rejected only when the header is present |
| `dqlrs --serve` | stdio | The parent process owns the pipes |
| `dqlrs --serve --bind` | Loopback TCP, one client | None |
| Header environment | Served to the page | Access key id is masked. Secret and session token are not read |

Sensitive HTTP routes include `/api/run`, `/api/rows`, `/api/file`, `/api/query-results`, `/api/tables`, `/api/table-cache`, and `/mcp`. Static assets for the UI stay readable without a token so the page can load. The token is then required for `/api/*` and `/mcp`.

## Token

Mint a fresh 256-bit token each time the web server or a TCP `--serve` listener starts. Print one stderr line, `dqlrs token <hex>`, next to the listen line. Do not write it into the workspace or the cache.

The web page is served by this process. The HTML response for `/` carries `<meta name="dqlrs-token" content="<hex>">`. That tag is added in memory. `index.html` on disk stays unchanged. The page script sends `Authorization: Bearer <hex>` on `/api/*` and `/mcp`. It does not use a cookie. A cookie would be attached to cross-site posts. A missing or wrong token is HTTP 401 and does not run the query. Reloading the page picks up the new process's token.

TCP `--serve` uses the same token in a JSON field named `token`. A client may send it on every line, or once. `{"op":"auth","token":"<hex>"}` only marks that connection; later lines on that connection may omit the field. A line that includes a wrong token is rejected. Stdio `--serve` does not check the field, because the client is the parent of the process. An unauthenticated TCP line, including `shutdown`, does not run.

Hold the token in memory for the life of the process. A new process gets a new token. Old pages from a previous process fail until reload.

The printed MCP snippet includes the same bearer value in `headers.Authorization`. Anyone who can read that stderr line, or the HTML response, can call the API. The token is not a secret from the operator.

## What this does not stop

A process running as the same user can read the stderr line, or load the page, and call the API. That limit is stated in [`agents.md`](agents.md). The token is for other OS users and for browsers that are not this page.

## Credentials

Keep the current environment view: local DynamoDB shows `host:port`, a live session shows profile and region, the access key id is masked, and the secret is never loaded into that view. Do not add the secret, the session token, or the cache key to `/api/config`.

## Done when

- `/api/run` and `/mcp` without the token return 401 and do not execute.
- The same calls with the token match today's success behavior.
- A non-loopback `--listen` or `--bind` is still refused.
- `/api/config` still has no secret and no cache key.
- Stdio `--serve` still accepts a JSON line from the parent with no token.
