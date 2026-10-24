# dqlrs examples workspace

Open this folder (`dqlrs-web/examples/`) as the Cursor workspace root. It is the same directory `dqlrs --web` uses for `.dql` files and matches the dqlrs MCP workspace when the web server runs from here.

Query-writer policy for this whole workspace lives in `.cursor/rules/query-writer.mdc` (write and lint DQL only; do not execute DQL for results).
