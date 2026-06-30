# AGENTS.md

## Cursor Cloud specific instructions

`dql` is a single Python CLI (the DynamoDB Query Language REPL). Dependency
management is `uv` + project tasks via `taskipy`. Standard dev commands live in
`doc/topics/develop.rst` and `pyproject.toml` (`[tool.taskipy.tasks]`); prefer
those instead of duplicating commands here.

The startup update script already installs `uv` (via `pip`, because the
`astral.sh` installer is network-blocked in this environment) and runs
`uv sync --dev`. `uv` is available on `PATH` (`~/.local/bin`) and also as
`python3 -m uv`.

Non-obvious caveats:

- Run the test suite with `TTY_COMPATIBLE=0`, e.g. `TTY_COMPATIBLE=0 uv run task test`.
  The VM's interactive shell exports `TERM=dumb`, which makes `rich` ignore the
  console width that the tests force (200) and render at width 80 with color
  codes, breaking the snapshot tests (`test_help_docs`, `test_ls`).
  `TTY_COMPATIBLE=0` makes `rich` treat output as non-interactive, matching CI.
- Tests require **DynamoDB Local** on port 8000 plus a Java runtime (Java is
  preinstalled). Start it before running tests:
  `./scripts/install_dynamodb_local.sh background` (downloads the jar into
  `.dynamo-local/` on first run, then reuses it). It is a background service, so
  it is intentionally not part of the update script.
- Running the app against DynamoDB Local: `uv run dql -H localhost -p 8000 -c "<query>"`.
  The client needs (dummy) AWS credentials, e.g. `AWS_ACCESS_KEY_ID=dummy
  AWS_SECRET_ACCESS_KEY=dummy AWS_DEFAULT_REGION=us-west-1`.
- Lint is `uv run task lint` (mypy, isort, black, pylint). It does not need
  DynamoDB Local.
