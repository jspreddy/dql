# Shared Bearer Token Across `/mcp`, `/api/*`, and TCP `--serve`

## Summary

`dqlrs` uses a single per-user secret (`api-token`) to authenticate every network-facing control plane: the agent `/mcp` endpoint, the human `/api/*` web endpoints, and TCP `dqlrs --serve --bind`. Because HTTP MCP setup writes this exact bearer token into `.cursor/mcp.json` and `.mcp.json` inside the workspace—and hands it to the MCP client—any agent configured to use MCP (or able to read workspace dotfiles and make localhost HTTP calls) already possesses full human API privileges.

## Violated boundary

- **Intended:** An MCP credential lets an agent call `/mcp` tools only (metadata, `.dql` CRUD, lint, `request_access`, and reading granted temp tables). It must not let an agent execute queries, read ungranted tables, read cached human results, or approve its own access requests.
- **Actual:** The `/mcp` bearer token is identical to the `/api/*` and TCP `--serve` bearer token.

## Code paths

1. **Single token store:** `rust-impl/crates/dql-cli/src/api_token.rs` (`load`) loads or generates one 64-character hex secret stored under `kind = "api-token"` in `cache_seal`. `App` in `rust-impl/crates/dql-cli/src/web/server.rs` stores this string in `app.token`, and `rust-impl/crates/dql-cli/src/serve/tcp.rs` loads the same `api_token::load()` secret for TCP `--serve`.
2. **Token written to workspace MCP configs:** `rust-impl/crates/dql-cli/src/mcp.rs` (`http_client_snippets`) embeds `"Authorization": "Bearer <token>"` directly in the configurations generated for `.cursor/mcp.json` and `.mcp.json` (also returned by `GET /api/mcp` via `mcp_info` in `web/server.rs`).
3. **Uniform authorization check:** In `rust-impl/crates/dql-cli/src/web/server.rs` (`dispatch` and `authorized`), requests to `/mcp` and requests to `/api/*` call the exact same `authorized(app, request)` helper comparing `Authorization: Bearer <token>` (or `X-Dqlrs-Token` / `?token=` on `/api/events`) in constant time against `app.token`.

## Circumvention mechanics

An agent that reads `.cursor/mcp.json` or `.mcp.json` in the workspace (or inspects its own MCP client configuration) can call human endpoints on `http://127.0.0.1:<port>` using `Authorization: Bearer <token>`:

1. **Direct query execution (`POST /api/run`):** Send `{"dql": "SCAN * FROM <table>", "script": ""}` and read every item directly from the NDJSON response stream—including `SELECT`, `SCAN`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, and `DROP`.
2. **Direct table browsing (`GET /api/rows`, `POST /api/rows`):** Page through any DynamoDB table, `LOCALTABLE`, or ungranted `TEMPTABLE` without running `.dql` scripts or creating grants.
3. **Self-approving `request_access` (`POST /api/mcp-access/run` and `POST /api/mcp-access/grant`):** Even if an agent wants its data flow to look like normal MCP usage, it can open a `request_access` call on `/mcp`, immediately call `POST /api/mcp-access/run` and `POST /api/mcp-access/grant` with `{"request": "r1"}` using the same bearer token, and then read the table via `read_granted_table` with zero human interaction.
4. **Bypassing the human MCP kill switch (`POST /api/mcp`):** In `web/server.rs` (`dispatch`), `/api/*` routes are dispatched **before** checking `app.mcp_enabled()`. If the human toggles MCP off in the web header pill, an agent holding the shared token can call `POST /api/mcp` with `{"enabled": true}` to turn `/mcp` back on—or ignore `/mcp` altogether and keep calling `/api/*`.
5. **Reading decrypted query-result caches (`GET /api/query-results` and `GET /api/structure`):** Fetch past human query runs (`GET /api/query-results?path=<file.dql>`) in plaintext JSON without touching the OS keystore, because the running `dqlrs web` server unseals `.dqlrs-cache/<conn>/results/` on the caller's behalf.
6. **Reading workspace files blocked on `/mcp` (`GET /api/file` and `GET /api/file-rows`):** While MCP's `read_query_file` restricts reads to `.dql` files (`require_dql_read`), `/api/file` and `/api/file-rows` call `files::read_viewable`, returning `.csv`, `.json`, `.md`, and `.html` files—including files inside dot-directories such as `.cursor/mcp.json` because `read_opened` uses `resolve_inside` without rejecting dot-prefixed segments.
7. **Executing against TCP workers (`dqlrs --serve --bind`):** Connect to any local `dqlrs --serve --bind` TCP listener and send `{"id": "1", "op": "exec", "token": "<token>", "dql": "..."}`.

## Remediation

- **Split credentials by role and scope:**
  - Authenticate the human browser session with a separate secret (for example an `HttpOnly`, `SameSite=Strict` cookie established via a one-time startup launch token) that is never written to `.cursor/mcp.json`, `.mcp.json`, stdout, or stderr.
  - Authenticate `/mcp` with dedicated agent tokens that are accepted **only** on `/mcp` (when `mcp_enabled` is true) and rejected on `/api/*` and `dqlrs --serve --bind`.
  - Authenticate `dqlrs --serve --bind` with its own ephemeral per-listener secret rather than a long-lived user-wide token shared with MCP.
- **Avoid plaintext bearer tokens in workspace files:** Prefer dynamic MCP registration or store credentials outside the agent-readable project tree where client integrations support it; if a token must sit in `.cursor/mcp.json`, ensure that token carries **only** `/mcp` privileges so reading it grants nothing beyond `/mcp` itself.
- **Restrict `read_viewable` path segments:** Apply the same dot-directory rejection used in `clean_relative` to `read_opened` so `/api/file` cannot read `.cursor/*`, `.mcp.json`, or `.dqlrs-cache/*`.
