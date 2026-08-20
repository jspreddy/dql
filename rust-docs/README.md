# DQL Rust client (`dqlrs`)

[![Rust CI](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/rust-workflows.yml)

User guide for the Rust implementation of DynamoDB Query Language. The CLI
binary is `dqlrs`. Source lives in [`rust-impl/`](../rust-impl/). Maintainer
notes are in [`rust-impl/README.md`](../rust-impl/README.md). Differences from
the Python client: [migration-from-python.md](migration-from-python.md).

This repository ([jspreddy/dql](https://github.com/jspreddy/dql)) is a fork of
[stevearc/dql](https://github.com/stevearc/dql).

## Install

Download a prebuilt binary from
[GitHub releases](https://github.com/jspreddy/dql/releases):

```bash
curl -fsSL https://raw.githubusercontent.com/jspreddy/dql/HEAD/scripts/rust-install.sh | sh
```

**Local release build:**

```bash
cd rust-impl
cargo build --release -p dql-cli
install -m 0755 target/release/dqlrs ~/.local/bin/dqlrs
```

**From a git checkout** (the Cargo workspace is not at the repository root):

```bash
git clone https://github.com/jspreddy/dql.git
cd dql/rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

The binary is `~/.local/bin/dqlrs`. Add that directory to your `PATH` if needed.

Override the GitHub repo with `DQL_REPO=owner/repo` when using the install
script.

## Connect

| Mode | How |
| --- | --- |
| Live AWS | `dqlrs` (default when `-H` is unset; uses the AWS SDK credential chain) |
| DynamoDB Local | `dqlrs -H localhost -p 8000` |
| In-memory (offline) | `DQL_BACKEND=memory dqlrs …` |

Region defaults to `AWS_REGION`, else `us-west-1`. Override with `-r`.

Config and history paths: `~/.config/dql.json` and `~/.dql/history`.

You can use `$HOME/.aws/credentials` or `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`.

## Flags

```text
-c, --command <command>  Run this command and exit
-r, --region <region>    AWS region
-H, --host <host>        Local DynamoDB host
-p, --port <port>        Local DynamoDB port (default 8000)
    --json               With -c, print results as JSON
    --version            Print version and exit
-h, --help               Print help
```

## Statements and meta-commands

DQL statements: `SELECT`, `SCAN`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`,
`DROP`, `ALTER`, `DUMP`, `LOAD`, `EXPLAIN`, `ANALYZE`.

In the REPL, type `help` or `help <statement>`. Meta-commands include `opt`,
`ls`, `use`, `local`, `file`, `throttle`, `unthrottle`, `whoami`, `shell`,
`clear`, `exit`, and `version`. Build with `--features watch` to enable the
`watch` CloudWatch dashboard.

Output formats (via `opt format`): `smart`, `column`, `expanded`, `json`,
`rich`. Non-TUI paths can page with `opt display less`.

## SAVE / LOAD

| Format | Extensions |
| --- | --- |
| JSON lines | `.json`, `.json.gz` |
| CSV | `.csv`, `.csv.gz` |
| MessagePack | `.msgpack`, `.msgpack.gz` (default binary) |

Legacy pickle (`.p` / `.pkl` / `.pickle`) is rejected; re-export from Python as
JSON or CSV first if needed.

## Quick examples

```bash
dqlrs --version
dqlrs -c "ls"
dqlrs --json -c "SCAN * FROM mytable LIMIT 5"
dqlrs -H localhost -p 8000 -c "CREATE TABLE t (id STRING HASH KEY); SCAN * FROM t"
DQL_BACKEND=memory dqlrs -c "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t"
```

REPL examples:

```text
us-west-1> CREATE TABLE forum_threads (name STRING HASH KEY,
         >                             subject STRING RANGE KEY,
         >                             THROUGHPUT (4, 2));
us-west-1> INSERT INTO forum_threads (name, subject, views, replies)
         > VALUES ('Self Defense', 'Defense from Banana', 67, 4);
us-west-1> SCAN * FROM forum_threads;
us-west-1> SELECT * FROM forum_threads WHERE name = 'Self Defense';
```

Use `help` in the REPL for statement details.
