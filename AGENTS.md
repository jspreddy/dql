# Agent notes

## Target state is Rust

`dqlrs` is the primary tool. The target state of this repository is Rust, not Python.

Python remains for the original `dql` client and for integrations where Rust is not feasible. Do not add new `dqlrs` behavior in Python when it can be implemented in the Rust workspace (`rust-impl/`).

The web UI is part of the `dqlrs` binary (`dqlrs web`). Its server is Rust. The browser assets live in `dqlrs-web/static/` and are bundled into the binary.

## Web UI evidence

Do not launch sub-agents. Do not use computer use.

When the work touches the web UI, or you need to see what the web UI shows, add or extend a Playwright test under `dqlrs-web/playwright/tests/` and run `just web-test`. Save screenshots with `playwrightArtifact`, as in the Playwright screenshots rule. Those shots are written only for `just web-test -- --with-screenshots`. Use them as the evidence of what the page showed.

## Pointers for new agents

Each line says where to look, not what is there.

### Workflow

- Skills live in `.cursor/skills/`, with mirrors in `.claude/skills/`.

### Layout

- `rust-impl/`: the `dqlrs` workspace and the target state. `dql-cli` holds the CLI, REPL, `web`, MCP and notebook. The parser, engine, expressions, models and output each have their own crate.
- `dqlrs-web/`: browser assets bundled into the binary (`static/`), the bundled examples (`examples/`), and Playwright tests (`playwright/`). Design guidelines and components: `dqlrs-web/DESIGN.md`.
- `py-impl/`: the original Python `dql`. Don't add `dqlrs` behavior there.
- `notebook/`: the JupyterLab integration. `black-box-tests/`: end-to-end CLI tests.
- `rust-docs/`, `py-docs/`, `README.md`: user docs. `MAINTAINER.md` and `VERIFICATION.md`: maintainer notes.

### Tooling

- `dev-env/`: the maintainer setup script (`init.sh`) and Docker sandbox template. See `MAINTAINER.md`, "Dev environment".
- The `Justfile` is the entry point for everything: build, test, lint, DynamoDB Local (`just dynamo`), web tests, the changelog and versions.
- Many tests need DynamoDB Local. Don't run tests against it while `just verify` is running.
- `CHANGELOG.md` is generated (`just changelog`). Never edit it by hand. A bot workflow updates it on `v-rust`.
- CI is in `.github/workflows/`. Actions must be pinned to commit SHAs.
- `rust-impl/target` grows large. Run `cargo clean` if the disk fills.

### Tone in user docs

- Describe checks plainly. Don't mention reading the user's personal details, such as their git name or email.
