# Testing the native UI the way Playwright tests the web UI

**Status:** plan. No test code yet. The window is not started until [`plan.md`](plan.md) step 1 has Rust tests.

Playwright (`dqlrs-web/playwright/tests`) is a browser driving `dqlrs --web`. It clicks, types, asserts text, and saves a few PNGs. The native app has no DOM and no Chrome. [iced 0.14](https://github.com/iced-rs/iced/tree/0.14.0) ships the same two jobs in `iced_test`:

- `Simulator` builds the `view()` tree with no window and no side effects. It is `page.locator` plus `click`.
- `Emulator` runs the real `Program`, including `Task`s. It is the Playwright fixture that boots the app.

The book at <https://book.iced.rs/architecture.html> splits an interface into state, messages, `update`, and `view`. Tests use that split. A Simulator test calls `view`, produces messages, feeds them to `update`, then calls `view` again. An Emulator test lets the runtime do that loop.

## What replaces a Playwright test

```rust
let mut app = App::preset_empty(workspace);
let mut ui = Simulator::with_size(Settings::default(), Size::new(1280.0, 800.0), app.view());
ui.click(id("menu-run-file"))?;
for message in ui.into_messages() {
    app.update(message);
}
let mut ui = Simulator::with_size(Settings::default(), Size::new(1280.0, 800.0), app.view());
ui.find(id("file-name"))?;
```

That is the shape of `files.spec.js` "runs the selected file from the context menu", without Chromium. `id(...)` is `widget::Id`, the same role as `data-testid` and `#menu-run-file`. Exact text (`ui.click("Run file")`) is only for labels that appear once.

`Simulator::click` is a left click at the center of the target. These Playwright gestures are not methods on `Simulator`. Build them from `simulate` or from an `.ice` line:

| Playwright | iced 0.14 |
| --- | --- |
| `click` | `Simulator::click`, or `.ice` `click "Run file"` |
| `dblclick` | two `click`s, or `mouse::Event` with click count via `simulate` |
| right-click | `simulate` a press and release of `mouse::Button::Right` after `point_at`, or `.ice` `click right "queries/edit.dql"` |
| drag | `simulate` move, press, move, release. `.ice` has `move`, `press`, and `release` |
| `fill` / `keyboard.type` | `typewrite` |
| `Enter`, `Escape` | `tap_key`, or `.ice` `type enter` / `type escape` |
| Ctrl+Enter, Ctrl+\\ | not in the `.ice` key list (enter, escape, tab, backspace only). A Rust test sends `KeyPressed` with the modifier through `simulate` |
| `expect(locator)` | `find` by id or exact text. `find` fails when the target is missing or has no visible bounds |
| `toHaveCSS` / `border-right` | no CSS. Put a widget id on the open-file row and the filename underline and assert `find` plus bounds. One snapshot covers the color |
| `screenshot` | `snapshot(&theme).matches_image(path)` |

`matches_image` writes the PNG when the file is absent and returns true. That would hide a missing baseline. The helper used in this repo checks that the file already exists, then calls `matches_image`. Baselines are committed. The snapshot file name gains the renderer (`open-file-mark-tiny-skia.png`), so a GPU renderer cannot be compared with the software one.

Pin the software renderer for tests: depend on `iced` with `tiny-skia` and `fira-sans`, and do not enable `wgpu` on the test target. `ICED_TEST_BACKEND` stays unset. `just verify` then needs no display and no GPU. The real `dqlrs --app` window still uses `wgpu`.

## Three layers

Most of today's spec is not about pixels. Keep it out of the renderer.

### 1. `Studio`, no widgets

Same list as [`plan.md`](plan.md) step 1. These are the Playwright files whose assertion is a file on disk, a lint span, a result row, or a table page:

- `files.spec.js` create, rename, move, duplicate, delete rules
- `file-watch.spec.js` dirty buffer versus a change on disk
- `lint.spec.js`
- `search.spec.js`
- `editor.spec.js` run, stop, failure, history
- `tables.spec.js` filter, refresh, 50-row pages, column order
- `save-load.spec.js`, `help.spec.js`, `nested.spec.js`, `functions.spec.js`

No socket, no `Simulator`. A failure here is an engine or workspace bug.

### 2. `Simulator`, no `Task`

The screen tests. `update` runs, and any `Task` it returns is dropped. The test feeds the message the task would have produced (`RunProgress`, `RunFinished`, `TablesLoaded`). That keeps the suite synchronous and free of DynamoDB except where the assertion is the engine, which stays in layer 1.

Cover the gestures Playwright checks on the widget tree:

- open-file mark and the filename underline (`files.spec.js` "marks the file open in the editor")
- running dot appears and clears (`files.spec.js` "pulses a blue dot")
- file context menu has Run file and does not have Run folder
- folder menu and the sidebar menu do not have Run file
- settings menu lists the shortcuts
- theme toggle changes the `Theme` value the next `view` uses
- pane split reports a new size after a drag

One light snapshot and one dark snapshot for the open-file mark and the running dot. Not a screenshot per test. Playwright's global setup deletes `.playwright-artifacts` every run. Native baselines are the opposite: they live in the crate and git, and a missing file fails.

### 3. `Emulator` and `.ice`, the real runtime

`iced_test::run(program, "tests/ice")` reads every `.ice` file and drives the real program. Presets build a temp workspace and point the session at DynamoDB Local, the same dependency `just web-test` has. The crate documents the format as experimental, so only a handful of scripts live here. The stable suite is layers 1 and 2.

```text
viewport: 1280x800
mode: Immediate
preset: Workspace
-----
click "queries/edit.dql"
click "Run file"
expect "run-status-ok"
```

`expect` checks visible text only. It does not know widget ids, modifiers, or images. Use it for "the result card shows the value", not for Ctrl+Enter or the running dot.

`mode: Immediate` runs tasks to completion before the next instruction. That is the right mode for a query. `Patient` and `Zen` are for tests that must observe an in-flight run. The running-dot test stays on the Simulator, with an injected `RunStarted`, because that is the assertion that must see the in-between frame.

Recording: the `tester` feature can record an `.ice` from a live window. Recorded scripts are a draft. Check them in only after they use a preset and do not depend on a machine path.

## What does not carry over

- A second browser context (`file-watch.spec.js` "a second window") is two `App` values on one workspace directory, or two iced windows. It is not two Chromiums.
- Title-bar profile icons exist so browser tabs and MCP can see each other. One native window does not poll to discover itself. MCP presence stays with `/mcp`, not with these tests.
- "The server went away and retries" is an HTTP failure. The window is in-process. Delete that case instead of porting it.
- Computed colors from CSS variables become theme palette values asserted in layer 2, plus the one snapshot.

## When `just verify` runs them

Add `just native-ui-test` when the `dql-studio` crate exists. It runs `cargo test -p dql-studio`. `just verify` calls it after `rust-test`. Playwright stays in `just web-test` until [`plan.md`](plan.md) step 4.

iced 0.14 sets `rust-version` to 1.88 and its crates use edition 2024. This repo's `rust-toolchain.toml` asks for stable. The native-ui crate does not compile on a toolchain older than 1.88. Raise the environment to that stable before adding the dependency. Do not vendor iced, and do not pin a git revision.
