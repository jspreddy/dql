# Plan: native Rust UI

**Status:** not started. Follow [`prd.md`](prd.md). Do not open the window until step 1 has tests.

The web server's match arms are the behavior list. `Studio` is those operations without `Reply::Json`. The window is a caller of `Studio`. Playwright stays until step 4.

## Step 1: Studio, still behind HTTP

Add a `Studio` type next to `web/server.rs` that owns the workspace root and the `Session`. Move the work the handlers already do into methods that return Rust values, not HTTP:

- files: tree, read, create, save, conflict, mkdir, duplicate, move, delete
- lint report
- search hits
- table list, cache status, clear cache, one page of rows
- query-result list, save, clear
- run: statements in, a channel of progress and result events out, cancel

`dispatch_get` and `dispatch_post` become encode-only. Behavior does not change. Existing server tests keep passing.

Add Rust tests on `Studio` for the Playwright cases that are not about pixels. Port the assertions, not the clicks. Cover at least:

- file create, rename, move, duplicate, delete rules (`files.spec.js`)
- dirty save versus a change on disk (`file-watch.spec.js`)
- lint issues (`lint.spec.js`)
- title search (`search.spec.js`)
- run, stop, per-statement failure (`editor.spec.js` run cases)
- result history saved and cleared with the file
- table list filter, refresh flag, 50-row pages (`tables.spec.js`)
- SAVE and LOAD (`save-load.spec.js`)
- HELP text (`help.spec.js`, `nested.spec.js`)

These tests do not open a socket.

## Step 2: Window chrome

New binary, or a `dqlrs --app` flag, in `dql-cli`. One egui window. It calls `Studio` on the UI thread for short work and on a worker thread for run and for table refresh. Run progress arrives on a channel. Cancel uses the flag the web server already uses.

Build the file tree, result cards, table browser, settings, theme, and endpoint popover. No editor yet beyond a plain text buffer if that unblocks the chrome. Directory watch replaces the one-second file read. The watch calls `Studio` read only when the file's revision changes.

Widget tests in this step: open the tree, switch Query and Tables, resize is optional until the layout is stable. Theme is a `Studio` or window setting asserted without a screenshot if the widget tree exposes it.

HTTP `/api/*` still works in this step so the browser and the window can be compared.

## Step 3: Editor

Replace the plain buffer with a rope editor drawn by egui.

- Token colors and statement spans in Rust. Start from `repl/highlight.rs` and match `dql-tokens.js` (keywords, write bands, statement ranges). One test file of DQL fixtures, shared expectations, so the window and the old page would paint the same words.
- Lint ranges map onto the rope. Run marks map onto statement starts.
- Ctrl+Enter and Ctrl+\ run the statement at the cursor. Esc cancels.
- Completion reads the catalog `Studio` already loaded for the table browser.
- Autosave and the conflict patch dialog call the same save method as step 1.

Widget tests: type a statement, see a lint range, run it, see a gutter mark, stop a throttled insert. Core tests from step 1 still own the engine results.

## Step 4: Cut over

When step 1 and step 3 tests cover the Playwright list in the PRD, remove `dqlrs-web/static` from the binary, remove `just web-test` from `just verify`, and delete `/api/*`. Keep `/mcp` and `--serve` on their own protocols. They do not grow a dependency on egui.

`dqlrs --web` goes away in this step, or prints that the desktop app replaced it. Do not leave the JSON API up for the window.

## Step 5: Screen tests that core tests cannot see

Only after the window is the product. Sticky headers, sideways scroll of a wide result, the patch dialog's wording, context-menu labels. egui headless clicks, not Chrome. Add them when a regression in paint shows up. They are not the gate for step 4.

## Out of this plan

- Rewriting ratatui REPL screens.
- A webview.
- Wasm in the browser.
- Changing DynamoDB execution to make the UI easier.
