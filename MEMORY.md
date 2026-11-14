# Memory

Pointers for a new agent. Each line says where to look, not what is there.

## Rules and workflow

- Rules live in `.cursor/rules/`: branching, commits, Playwright screenshots, icons, no sub-agents. Read them first.
- `v-rust` is the target branch. Never commit to it. Work reaches it only through merged pull requests, often stacked.
- Before you commit, run `just fix`, then `just check`.
- After you open a pull request, run `just verify` through the `just-verify` skill and fix what fails before you start the next work item.
- Skills live in `.cursor/skills/`, with mirrors in `.claude/skills/`.

## Layout

- `rust-impl/`: the `dqlrs` workspace and the target state. `dql-cli` holds the CLI, REPL, `web`, MCP and notebook. The parser, engine, expressions, models and output each have their own crate.
- `dqlrs-web/`: browser assets bundled into the binary (`static/`), the bundled examples (`examples/`), and Playwright tests (`playwright/`).
- `py-impl/`: the original Python `dql`. Don't add `dqlrs` behavior there.
- `notebook/`: the JupyterLab integration. `black-box-tests/`: end-to-end CLI tests.
- `rust-docs/`, `py-docs/`, `README.md`: user docs. `MAINTAINER.md` and `VERIFICATION.md`: maintainer notes.

## Tooling

- The `Justfile` is the entry point for everything: build, test, lint, DynamoDB Local (`just dynamo`), web tests, the changelog and versions.
- Many tests need DynamoDB Local. Don't run tests against it while `just verify` is running.
- `CHANGELOG.md` is generated (`just changelog`). Never edit it by hand. A bot workflow updates it on `v-rust`.
- CI is in `.github/workflows/`. Actions must be pinned to commit SHAs.
- `rust-impl/target` grows large. Run `cargo clean` if the disk fills.

## Tone in user docs

- Describe checks plainly. Don't mention reading the user's personal details, such as their git name or email.
