# Native Rust UI

`dqlrs --ui` opens the iced window inside the `dqlrs` binary. The window calls `web::studio` in process. There is no second install and no HTTP hop. `dqlrs --web` stays.

| Document | What it decides |
| --- | --- |
| [`prd.md`](prd.md) | Same features, what happens to the HTTP API, iced as the window |
| [`plan.md`](plan.md) | Studio facade, then the iced window. The web UI stays. |
| [`testing.md`](testing.md) | `Simulator` and a few `.ice` scripts in place of Playwright |
| [`refinement.md`](refinement.md) | iced 0.14 widgets to take for look and interaction |

Ratatui stays the terminal REPL. It is not this UI. A webview around the current page is not this UI either. The window is iced 0.14, and it needs rustc 1.88 before any crate depends on it.
