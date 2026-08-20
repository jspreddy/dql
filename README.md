# DQL

A SQL-ish language and CLI for Amazon DynamoDB. You write statements such as
`CREATE TABLE`, `INSERT`, `SELECT`, `SCAN`, `UPDATE`, and `DELETE`, and DQL
turns them into DynamoDB API calls.

## Examples

Start the shell by running `dql`. Once inside the shell you can run queries or other available commands. `help` command is available.

Example queries and commands... 

```sql
CREATE TABLE forum_threads (name STRING HASH KEY, subject STRING RANGE KEY, THROUGHPUT (4, 2));

INSERT INTO forum_threads (name, subject, views, replies) VALUES ('Self Defense', 'Defense from Banana', 67, 4);

SELECT * FROM forum_threads WHERE name = 'Self Defense';

help

help select

whoami
```

## Credentials

DQL uses the same credentials as the AWS CLI: `~/.aws/credentials` or
`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`. The default region is
`us-west-1` (override with `AWS_REGION` or `-r`). Type `help` in the REPL.

Works with aws-vault. If you are using aws-vault, you can run `aws-vault exec <my-profile-name> -- dql`

Additionally, `whoami` command is available in the interactive shell.


----



## Notes

1. **PartiQL.** Amazon ships [PartiQL for
   DynamoDB](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ql-reference.html).
   Look at that first if it already covers what you need. I do not recommend
   this as it is extremely basic.
2. **Fork.** This repository ([jspreddy/dql](https://github.com/jspreddy/dql))
   is a fork of [stevearc/dql](https://github.com/stevearc/dql). Use this repo
   for source, issues, and GitHub releases. The PyPI package `dql` and
   [Read the Docs](http://dql.readthedocs.org/) still belong to, and are
   published from, upstream.
3. **Dual implementations.** The repo contains a Python client (`dql`) and a
   Rust rewrite (`dqlrs`). Install instructions below assume the `v-next`
   branch.
4. **`v-next` Python is stable.** Python `dql` is the recommended tool. It is
   the original implementation plus additional features and bug fixes, and the
   one that matches documented behavior.
5. **`v-next` Rust is WIP.** The Rust client is catching up to Python parity.
   Try it if you want, but expect gaps / bugs.
6. You can have both `dql` (Python) and `dqlrs` (Rust) installed side by side;
   they are different binaries.

## Install instructions

These install from GitHub on the **`v-next`** branch. The Python package is in
`py-impl/`; the Rust workspace is in `rust-impl/`.

### 1. Python `dql` (recommended)

Requires Python 3.9–3.11 and [uv](https://docs.astral.sh/uv/).

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
uv tool install --python 3.9 \
  "git+https://github.com/jspreddy/dql.git@v-next#subdirectory=py-impl"
```

Run `dql` or `dql -r us-east-1`. Language docs:
[`py-docs/topics/getting_started.rst`](py-docs/topics/getting_started.rst).

### 2. Rust `dqlrs` (WIP)

Requires [Rust/Cargo](https://rustup.rs/).

```bash
cargo install --git https://github.com/jspreddy/dql.git --branch v-next \
  --locked --root ~/.local dql-cli
```

Add `~/.local/bin` to your `PATH` if needed.

More: [`rust-docs/README.md`](rust-docs/README.md).


----



## For maintainers

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
| [`manual-tests/`](manual-tests/) | Language-agnostic DQL scripts and fixtures |
| [`.github/workflows/`](.github/workflows/) | Python and Rust CI |

### Local install from a clone

```bash
git clone https://github.com/jspreddy/dql.git
cd dql
git checkout v-next
```

#### Python (editable):

```bash
cd py-impl
uv tool install --python 3.9 --editable .
```

#### Rust:

```bash
cd rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

Run local verifications: [`VERIFICATION.md`](VERIFICATION.md).
