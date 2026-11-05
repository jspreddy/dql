# Plan: native Rust UI

**Status:** not started. Follow [`prd.md`](prd.md). Do not open the window until step 1 has tests.

The web server's match arms are the behavior list. `Studio` is those operations without `Reply::Json`. The window is an iced `Program` that calls `Studio`. Playwright stays until step 4. How the clicks move from Playwright to `iced_test` is [`testing.md`](testing.md).

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

New crate `dql-studio`, and a `dqlrs --app` flag in `dql-cli` that depends on it. One iced window. `update` calls `Studio` for short work. Run and table refresh return a `Task`. A `subscription` reads the run channel and the directory watch. Cancel uses the flag the web server already uses. The watch calls `Studio` read only when the file's revision changes.

Build the file tree, result cards, table browser, settings, theme, and endpoint popover with `pane_grid`. The editor in this step is `text_editor` with no highlighter yet, enough to open a file. Every control the tests click gets a `widget::Id` (`menu-run-file`, `file-name`, `run`).

`Simulator` tests in this step: open the tree, switch Query and Tables, open a file, see the open-file id. Theme is the `Theme` value passed to the next `view`, asserted without a snapshot. Resize can wait until the grid is stable.

Do not add the iced dependency until rustc is at least 1.88. Headless tests use `tiny-skia` and `fira-sans`. The app window uses `wgpu`.

HTTP `/api/*` still works in this step so the browser and the window can be compared.

## Step 3: Editor

Keep `text_editor`. Add a DQL `Highlighter`.

- Token colors and statement spans in Rust. Start from `repl/highlight.rs` and match `dql-tokens.js` (keywords, write bands, statement ranges). One test file of DQL fixtures, shared expectations, so the window and the old page would paint the same words. The highlighter is line based. Assert the ranges in that fixture test, not by reading pixels.
- Lint ranges are highlighter spans. Run marks are a gutter widget beside the editor, one id per statement start.
- Ctrl+Enter and Ctrl+\ run the statement at the cursor. Esc cancels. Those shortcuts are `simulate` events in a Rust test. The `.ice` key list cannot express them.
- Completion reads the catalog `Studio` already loaded for the table browser.
- Autosave and the conflict patch dialog call the same save method as step 1.

`Simulator` tests: type a statement, see a lint span in the fixture, run it, see the gutter id, stop a throttled insert by injecting `RunStarted` and then clicking Stop. Core tests from step 1 still own the engine results. One light and one dark snapshot for the open-file mark and the running dot.

## Step 4: Keep the web UI

The window is a feature of the `dqlrs` binary (`dqlrs --ui`), not a replacement and not a second install. Leave `dqlrs-web`, `just web-test`, `/api/*`, `/mcp`, and `--serve` in place. The window does not talk to them. It calls `web::studio` with ordinary Rust calls.

## Step 5: Screen tests that core tests cannot see

Only after the window is the product. Sticky headers, sideways scroll of a wide result, the patch dialog's wording, context-menu labels. `Simulator` clicks and a committed PNG, not Chrome. Add them when a regression in paint shows up. They are not the gate for step 4. `.ice` scripts stay the small set in [`testing.md`](testing.md). The format is experimental.

## Out of this plan

- Rewriting ratatui REPL screens.
- A webview.
- Wasm in the browser.
- Changing DynamoDB execution to make the UI easier.
- egui, slint, or gpui.
