# Objective

## Product

A person opens a workspace of `.dql` files, edits them, runs statements against DynamoDB (or the memory backend), and browses tables. Today that person uses either `dqlrs --web` or `dqlrs --ui`. The web UI is the behavioral oracle. The iced window proved the same features can live in the `dqlrs` process, and it also proved which toolkit limits show up as visible bugs.

The rewrite keeps the product and changes the window toolkit.

## Binary shape

- The window is a flag on `dqlrs` (`--ui` or its successor). It is not a separate package.
- The window and the engine share a process. File open, lint, run, table list, and search are Rust function calls into the same session the web server already uses (`web::studio` and the pieces under it).
- There is no bearer token on the window path. The process is the user.
- Theme, sidebar width, results height, and highlight toggles persist in the app config (`ui.json` for the iced window: `DQLRS_UI_PREFS`, else the platform config dir, else `~/.config/dqlrs/ui.json`). They are not `localStorage`.

## Who it is for

The person at the machine who edits and runs `.dql`. Agents that talk to `/mcp` are not this window. A notebook worker (`dqlrs --serve`) is not this window.

## Stays as it is

- DQL syntax, the engine, and the terminal REPL.
- `dqlrs --web` until a later cutover. This rewrite does not delete the page.
- `/mcp` and `dqlrs --serve`.
- SAVE and LOAD as statements. Paths resolve from the process working directory, not from the workspace `--dir`. `CREATE PARENTS` creates missing directories. A missing parent reports the OS error (`failed to create '…': No such file or directory (os error 2)`).

## Out of scope

- A new query language or a smaller feature set than the page.
- Embedding `dqlrs-web/static` in a webview.
- Putting the UI in Wasm.
- A hidden HTTP server whose only client is the window.
- Multi-window presence. The page shows Web and MCP clients because several browser tabs share one server. One native window does not poll to discover itself. MCP presence is optional and only matters if `/mcp` is running beside the window.
- “The server went away.” That dialog exists because the page can lose its process. The window is the process. A failed engine call is an error in the window, not a reconnect screen.
- Replacing ratatui.

## Done when

- The window covers the behaviors in [13-acceptance.md](13-acceptance.md) against the same engine the page uses, with no HTTP call on those paths.
- Light and dark match the tokens in [03-look.md](03-look.md).
- The rendering constraints in [12-toolkit.md](12-toolkit.md) hold on a software presenter, not only on a backend that clears the full frame.
