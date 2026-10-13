# Maintainer notes

[![Python CI](https://github.com/jspreddy/dql/actions/workflows/python-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/python-workflows.yml)

[![Rust CI](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml)

| Path | Contents |
| --- | --- |
| [`py-impl/`](py-impl/) | Python package, tests, packaging, tooling |
| [`py-docs/`](py-docs/) | Sphinx docs for the Python client |
| [`py-plans/`](py-plans/) | Python-only design notes |
| [`rust-impl/`](rust-impl/) | Rust Cargo workspace (`dqlrs`) |
| [`rust-docs/`](rust-docs/) | Markdown user docs for the Rust client |
| [`rust-plans/`](rust-plans/) | Rust rewrite / architecture notes |
| [`black-box-tests/`](black-box-tests/) | Black-box acceptance cases (binaries + DynamoDB Local) |
| [`notebook/`](notebook/) | JupyterLab UI and DQL kernels |
| [`dqlrs-web/`](dqlrs-web/) | Web UI assets and Playwright tests |
| [`docs/images/`](docs/images/) | README screenshots |
| [`scripts/`](scripts/) | Shared installers, version bump, helpers |
| [`.github/workflows/`](.github/workflows/) | Python and Rust CI |

## Local install from a clone

```bash
git clone https://github.com/jspreddy/dql.git
cd dql
git checkout v-rust
```

Python (editable):

```bash
cd py-impl
uv tool install --python 3.9 --editable .
```

Rust:

```bash
cd rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

## Verification

[`VERIFICATION.md`](VERIFICATION.md). From the repo root,
[`just`](https://just.systems/) lists tasks; `just verify` runs the full check.

Notebook from a checkout: `./notebook/start.sh --local --start-local`.
Details: [`notebook/README.md`](notebook/README.md).
