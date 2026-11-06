# Engine boundary

The window is a client of the studio, not of HTTP. The handlers in `rust-impl/crates/dql-cli/src/web/server.rs` already sit on `files`, `query_cache`, `web::lint`, `web::search`, `meta::ls`, and `Session::execute_for_serve`. The window calls those operations with typed values. While `dqlrs --web` still exists, its handlers call the same functions and then encode JSON.

## What the window calls

| Action | Operation |
| --- | --- |
| List, read, write, create, rename, move, duplicate, delete | Workspace files. Delete of a foreign folder returns the refusal the page shows. |
| Watch | A directory watch. The page re-reads on a timer. The window subscribes. A clean change replaces the buffer. A dirty change opens the patch dialog. |
| Lint | The same linter as `POST /api/lint`, on the buffer text, debounced. |
| Complete | Table names and key names from the cached catalog, using the same site rules as the page. |
| Run | `Session::execute_for_serve` on the statement text. Progress and cancel are a channel, not `application/x-ndjson`. |
| Stop | The session cancel. |
| Results | The per-file query-result cache. Sealed files, mode `0600`, directory `0700`, no plaintext secrets in the body. |
| Tables | `web_table_list` with pattern and refresh. Cache age comes from the cache timestamp. |
| Page of items | The same 50-row page the page requests. |
| Search | The same search index: tables, columns, indexes, queries. |
| Clear table cache | The same clear, then a fetch. |
| Endpoint label | Whatever the studio reports for the header pill. |

SAVE and LOAD stay inside the engine. The window does not add buttons for them. Relative paths use the process working directory.

## What the window does not do

- Open a socket to itself.
- Send a bearer token.
- Re-encode the studio types to JSON and parse them back.
- Embed the JavaScript page.
- Depend on `dql-output` drawing the terminal. Result cards are the window’s widgets. The terminal rich formatter stays with the REPL.

## Preferences

`ui.json` is the window’s. The page’s `localStorage` keys (`dqlrs-web.theme`, `dqlrs-web.highlight`, `dqlrs-web.side-width`, `dqlrs-web.results-height`, `dqlrs-web.explain-json`) are not read. A person who used the page does not carry those values over unless a later migration says so. Explain’s raw toggle persists in the window config under its own key.

## Errors

Engine errors become the error note on the card and, for a failed run, a toast whose title is **Run failed**. File operations toast **Saved**, **Created**, **Renamed**, **Moved**, **Duplicated**, **Deleted**, or the matching failure (**Create failed**, **Rename failed**, **Move failed**, **Duplicate failed**). Conflict toasts **The file changed on disk.** An empty folder run toasts or sets meta to **Nothing to run** / **That folder has no .dql files.** The results meta uses the page’s **Nothing to run**.
