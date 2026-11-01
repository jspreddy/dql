# Native Rust UI

Investigation only. No UI code in this set.

The browser app in `dqlrs-web/static` can be replaced by a desktop window written in Rust. The window would call the same session the web server uses today, in process, with no HTTP hop between the screen and the engine.

| Document | What it decides |
| --- | --- |
| [`prd.md`](prd.md) | Same features, what happens to the HTTP API, how tests move |
| [`plan.md`](plan.md) | Steps: a shared core first, then an egui window, then delete the JS app |

Ratatui stays the terminal REPL. It is not this UI. A webview around the current page is not this UI either.
