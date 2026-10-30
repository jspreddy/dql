# API JSON deviations

Status: plan only. No endpoint behavior is changed by this document.

Scope is deviation 7 only. The other seven deviations are accepted as they are. Do not plan them, change them, or follow up on them.

## Accepted as-is

These stay unchanged. No work items.

1. `POST /api/run` success is chunked newline-delimited JSON so the page can show progress.
2. `GET /` and HTML files are HTML, including the API token meta tag in the home page.
3. Stylesheets are CSS.
4. Script files are JavaScript.
5. Any other static file is raw bytes.
6. An MCP notification is HTTP 202 with an empty body labeled JSON.
8. A request body that is not a JSON object is treated as `{}`.

## To-do: deviation 7 uses POST JSON

Status: not started. This is the only change to make.

Five reads put filters in the URL as form-encoded query strings. Each one becomes an HTTP POST. The body is one JSON object, `Content-Type: application/json`, sent through the existing `api()` helper as `{ method: "POST", body: { ... } }`. They do not stay GET. The response stays the JSON document the GET handler already returns. No query string. `refresh` and `async` become booleans (`true` only where the URL used to send `1`). `page` becomes a number. The other fields stay strings.

`POST /api/file` already creates a file, and `POST /api/query-results` already saves a result. Those write calls stay as they are. The two reads that share those paths move to a new POST path so a read cannot create or save.

| Read | Method and path | JSON body | Replaces |
| --- | --- | --- | --- |
| File contents | `POST /api/file/read` | `{"path":"<workspace-relative path>"}` | `GET /api/file?path=` |
| Saved results | `POST /api/query-results/list` | `{"path":"<workspace-relative path>"}` | `GET /api/query-results?path=` |
| Title search | `POST /api/search` | `{"q":"<trimmed search text>"}` | `GET /api/search?q=` |
| Table list | `POST /api/tables` | `{"pattern":"<trimmed filter or empty string>","refresh":false,"async":false}` | `GET /api/tables?pattern=&refresh=&async=` |
| Table rows | `POST /api/rows` | `{"table":"<table name>","page":0}` | `GET /api/rows?table=&page=` |

`pattern`, `refresh`, and `async` are always present on the table-list body. `refresh` is `true` only for a forced refresh. `async` is `true` only for a background load. The catalog load that currently calls `GET /api/tables` with no query posts `{"pattern":"","refresh":false,"async":false}`.

Server work in `rust-impl/crates/dql-cli/src/web/server.rs`:

- Handle the five POSTs in `dispatch_post` by reading the JSON fields above and calling the same helpers the GET branches use now (`files::read_file`, `query_cache::list`, `search::search`, `web_table_list`, `table_rows`).
- Remove those five reads from `dispatch_get`. A leftover GET falls through as it does today (static 404 JSON for unknown `/api` paths, or the static file handler). Do not keep a GET query form beside the POST.
- Leave `POST /api/file` (create), `PUT /api/file` (save), `POST /api/query-results` (save one result), and `POST /api/query-results/clear` unchanged.

Front-end call sites in `dqlrs-web/static/app.js` to switch to POST:

- `openFile` reads a file with `path`.
- `watchOpenFile` re-reads the open file with `state.disk.path` about once a second.
- `folderDocuments` reads each file under a folder with `file.path`.
- `loadFileResults` loads history with `path`.
- `runTitleSearch` sends `q`.
- `loadTables` sends `pattern`, `refresh`, and `async`.
- `ensureCatalog` loads tables with an empty pattern and both flags false.
- `loadRows` sends `table` and `page`.

Tests that still assume GET query strings, to update in the same change:

- `dqlrs-web/playwright/tests/tables.spec.js` waits on `/api/tables` and reads `refresh`, `async`, and `pattern` from the URL. Assert POST and the JSON body instead.
- `rust-impl/crates/dql-cli/src/web/server.rs` GETs `/api/file?path=...` in the file-read tests. POST `/api/file/read` with `{"path":"..."}`.
- `dqlrs-web/playwright/tests/file-watch.spec.js` intercepts `/api/file` and delays PUT. Point reads at `/api/file/read` so that intercept still only delays saves.
