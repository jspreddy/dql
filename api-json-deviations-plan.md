# API JSON deviations

Status: plan only. No endpoint behavior is changed by this document.

The browser UI in `dqlrs-web/static` talks to the custom HTTP server in `rust-impl/crates/dql-cli/src/web/server.rs`. The expected data contract is one JSON document per API call: `Content-Type: application/json; charset=utf-8`, body produced by `Reply::Json`. The client helper `api()` in `dqlrs-web/static/app.js` sends `JSON.stringify` bodies and reads `response.json()`.

## How data moves

1. `GET /` returns `index.html`. The server splices `<meta name="dqlrs-token" content="...">` into that HTML. `app.js` reads the meta tag and patches `window.fetch` so `/api/*` and `/mcp` send `Authorization: Bearer <token>`.
2. Almost every data call goes through `api()`. A non-OK status throws with `payload.error`. A body that is not JSON becomes `{}` on the client (`response.json().catch(() => ({}))`).
3. `POST /api/run` is the exception on the success path. `runDocuments` uses `fetchServer` and `readNdjson`, which splits the body on newlines and `JSON.parse`s each line.
4. `retryServer` calls `fetch("/api/config")` and `response.json()`. That response is JSON. The patched `fetch` still attaches the bearer token.

### Calls that already return one JSON document

| Method | Path | Client |
| --- | --- | --- |
| GET | `/api/config` | `api`, and `retryServer` |
| GET | `/api/identity` | `api` |
| GET | `/api/tree` | `api` |
| GET | `/api/file?path=` | `api` |
| GET | `/api/presence` | server only; the page posts presence |
| GET | `/api/query-results?path=` | `api` |
| GET | `/api/search?q=` | `api` |
| GET | `/api/table-cache` | `api` |
| GET | `/api/tables` | `api` |
| GET | `/api/rows?table=&page=` | `api` |
| POST | `/api/stop` | `api` |
| POST | `/api/presence` | `api` |
| POST | `/api/file` | `api` |
| POST | `/api/mkdir` | `api` |
| POST | `/api/duplicate` | `api` |
| POST | `/api/move` | `api` |
| POST | `/api/delete` | `api` |
| POST | `/api/query-results` | `api` |
| POST | `/api/query-results/clear` | `api` |
| POST | `/api/lint` | `api` |
| POST | `/api/table-cache/clear` | `api` |
| PUT | `/api/file` | `api` |

JSON error bodies already used: `{"ok":false,"error":"..."}` for 400, 401, 404, 409, and 502 on `/api/*`, including overflow `{"ok":false,"error":"body too large"}`. `/mcp` errors are JSON-RPC objects. `POST /api/run` with no statements returns JSON 502 `{"ok":false,"error":"nothing to run"}` before any stream starts.

These are JSON and are not deviations: HTTP 200 `{"ok":false,"error":"..."}` from `/api/identity` when there is no caller; `serde_json::to_value(...).unwrap_or(Value::Null)` still serializes as JSON `null`.

## Deviations

Eight interactions are not one JSON document.

### 1. `POST /api/run` success body

- Call site: `runDocuments` → `fetchServer("/api/run")` → `readNdjson` in `dqlrs-web/static/app.js`. Server: `handle_run` → `Reply::Ndjson` → `write_ndjson` in `server.rs`.
- Actual format: `Content-Type: application/x-ndjson; charset=utf-8`, HTTP chunked. Each chunk payload is a JSON object plus `\n`. Event objects are `{"event":"statement","index":N}`, `{"event":"progress","index","done","total","phase"}`, and `{"event":"result","index","result"}`. The whole body is not one JSON value.
- Why: results and progress are written as each statement runs (`run_script` / `write_json`) so the page can update the progress bar before the script finishes. The connection closes when the stream ends (`Connection: close`).
- Suggested fix: stop emitting `application/x-ndjson`. Return one JSON document, for example `{"ok":true,"events":[...]}` with the same event objects in order, status 200, `Content-Type: application/json`. Point `readNdjson` at `response.json()` and replay `events` in order. Live progress then waits until the script finishes. If progress must stay live, add `GET /api/runs/<id>` that returns one JSON object `{"ok":true,"done":false,"events":[...]}` on each poll, and make `POST /api/run` return `{"ok":true,"id":"..."}` immediately. The client already looks for `event === "error"`, but the server never writes that event; fold failures into the `result` object (already done) or add an `error` event inside the same JSON document.

### 2. `GET /` and `*.html`

- Call site: browser navigation to `/`. Server: `dispatch_get` falls through to `serve_static` for any GET that is not an `/api/*` route. `/` maps to `index.html`.
- Actual format: `text/html; charset=utf-8`. `index.html` is rewritten in memory to insert the API token meta tag. Unknown paths that fail the static-file check return JSON 404.
- Why: the browser needs a document. The token is embedded here so later `/api` calls can send a bearer header. `GET /api/config` does not include the token.
- Suggested fix: keep HTML off the data API. Serve `dqlrs-web/static` from a static host, and have this server answer only `/api` and `/mcp` with JSON. The token still has to reach the page once; a JSON substitute is a loopback-only `GET /api/bootstrap` body `{"token":"..."}` fetched by a tiny static boot script, after which every later call stays JSON. Do not wrap the HTML document in a JSON string if a browser is expected to render `GET /`.

