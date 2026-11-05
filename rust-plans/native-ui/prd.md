# PRD: native Rust app instead of the web UI

**Status:** the window ships as `dqlrs --ui` in the `dqlrs` binary. The web UI stays.

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

The window does not use `/api/*`. The web server, `/mcp`, and `--serve` stay. They are other programs, not this window.

## Toolkit

Ratatui is already the REPL (`repl/app.rs`, `repl/rich_table.rs`). It draws in a terminal. It does not replace the mouse-driven IDE: drag and drop, popovers, resizable panes, a real text caret, sticky table headers.

Do not embed the current page in a webview (`wry` or similar). That keeps JavaScript and a bridge, which is another API.

Use **[iced 0.14](https://book.iced.rs/index.html)** for the window. The book splits an interface into state, messages, `update`, and `view`. In 0.14 that is a `Program`: `update` may return a `Task`, and a `subscription` receives the directory watch and the run channel. The window state owns `Studio`. Short work runs inside `update`. Run and table refresh are `Task`s.

Chrome comes from widgets that already exist:

- `pane_grid` for the file tree, the editor, and the results pane. Dragging a split is the nav resize and the results resize.
- `text_editor` for the buffer. A custom `Highlighter` colors keywords, write bands, and lint ranges from the same tokenizer rules as `dql-tokens.js`. `repl/highlight.rs` is a start and is not enough. The highlighter is per line. Gutter run marks and the filename underline are widgets beside the editor, not decorations inside `text_editor`.
- `scrollable` with the header row outside the scroller for sticky table headers. Do not assume the 0.14 `table` widget pins its header.
- `button`, `text_input`, and `mouse_area` for menus, search, and dialogs.

egui, slint, and gpui are not the toolkit. gpui is still not a stable library for this repo. iced's editor is enough that we do not paint a rope ourselves unless `text_editor` cannot show a caret, a selection, and the highlighter.

iced 0.14 requires rustc 1.88 (its crates are edition 2024). This repo's toolchain file asks for stable. The first crate that depends on iced waits until the environment's stable is at least 1.88.

## Testing

Playwright is the spec today (`dqlrs-web/playwright/tests`). A native app cannot run those tests. [`testing.md`](testing.md) is the replacement.

1. **Core tests, no window.** Drive `Studio` the way the HTTP tests in `web/server.rs` drive routes. Most Playwright cases are this layer.
2. **`Simulator` tests.** `iced_test` builds `view()` with no display and no `Task`. Click, type, and `find` by widget id. This is the Playwright page object.
3. **A few `Emulator` scripts.** `.ice` files run the real program, including query `Task`s, against DynamoDB Local. The format is experimental, so it stays small.
4. **Snapshots.** `snapshot` writes a PNG through the software renderer (`tiny-skia`). A missing baseline fails. These replace the Playwright screenshots, not the assertions.

Do not delete Playwright until layers 1 and 2 cover the same behaviors. `just verify` grows `just native-ui-test`. It does not keep a browser in the loop after the cutover.

## Non-goals

- A new query language or a smaller feature set.
- Replacing the terminal REPL.
- Putting the UI in Wasm.
- Keeping a hidden HTTP server just so the window can `fetch` itself.
- Rewriting the DynamoDB engine.

## Decision

Build the native app as an iced 0.14 window over an in-process `Studio`. Use `text_editor` and a DQL `Highlighter`, and `pane_grid` for the layout. Drop `/api/*` and the JavaScript page only after the `Studio` tests and the `Simulator` tests cover today's Playwright behavior. Keep `/mcp` and `--serve`.

## Done when

- The window can edit, run, lint, search, and browse tables against the same engine the web UI uses, with no HTTP call on those paths.
- `just verify` runs the `Studio` suite and the `iced_test` suite without Playwright.
- `dqlrs --web` is either gone or documented as a leftover. The plan's last phase removes it.
