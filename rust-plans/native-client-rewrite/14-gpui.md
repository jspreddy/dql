# Plan: GPUI client

**Status:** plan only. No window code on this branch.

The product is [the requirements in this folder](README.md). This document chooses the toolkit and the order of work. GPUI Kit 0.9 is the window. The page remains the oracle.

## Choice

Use the published crates, not a git checkout of Zed or Longbridge:

| Crate | Requirement |
| --- | --- |
| `gpui-kit` | `0.9` (0.9.0, 2 Nov 2026). Default features `component` and `assets`. |
| GPUI | Whatever `gpui-kit` 0.9 pins. That pin is `gpui-pre` **=0.3.11**, `gpui-pre-platform` **=0.3.11**, `gpui-pre-macros` **=0.3.11**. The Rust name is `gpui`. |

Do not add a direct `gpui` or `gpui-pre` dependency at another version. A caret on `gpui-pre` has already moved applications onto a snapshot `gpui-component` did not compile with (gpui-kit issue 3156). The kit pin is the snapshot.

Leave these features off: `gpui-webview` (no WebKit), `speech`, `tree-sitter-languages`, `syntect`, `gpui-fast` until a measured reason. `test-support` is a dev-dependency feature only.

`gpui-kit` is edition 2024. `dql-cli` stays edition 2021. A 2021 crate may depend on a 2024 crate. The repo toolchain is stable, which is new enough.

The window is GPUI, not a webview of `dqlrs-web/static`. `gpui-shell` and Wasm are out. The terminal REPL stays on ratatui.

## How it runs

Same binary, same connection flags, a window instead of a browser.

```bash
dqlrs --ui
dqlrs --ui -H localhost -p 8000 --dir ~/queries
dqlrs --ui --load-tables
just ui
```

| Flag | Same rule as |
| --- | --- |
| `-H`, `-p` | `--web`. Omit `-H` for AWS. |
| `--dir` | `--web`. Default is the current directory. |
| `--load-tables` | `--web`. Background load of every table definition. |
| `--listen` | Stays web-only. The window has no port. |
| `--mcp` | Stays web or stdio. The window does not serve `/mcp`. |

`--ui` and `--web` together are an error. `--ui` with `--serve` or `-c` is an error. `--dir` and `--load-tables` accept either `--web` or `--ui`.

`just ui` matches `just web`: build `dqlrs`, start DynamoDB Local when port 8000 is down, and open the window on `dqlrs-web/examples`. `just ui -- --aws` skips Local. `just ui -- --dir somewhere` is passed through. There is no browser and no `run.sh`.

Launch inside the process:

```rust
gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);
    gpui_kit::open_window(options, cx, |_, cx| cx.new(|cx| DqlWindow::new(cx)));
});
```

`open_window` wraps the view in `Root`, which is what makes dialogs, sheets, and menus work. Window title `dqlrs`, default size 1280×800. Preferences stay the path in [02-shell.md](02-shell.md).

The `ui` cargo feature is on by default for `dql-cli`, so a normal `cargo build -p dql-cli` contains `--ui` the way it already contains `--web`. A build with `--no-default-features` stays a CLI without the GUI crates.

## Platforms

One codebase. GPUI Kit states macOS, Windows, and Linux.

| OS | Window | Build needs |
| --- | --- | --- |
| macOS | Metal, system fonts | Xcode command-line tools. |
| Windows | The GPUI Windows backend | MSVC and the Windows SDK. An `link.exe` error means that workload is missing. |
| Linux | Vulkan, Wayland or X11 | `pkg-config`, `gcc`, `g++`, `clang`, `libfontconfig-dev`, `libwayland-dev`, `libxkbcommon-x11-dev`, `libx11-xcb-dev`, `libssl-dev`, `libzstd-dev`, `libvulkan1`. `gpui-pre-platform` is built with the `x11`, `wayland`, and `font-kit` features. |

A visible Linux window also needs a Vulkan driver in a graphical session. `libvulkan1` alone does not install a driver. Headless tests do not open that window. They use GPUI’s `TestPlatform`. `just ui-test` must pass with no `DISPLAY` and no GPU.

Do not depend on `libwebkit2gtk`. The upstream Linux install script includes it for `gpui-webview`. This client does not use that crate.

