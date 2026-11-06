# Plan: headless UI tests

The suite plays the role Playwright plays for `dqlrs --web`. It builds the real GPUI view, sends pointer and keyboard input, and asserts what a person would see. It does not launch Chrome, a display, or a GPU.

## Harness

GPUI Kit 0.9 documents this as UI integration testing. `test-support` turns on GPUI’s test harness and the kit helpers.

```toml
[dev-dependencies]
gpui-kit = { version = "0.9", features = ["test-support"] }
```

That feature stays out of the release build. Tests use `#[gpui_kit::test]` and `gpui_kit::test::TestWindowExt`.

A test opens the same `DqlWindow` the binary opens, against a temporary workspace and `DQL_BACKEND=memory`, then:

```rust
window.render_frame(cx);
window.click("Run selection", cx);
window.input("SELECT 1;", cx);
window.press("backspace", cx);
```

`render_frame` paints one frame. `click`, `input`, and `press` target the accessible name. `find` returns an `ElementSnapshot` for that frame: `role()`, `label()`, `value()`, `focused()`, `checked()`, `selected()`, `expanded()`, and geometry. A snapshot is stale after the next interaction. Query again. `None` means the property was not exposed, not that it is false.

Every interactive control gets a stable element id and an accessible name from the requirements. The name is the visible label or the tooltip the page already uses: **Run selection**, **Show row JSON**, **Clear results**, **New file**. A placeholder is not a name. Custom nodes that are not kit widgets call `.test_support()` so `find` can see them when the feature is on. In a normal build that call returns the element unchanged.

Assert the model as well as the label. A test that only checks a string can pass when the click did nothing and the label was hardcoded.

`#[gpui::test]` and `TestAppContext` remain available for entity tests that do not need a window. Prefer `#[gpui_kit::test]` once the assertion is about a control. Pure token and studio tests stay ordinary `#[test]` and do not link the harness.

The executor is single-threaded and deterministic. Async run progress uses the test background executor and `run_until_parked` so a deferred callback actually runs. Do not add retries to hide a race.

## Layout of the suite

Mirror the Playwright files. One module per spec, fixtures copied from `dqlrs-web/playwright/fixtures` (the `empty/` directory with `.gitkeep`, `queries/edit.dql`, `queries/read.dql`, `queries/functions.dql`).

| Module | Covers | Playwright source |
| --- | --- | --- |
| `tokens.rs` | Bands, selection, statement spans, highlight matrix | `dql-tokens.js`, `web-unit` |
| `files.rs` | Tree, menus, resize, create, rename, drag, delete, watch | `files.spec.js`, `file-watch.spec.js` |
| `editor.rs` | Frame, comments, chords, padding, completion | `editor.spec.js` |
| `run.rs` | Run, Run file, Stop, progress, gutter marks, scroll to error | `editor.spec.js` |
| `results.rs` | Tabs, cards, capacity, explain, help, history pin | `editor.spec.js`, `help.spec.js`, `nested.spec.js` |
| `tables.rs` | Catalog, cache age, pager, sticky columns, structure sheet | `tables.spec.js` |
| `search.rs` | Groups, jump, **No matches**, Escape keeps the text | `search.spec.js` |
| `settings.rs` | Toggles, theme, shortcuts, clear cache | `editor.spec.js`, `tables.spec.js` |
| `sheets.rs` | Both structure sheets and the row sheet | `tables.spec.js`, `editor.spec.js` |
| `lint.rs` | Squiggle, chip, Lint tab | `lint.spec.js` |
| `save_load.rs` | SAVE and LOAD paths | `save-load.spec.js` |

`save_load.rs` and the sealed query-cache checks can run on the studio with no window. The UI tests call the studio only through the window.

Presence and **The server went away** are not ported. See [01-objective.md](01-objective.md).

## What a test is allowed to know

| Assert | Example |
| --- | --- |
| A control exists by name | `window.find("Files")` |
| A value or label | Result meta **Nothing to run**, row title **id = a** |
| Checked, selected, expanded | Even/odd starts checked. A directory click flips expanded. |
| Focus | After a sheet opens, focus is on the close control. |
| Geometry when the requirement is spatial | Settings card is 232px wide. Play circle’s left edge is left of the run frame. Side width restores within 3px. |
| Model | The workspace file matches the buffer. The run text includes the leading comment. JSON key order is `id`, `sku`, `name`. |

Do not assert a screenshot as the pass condition. A few geometry checks replace the Playwright PNGs. A missing accessible name is a product bug, same as a missing `data-testid`.

## Commands

| Recipe | What it does |
| --- | --- |
| `just ui-test` | `cargo test -p dql-cli --features ui --lib gpui_ui::` with `test-support` from dev-dependencies. No DynamoDB, no browser. |
| `just ui-test files` | The same, filtered to one module, the way `just web-test -- tests/files.spec.js` filters Playwright. |
| `just ui` | The real window. See [14-gpui.md](14-gpui.md). |

`just verify` gains `just ui-test` next to `just web-test`. `web-test` stays until the modules above cover [13-acceptance.md](13-acceptance.md). `ui-test` does not start DynamoDB Local. Cases that need a real engine use the memory backend. `functions.spec.js` and `nested.spec.js` move to memory-backend studio tests first. If a case truly needs DynamoDB Local, it is marked and `just ui-test` skips it unless `just dynamo` has already been run.

## First tests, in order

These land with the phases in [14-gpui.md](14-gpui.md). They are the acceptance bar for each phase, not a backlog to write up front.

1. The window shows **dqlrs**, **Query**, and **Tables**. Clicking **Tables** shows **No file open** and hides **New file**.
2. Side drag persists and double-click returns to about 220px. Results double-click returns to 46%.
3. Opening `queries/edit.dql` marks that row and the file name. A second file takes the mark away from the first.
4. Root menu is only **New file** and **New folder**. A file menu contains **Run file**, **Move to…**, and **Clear results**, and does not contain **Run folder**.
5. Rename Escape cancels. Delete of `empty/` is refused because of `.gitkeep`.
6. Cursor in `WHERE id = 'a'` frames the header comment and not the following `SCAN`. A non-whitespace selection wins.
7. Ctrl+Enter on a missing table shows an error and **Run selection** is enabled again. Stop yields **Query stopped**.
8. Run file on the fixture leaves one card on Result and the earlier statements on History.
9. Cache age strings from **not cached** through **2 days ago**. **Clear table cache** leaves **Table cache cleared.**
10. Search **No matches**. Escape closes the list and keeps the text. A query hit opens the statement.
11. Result structure shows **Result structure**, the info name **Based on this query result, not the entire table.**, and the footer **No extra query was run.** Table structure hides the info button and the query. Row title is **id = a**.
12. While the sheet is open, **Run selection**’s gutter button is absent. After Escape it is back, and it is not painted in the sheet’s corner.

## Platform matrix

The headless suite is the same on macOS, Windows, and Linux. CI on Linux installs the build packages from [14-gpui.md](14-gpui.md) and does not set `DISPLAY`. A failure to open a Vulkan device inside a `#[gpui_kit::test]` is a harness bug. A failure to open a Vulkan device from `just ui` on a machine with no driver is an environment limit, and the test suite does not cover it.

One smoke on each OS, not in `just ui-test`: `dqlrs --ui --dir <fixture>` shows the tree. That is a manual or nightly check. The headless suite is the merge bar.
