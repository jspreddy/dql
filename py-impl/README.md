# Python DQL implementation

Python package, tests, and tooling for DynamoDB Query Language. This tree is the
legacy client (`dql`). The recommended client is the Rust binary `dqlrs` in
[`rust-impl/`](../rust-impl/).

User and API docs: [`py-docs/`](../py-docs/) (Sphinx). Design notes:
[`py-plans/`](../py-plans/).

## Setup

Requires [uv](https://docs.astral.sh/uv/) and Java (for DynamoDB Local during tests).

```bash
cd py-impl
uv sync --dev
source .venv/bin/activate
```

Install from git (this package is not at the repository root):

```bash
uv tool install --python 3.9 "git+https://github.com/jspreddy/dql.git#subdirectory=py-impl"
```

## Tasks

```bash
uv run task --list
uv run task test
uv run task lint
uv run task dynamo    # start DynamoDB Local
```

Start DynamoDB Local before tests if the pytest plugin is not downloading it:

```bash
../scripts/install_dynamodb_local.sh background
```