## What this branch already has

`v-rust` has the web modules and no `--ui` flag. There is no `web::studio` and no `dqlrs_ui`. The HTTP handlers in `web/server.rs` call `web::files`, `web::query_cache`, `web::lint`, `web::search`, `web::shape`, and `Session::execute_for_serve`. The first phase is a typed facade over those functions. The window and the HTTP handlers both call it. The facade returns values, not JSON.

## Module map

New code lives in `rust-impl/crates/dql-cli/src/gpui_ui/`, behind the `ui` feature. The name is not `dqlrs_ui`, which was the iced window on another branch.

| Module | Owns |
| --- | --- |
| `mod.rs` | `dqlrs --ui` entry, the `Root` window, prefs load and save. |
| `studio.rs` | Typed operations. No GPUI types. |
| `theme.rs` | Light and dark tokens from [03-look.md](03-look.md). |
| `icons.rs` | The web SVGs, drawn with GPUI `svg()` and recolored. |
| `shell.rs` | Header, mode switch, status line. |
| `files.rs` | Tree, menus, prompts, drag. |
| `editor.rs` | Buffer, bands, gutter, completion. |
| `run.rs` | Run, stop, progress. |
| `results.rs` | Tabs, cards, explain, help. |
| `tables.rs` | Catalog, page, pager. |
| `search.rs` | Title search dropdown. |
| `settings.rs` | The 232px card. |
| `sheets.rs` | Structure and row sheets. |
| `tokens.rs` | Port of `dql-tokens.js`: `queryBands`, `expandQueryBands`, `runTarget`, `statementSpans`, `bandAppearance`. |

`tokens.rs` has no GPUI dependency. Its tests are ordinary `#[test]` functions and they land before the window does. The page’s `dqlrs-web/tests/*.mjs` and `dql-tokens.js` are the oracle for that port.

## Widget map

GPUI Kit already has the behaviors the iced window had to build by hand. Use them. Paint the product colors on top. The stock Longbridge theme is not the look in [03-look.md](03-look.md).

| Product | Widget | Notes |
| --- | --- | --- |
| Header buttons, Run, Stop, pager | `Button` | Labels vertically centered. Run file uses the danger color. |
| Mode group | `Tabs` or two buttons in one frame | Selected state is teal `#0f766e`. |
| Search, catalog filter, prompts | `Input` | Single line. Placeholder is not the accessible name. |
| Settings checks | `Checkbox` or `Switch` | Defaults from [08-search-and-settings.md](08-search-and-settings.md). |
| File tree | `Tree` / `TreeState` | Collapse and expand. Key rows by path. |
| File menu, search groups | `Menu` or `Popover` | Float. Do not insert them into the column. Anchor at the pointer or under the field. Submenus open to the left only when the right side has no room. |
| Editor | `Editor` / `EditorState` | Rope, caret, selection, gutter, diagnostics, decorations. Not `Textarea` and not `Input`. |
| Statement frame | Editor decoration or overlay | Blue border, 5px outside the text, run-fill wash. One frame. |
| Gutter play, lint, `✓` / `×` | `GutterDecorationCollection` | Scrolls with the text. The play circle is hidden when the frame is off screen, and while a sheet, menu, search list, or settings card is open. |
| Lint squiggle | Editor diagnostics | A byte range, not a line-only color. |
| Completion | Editor completion provider | No LSP process. The provider uses the site rules in [05-editor.md](05-editor.md). |
| Highlight | Highlighter interface on `EditorState` | A port of the page tokens. Not `tree-sitter-sql` and not syntect’s SQL grammar. Keywords bold. Write words `#dc2626`. Strings `#9f1239`. |
| Write bands | Line decorations | Washes and the 3px gutter bar from [03-look.md](03-look.md). |
| Side and results splits | `ResizableState` | Two splits. Drag, keyboard nudge, double-click reset, persist pixels. No close, maximize, or extra docks. `DockArea` is the fallback only if `ResizableState` cannot persist the two ratios. |
| Result and table grids | `DataTable` | Sticky header, fixed peek column, both-axis scroll, column resize from the drag origin. Virtualization may be on. Pages stay at 50 rows. |
| Row eye, schema icon | Buttons in cells | Accessible names **Show row JSON**, **Show result structure**, **Show table structure**. |
| Progress | `Progress` | Shown while `total` is known. |
| Toasts | `Toast` | Save, conflict, run failure. Status line stays. |
| Structure and row sheets | Focus-trapped overlay sized to the window | `Dialog` and `Sheet` supply escape, backdrop, and focus trap. The default centered dialog is the wrong size. Radius 0, card fills the window, as in [09-sheets.md](09-sheets.md). |
| Settings and environment | `Popover`, 232px | Right-aligned under the settings button, 6px below it. |
| Help body | Markdown renderer only if it matches the help card | Otherwise a column of heading, code, and prose blocks. Do not dump JSON. |
| Icons | `svg()` plus `set_icon_resolver` | Symbolic SVGs from the page, recolored with the current ink. They are children of the control that owns them. |

