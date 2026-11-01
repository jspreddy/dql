# PRD: native Rust app instead of the web UI

**Status:** investigation. Do not start the window from this document until the core in [`plan.md`](plan.md) is accepted.

## Problem

`dqlrs --web` is two programs. A browser runs `dqlrs-web/static` (hand-written JavaScript plus CodeMirror). A Rust process serves that page and a JSON HTTP API. Every click is a request: open a file, lint, run, list tables, load a page of rows, save a result.

That split costs a token, a JSON encode, a socket, and a JSON decode on the way to a `Session` that already lives in the same process. The UI cannot call `Session::execute_for_serve` or `web::lint::lint_script` directly. Playwright is the test suite, so the behavior is specified as browser clicks against that API.

A native window in the same process can call those functions. The HTTP API between the screen and the engine can go away. External clients are a separate question: MCP and `dqlrs --serve` are not the screen.

## Who this is for

The person who runs the web UI locally to edit `.dql` files, run them, and browse tables. Agents that talk to `/mcp` are out of this replacement.

## Same functionality

The native app has to do what the page does now. Layout details can change. Behavior cannot.

- Workspace file tree: create, rename, duplicate, move, delete, drag onto a folder, refuse to delete a folder that holds a non-`.dql` file.
- Editor for the open `.dql` file: autosave, conflict when the file changes underneath a dirty buffer, unsaved patch kept in the window, syntax colors, write bands, lint underlines, completion of table and key names, run the statement at the cursor, run the selection, run the file, run a folder.
- Run feedback: progress, a running mark in the gutter, stop, per-statement success and failure, scroll the failing statement into view.
- Results: tables, explain plans, help pages, named cards, row JSON, schema from the rows already loaded, capacity chips, history stored with the query file and cleared per file or per folder.
- Table browser: list with the same matching as `ls`, refresh in the background, cache age, clear cache, 50-row pages, sticky headers, column order by keys then index then name.
- Title search across tables, cached columns and indexes, and queries in files.
- Header: endpoint and caller identity, light and dark theme, highlight toggles, keyboard shortcut list.
- SAVE and LOAD of JSON and CSV, including absolute paths and `CREATE PARENTS`.

Presence icons exist so several browser tabs and an MCP client can see each other. A native app is one window in the server process. It does not need a poll to discover itself. MCP clients still need a way to show up if `/mcp` stays.

## What is not the same

- No browser, no `index.html`, no CodeMirror, no `app.js`.
- No bearer token on the UI path. The window is the process. The token stays only if `/mcp` or TCP `--serve` stay.
- File changes are a directory watch, not `POST /api/file/read` every second with the whole file body.
- Progress is a typed channel from the run, not `application/x-ndjson` over HTTP.
- Theme and pane sizes live in the app's config, not `localStorage`.

## Move off the UI API

The handlers in `rust-impl/crates/dql-cli/src/web/server.rs` already sit on top of plain Rust: `files`, `query_cache`, `web::lint`, `web::search`, `meta::ls::web_table_list`, and `Session::execute_for_serve`. The native UI should call a `Studio` facade with those operations and typed values. HTTP handlers, while they still exist, call the same facade and then encode JSON.

When the window is the only screen, delete `/api/*` and the static file server. Keep `/mcp` and stdio or TCP `--serve` until something else replaces them. Those are other programs, not this window.

## Toolkit

Ratatui is already the REPL (`repl/app.rs`, `repl/rich_table.rs`). It draws in a terminal. It does not replace the mouse-driven IDE: drag and drop, popovers, resizable panes, a real text caret, sticky table headers.

Do not embed the current page in a webview (`wry` or similar). That keeps JavaScript and a bridge, which is another API.

Use **egui** for the window chrome: tree, tables, settings, result cards, dialogs. It is pure Rust, and it can run headless in tests. Its stock text field is not this editor. The editor is a rope (`ropey`) painted by egui, with highlights from a Rust tokenizer. `repl/highlight.rs` is a start and is not enough: the web tokenizer in `dql-tokens.js` knows more words, query bands, and statement spans. One tokenizer has to feed both the terminal help path and the window, or the window gets its own and the REPL list is left alone.

gpui (the Zed toolkit) is the only ready-made Rust code editor, and it is not a stable library for this repo. iced and slint do not remove the need to write the editor. They are not the recommendation.

## Testing

Playwright is the spec today (`dqlrs-web/playwright/tests`). A native app cannot run those tests. The replacement has two layers.

1. **Core tests, no window.** Drive `Studio` the way the HTTP tests in `web/server.rs` drive routes: create a file, save, conflict, lint, run, stop, search, list tables, page rows, save and load. Most Playwright cases are this layer: `files.spec.js`, `lint.spec.js`, `search.spec.js`, `save-load.spec.js`, `functions.spec.js`, `nested.spec.js`, `help.spec.js`, and the run and result behavior in `editor.spec.js` and `tables.spec.js`.
2. **Widget tests.** egui can render without a display and accept clicks. Use that for the cases that are about the screen: theme, pane resize, sticky headers, sideways scroll, gutter marks, the patch dialog, context menus. Those tests replace the Playwright assertions that read pixels and DOM ids. They do not need Chrome.

Do not delete Playwright until the core tests cover the same behaviors and the widget tests cover the screen cases that core tests cannot see. `just verify` grows a native-ui test target. It does not keep a browser in the loop after the cutover.

## Non-goals

- A new query language or a smaller feature set.
- Replacing the terminal REPL.
- Putting the UI in Wasm.
- Keeping a hidden HTTP server just so the window can `fetch` itself.
- Rewriting the DynamoDB engine.

## Decision

Build the native app as a window over an in-process `Studio`, with egui for chrome and a Rust editor. Drop `/api/*` and the JavaScript page only after Rust tests cover today's Playwright behavior. Keep `/mcp` and `--serve`.

## Done when

- The window can edit, run, lint, search, and browse tables against the same engine the web UI uses, with no HTTP call on those paths.
- `just verify` runs the core suite and the widget suite without Playwright.
- `dqlrs --web` is either gone or documented as a leftover. The plan's last phase removes it.
