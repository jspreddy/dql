# dqlrs examples workspace

Open this folder (`dqlrs-web/examples/`) as the Cursor workspace root. It is the same directory `dqlrs --web` uses for `.dql` files and matches the dqlrs MCP workspace when the web server runs from here.

The agent writes queries only and never runs them (`.cursor/rules/query-writer.mdc`). It may use MCP metadata for linting. Do not infer data from `.dql` files (`.cursor/rules/live-data.mdc`).
