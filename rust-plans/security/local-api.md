# Plan: local API

**Group:** API. **Status:** plan.

Loopback is already required. This plan adds a token so another user, or a page on another origin, cannot call the server just by knowing the port.

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

Mint a fresh token each time the web server or a TCP `--serve` listener starts. Print it on stderr once, next to the listen line. Do not write it into the workspace or the cache.

The web page is served by this process. Its script sends the token on later calls, in a header, not in a cookie. A cookie would be attached to cross-site posts. A missing or wrong token is HTTP 401 and does not run the query.

TCP `--serve` requires the token on each JSON line, or in a first auth line, using one format for both. Stdio `--serve` does not, because the client is the parent of the process.

Hold the token in memory for the life of the process. A new process gets a new token. Old pages from a previous process fail until reload.

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