### 3. `GET` of `*.css`

- Call site: `index.html` loads `/theme.css` and `/app.css`.
- Actual format: `text/css; charset=utf-8` (`content_type` in `server.rs`).
- Why: stylesheets are static assets, same `serve_static` path as the document.
- Suggested fix: same split as HTML. Do not return CSS from the JSON API. A JSON body such as `{"ok":true,"css":"..."}` would not be applied by `<link rel="stylesheet">`.

### 4. `GET` of `*.js`

- Call site: `<script type="module" src="/app.js">`, the import map (`/vendor/*.js`), and imports of `/dql-mode.js`, `/dql-tokens.js`, `/describe-card.js`, `/file-patch.js`.
- Actual format: `text/javascript; charset=utf-8`.
- Why: the page is a module graph served as files.
- Suggested fix: same split as HTML. Leave these responses as JavaScript on a static host. They are not data payloads, and a JSON wrapper would fail the module loader.

### 5. Any other static file

- Call site: `serve_static` for a GET path that resolves to a file whose extension is not `html`, `css`, or `js`. No such file is in `dqlrs-web/static` today. The branch is still live: `content_type` returns `application/octet-stream`.
- Actual format: raw file bytes, `application/octet-stream`.
- Why: the type map has no other media types (no PNG, SVG, WOFF, or plain text).
- Suggested fix: do not serve those bytes from the API process. If a file must be data, return JSON, for example `{"ok":true,"path":"...","encoding":"base64","bytes":"..."}`, and teach the client to decode it. Prefer not adding binary assets to this server.

### 6. `POST /mcp` when the message is a notification

- Call site: `dispatch_mcp`. The web UI does not call `/mcp`; it only treats `/mcp` as an API path when attaching the bearer token. MCP clients do call it.
- Actual format: HTTP 202, `Content-Type: application/json`, body length 0 (`Reply::Bytes(202, "application/json", Vec::new())`). An empty body is not JSON. `exchange` in `mcp.rs` returns `None` when the message has no `id` (a notification). Requests that do have an `id` return a JSON-RPC object.
- Why: JSON-RPC notifications do not get a result object. The HTTP layer still labels the empty body as JSON.
- Suggested fix: `Reply::Json(202, json!({}))` so the body is `{}`. Callers that ignore notifications can ignore the object. Do not leave `Content-Type: application/json` on a zero-length body.

### 7. GET query strings are form-encoded, not JSON

- Call sites in `app.js`:
  - `GET /api/file?path=`
  - `GET /api/query-results?path=`
  - `GET /api/search?q=`
  - `GET /api/tables?pattern=&refresh=&async=` via `URLSearchParams`
  - `GET /api/rows?table=&page=`
- Actual format: request targets use `application/x-www-form-urlencoded` query encoding (`encodeURIComponent` / `URLSearchParams`). Responses for these routes are JSON. `+` and `%HH` are decoded in `query_value` / `percent_decode`.
- Why: the handlers are GET and take filters in the URL. POST and PUT bodies on the other routes are JSON objects.
- Suggested fix: accept the same fields as a JSON body so the interaction is JSON on both sides. Either switch these reads to POST (`{"path":"..."}`, `{"q":"..."}`, `{"pattern":"...","refresh":true,"async":true}`, `{"table":"...","page":0}`) or accept a single query parameter `?json=` whose value is a JSON object. Prefer POST bodies matching the existing `api()` helper. Keep returning one JSON document.

### 8. Non-JSON request bodies are treated as `{}`

- Call site: `parse_json` in `server.rs`, used by `POST /api/run` and every other POST/PUT under `/api` except `/api/stop` (no body).
- Actual format: an empty body, a body over the limit, a JSON array or string, or invalid JSON becomes an empty object. The handler then runs with missing fields (`dql` becomes `""`, `path` becomes `""`, and so on) and often responds with a normal JSON success or a later JSON error. The failure is not reported as invalid JSON.
- Why: `parse_json` matches only `Value::Object` and maps everything else to `Value::Object(Default::default())`.
- Suggested fix: on a non-empty body that is not a JSON object, return 400 `{"ok":false,"error":"invalid json"}` with `Content-Type: application/json`. Do not run the handler. Empty body may stay `{}` only for routes that allow it.

## Not counted

- In-page drag and drop uses `text/plain` (`dataTransfer` in `app.js`). That stays in the browser and never hits the server.
- The bearer token is an HTTP header, not a body format.
- Stdio and TCP `--serve` framing are outside this web UI path.
