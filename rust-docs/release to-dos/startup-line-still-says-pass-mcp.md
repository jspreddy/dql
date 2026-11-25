# The startup line still says MCP is only available with `--mcp`

Status: open. Found on `0.7.12-alpha.1` in the MCP pill from #304.

## What goes wrong

`dqlrs web` without `--mcp` prints that MCP is off and that the way to serve it is to pass `--mcp`. The header pill can turn `/mcp` on without a restart. The printed line is now wrong, and it is the first thing the terminal shows.

## Why

`serve` in `rust-impl/crates/dql-cli/src/web/server.rs` still has the old branch. When `mcp` is false it prints “off. Pass `--mcp` to serve `http://<bound>/mcp`”. The `--mcp` flag only sets the starting value of `App.mcp`. `POST /api/mcp` changes it later. `rust-docs/README.md` already describes the pill. This print was not updated with it.

When the process is started with `--mcp`, the on branch is still right: it prints the address and the client config.

## What to change

When MCP starts off, say that it is off and that the MCP pill in the header turns it on. Keep the bound `/mcp` address in that line so the operator can see where it will answer. Do not tell them that `--mcp` is required.

Check the text from a `dqlrs web` start with and without `--mcp`. The on path should keep printing the Cursor and Claude config.
