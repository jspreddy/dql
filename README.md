# DQL

[![Python CI](https://github.com/jspreddy/dql/actions/workflows/python-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/python-workflows.yml)
[![Rust CI](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml)

A SQL-ish language for DynamoDB.

This repository ([jspreddy/dql](https://github.com/jspreddy/dql)) is a **fork**
of [stevearc/dql](https://github.com/stevearc/dql). Use this repo for clone,
install-from-source, and GitHub release URLs. The PyPI package and Read the Docs
site still belong to upstream.

Amazon has also released
[PartiQL](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ql-reference.html)
for DynamoDB; consider that first if it fits your needs.

## Implementations

| Tree | Role |
| --- | --- |
| [`rust-impl/`](rust-impl/) | Rust workspace; builds the `dqlrs` CLI (recommended) |
| [`rust-docs/`](rust-docs/) | Rust user guide and [Python → Rust migration](rust-docs/migration-from-python.md) |
| [`rust-plans/`](rust-plans/) | Rust rewrite / architecture notes |
| [`py-impl/`](py-impl/) | Legacy Python package (`dql`) |
| [`py-docs/`](py-docs/) | Python Sphinx docs |
| [`py-plans/`](py-plans/) | Python-only design notes |

[`manual-tests/`](manual-tests/) holds language-agnostic DQL scripts and fixtures.

## Quick start (Rust)

See [`rust-docs/README.md`](rust-docs/README.md).

```bash
curl -fsSL https://raw.githubusercontent.com/jspreddy/dql/HEAD/rust-impl/scripts/install-rust.sh | sh
# or
git clone https://github.com/jspreddy/dql.git
cd dql/rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

## Quick start (Python)

See [`py-impl/README.md`](py-impl/README.md) and [`py-docs/`](py-docs/).

```bash
uv tool install --python 3.9 "git+https://github.com/jspreddy/dql.git#subdirectory=py-impl"
```
