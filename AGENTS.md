# Agent notes

## Target state is Rust

`dqlrs` is the primary tool. The target state of this repository is Rust, not Python.

Python remains for the original `dql` client and for integrations where Rust is not feasible. Do not add new `dqlrs` behavior in Python when it can be implemented in the Rust workspace (`rust-impl/`).

The web UI is part of the `dqlrs` binary (`dqlrs --web`). Its server is Rust. The browser assets live in `dqlrs-web/static/` and are bundled into the binary.
