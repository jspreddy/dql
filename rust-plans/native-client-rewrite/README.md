# Native client rewrite

This folder is the product requirements for a Rust window that replaces the iced client (`dqlrs --ui`), plus the GPUI implementation plan. The numbered requirements specify the product. They do not assume a toolkit. The plan documents at the end choose GPUI Kit.

The folder is self-contained. `rust-plans/README.md` does not index it, so the directory can move onto a branch that starts from `v-rust` without editing the other plans. The intended lift is this directory only.

## What the next client is

One window inside the `dqlrs` binary. It calls the engine in process. It is not a second install, not a browser, and not a webview around `dqlrs-web/static`. `dqlrs --web`, `/mcp`, and `dqlrs --serve` stay. The terminal REPL stays on ratatui.

The behavior to match is the web UI, locked by `dqlrs-web/playwright/tests` and by `dqlrs-web/static`. The iced window is a second source: it records the decisions made while matching that page, and the rendering bugs that a new toolkit has to avoid. Where the iced window is thinner than the page, the page wins. Those gaps are listed in [13-acceptance.md](13-acceptance.md).

[14-gpui.md](14-gpui.md) and [15-gpui-testing.md](15-gpui-testing.md) are the implementation plan. They choose GPUI Kit 0.9. The requirements above stay the product spec.

## How to read it

| Document | What it specifies |
| --- | --- |
| [01-objective.md](01-objective.md) | Who it is for, what stays, what is out of scope |
| [02-shell.md](02-shell.md) | Window regions, sizes, persistence, modes |
| [03-look.md](03-look.md) | Color, type, icons, spacing, light and dark |
| [04-files.md](04-files.md) | Tree, menus, create, rename, move, delete, watch |
| [05-editor.md](05-editor.md) | Statement bands, comments, highlight, lint, completion |
| [06-run-and-results.md](06-run-and-results.md) | Run, stop, progress, cards, history, explain, help |
| [07-tables.md](07-tables.md) | Catalog, paging, column order, cache |
| [08-search-and-settings.md](08-search-and-settings.md) | Title search and the settings card |
| [09-sheets.md](09-sheets.md) | Result structure, table structure, row JSON |
| [10-interaction.md](10-interaction.md) | Keyboard, overlays, clicks, focus |
| [11-engine.md](11-engine.md) | In-process calls, what the window must not do |
| [12-toolkit.md](12-toolkit.md) | Constraints learned from iced, for any Rust GUI |
| [13-acceptance.md](13-acceptance.md) | Oracles: Playwright specs and `dqlrs_ui` tests |
| [14-gpui.md](14-gpui.md) | GPUI Kit 0.9, how `dqlrs --ui` runs, widget map, work order |
| [15-gpui-testing.md](15-gpui-testing.md) | Headless UI tests that stand in for Playwright |

## Sources

- Web page: `dqlrs-web/static/index.html`, `app.css`, `theme.css`, `app.js`, `dql-tokens.js`, `dql-mode.js`.
- Web tests: `dqlrs-web/playwright/tests/*.spec.js`.
- Iced window: `rust-impl/crates/dql-cli/src/dqlrs_ui/`.
- Iced tests: `dqlrs_ui` in that crate (`mod.rs`, `view.rs`, `query.rs`, `highlight.rs`).
- Earlier plans, historical: `rust-plans/native-ui/`. Those still name iced as the toolkit. This folder does not.
- GPUI Kit 0.9 (`gpui-kit` on crates.io, `gpui-pre` 0.3.11). Guides: https://gpui-kit.com/docs/ and https://github.com/longbridge/gpui-kit.