## Rendering constraints from the iced window

GPUI paints through the GPU (Metal, DirectX, Vulkan), so it does not have the tiny-skia damage bug. The rule still holds: an icon is an element in the tree of its control. A decoration that is not in that tree will not move or clear when the control does. Overlays use the kit’s popup and dialog layers, which paint above the window and capture the pointer. The gutter play control is omitted from the tree while a covering surface is open, so a lower layer cannot take the click.

Lists are keyed by path, table name, or statement index. GPUI entities keep their own state. Do not rebuild a row’s entity under another row’s id.

## Work order

Each phase leaves `cargo test -p dql-cli` green and adds the headless tests in [15-gpui-testing.md](15-gpui-testing.md) for the behavior it adds.

1. **Facade and flag.** `studio.rs`. `--ui` parses and rejects the illegal combinations. `just ui` opens a window that shows the workspace path and the endpoint label. One headless test finds the wordmark **dqlrs**.
2. **Tokens.** Port `queryBands`, `expandQueryBands`, `runTarget`, `statementSpans`, and `bandAppearance`. Tests match the page examples in [05-editor.md](05-editor.md), including the `read.dql` comment and blank-line case.
3. **Shell.** Header, Query and Tables modes, light and dark theme, persisted splits at the page defaults (220px side, 46% results).
4. **Files.** Tree, menus, create, rename, duplicate, move, drag, delete, the `.gitkeep` refusal, autosave, patch dialog.
5. **Editor.** Open a file, highlighter, bands, frame, gutter play, chords Ctrl+Enter, Ctrl+\\, Ctrl+S.
6. **Run and results.** Run, Run file, Stop, progress, Result / History / Lint tabs, cards, capacity and time pills, scroll the failing statement into view.
7. **Tables.** Catalog, refresh, cache age, 50-row pages, sticky header and peek column.
8. **Search and settings.** 150ms debounce, groups, Escape keeps the text, the 232px card.
9. **Sheets.** Result structure, table structure, row JSON in column order.
10. **The rest of the page.** Lint squiggles, completion, explain, help cards, describe, intelligent match, directory watch.

`dqlrs --web` stays. Playwright stays until phase 10’s headless suite covers [13-acceptance.md](13-acceptance.md).

## Risks

| Risk | What to do |
| --- | --- |
| `gpui-pre` API moves between 0.9 patch releases | Keep `gpui-kit = "0.9"` and commit `Cargo.lock`. Do not bump the pin by hand. |
| `Editor` highlighter cannot take per-span bold and a second color | Confirm in the phase 5 spike. If the interface is line-only, stop and extend the highlighter. A line-only color is the iced gap and is not acceptable. |
| `DataTable` cannot pin the peek column and the header together | Use its fixed-column support. If the peek column scrolls away, the table is wrong. |
| `Dialog` cannot fill the window | Build the sheet on the focus-trap and backdrop primitives, sized to the window bounds. |
| Linux CI has no Vulkan driver | Headless tests must not need one. The build still needs the dev packages listed above. |
| Binary size and compile time | Expected. The feature is default so `--ui` is present. Do not pull tree-sitter languages or WebKit to “save” that cost. |
| In-app drag is not OS file-drop | The requirement is dragging a tree row onto a folder or the tree root. That is an in-window drag. |
