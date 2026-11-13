# DQL Rust client (`dqlrs`)

[![Check](https://github.com/jspreddy/dql/actions/workflows/check.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/check.yml)

User guide for the Rust implementation of DynamoDB Query Language. The CLI
binary is `dqlrs`. Source lives in [`rust-impl/`](../rust-impl/). Maintainer
notes are in [`rust-impl/README.md`](../rust-impl/README.md).

This repository ([jspreddy/dql](https://github.com/jspreddy/dql)) is a fork of
[stevearc/dql](https://github.com/stevearc/dql).

## Documentation map

| Where | What |
| --- | --- |
| This file | Install, connect, flags, statements, web UI, `--serve`, notebook |
| [migration-from-python.md](migration-from-python.md) | Differences from the Python client |
| [architecture.html](architecture.html) | Crate, module, and runtime diagrams |
| [packaging.html](packaging.html) | How `dqlrs` is packaged, installed, and run (web assets included) |
| [notebook.html](notebook.html) | How `dqlrs notebook` installs, configures, and launches kernels |
| [`metrics/`](metrics/) | How long the rewrite took and what it cost: [by day](metrics/rewrite-metrics.html), [by hour](metrics/rewrite-metrics-hours.html), [Plotly charts](metrics/work-metrics.html) |
| [`plans/`](plans/README.md) | Architecture notes, testing strategy, open design debt, and the archive of finished plans (including the security design) |

## Install

There are no prebuilt binaries or GitHub releases yet. Build from source with
[Rust/Cargo](https://rustup.rs/).

**From the git URL** (no checkout needed):

```bash
cargo install --git https://github.com/jspreddy/dql.git --branch v-rust \
  --locked --root ~/.local dql-cli
```

**Local release build:**

```bash
cd rust-impl
cargo build --release -p dql-cli
install -m 0755 target/release/dqlrs ~/.local/bin/dqlrs
```

**From a git checkout** on the **`v-rust`** branch (the Cargo workspace is not
at the repository root):

```bash
git clone https://github.com/jspreddy/dql.git
cd dql
git checkout v-rust
cd rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

The binary is `~/.local/bin/dqlrs`. Add that directory to your `PATH` if needed.

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

## Usage and flags

```text
dqlrs [options]          Start the REPL (or run -c)
dqlrs web [options]      Serve the query editor and table browser
dqlrs web --init-examples
                         Start a new workspace in an empty folder from the examples
dqlrs notebook           Start JupyterLab with a DQL (Rust) kernel (this binary)

-c, --command <command>  Run this command and exit
-r, --region <region>    AWS region
-H, --host <host>        Local DynamoDB host
-p, --port <port>        Local DynamoDB port (default 8000)
    --json               With -c, print results as JSON
    --serve              Headless JSON-lines worker (stdio, or --bind on loopback)
    --bind <ADDR>        With --serve, listen on loopback HOST:PORT (or PORT as 127.0.0.1:PORT)
    --mcp                MCP tools. Alone, stdio. With web, HTTP at /mcp
    --listen <ADDR>      With web, listen on loopback HOST:PORT (default 127.0.0.1:8765)
    --dir <DIR>          With web or --mcp, folder of .dql files (default: current directory)
    --load-tables        With web, load every table definition (with progress)
                         before the server starts
    --init-examples      With web, after a confirm: git init an empty folder, copy the
                         examples in, and commit them
    --yes                With web --init-examples, skip the confirmation
    --version            Print the version and exit
    -h, --help           Print help
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

## Web UI (`dqlrs web`)

`dqlrs web` serves the query editor and table browser on `127.0.0.1:8765`.
The server runs inside this binary. Browser assets are bundled with it.
`-H` and `-p` still choose DynamoDB Local; omit `-H` to use AWS.

```bash
dqlrs web -H localhost -p 8000
dqlrs web --listen 127.0.0.1:9000 --dir ~/queries
dqlrs web --init-examples --dir ~/dql-examples
dqlrs web --load-tables -H localhost -p 8000
```

`--load-tables` loads the table list and every table definition before the
server starts. In a terminal the first line names the connection (on AWS, the
region, account, and the role or user from STS) and the second shows a progress
bar and a count, then how many definitions loaded and how long it took. Only a table whose definition fails to
load gets its own line, a red cross with the error; the server starts either way.

`--init-examples` sets up an empty (or missing) folder before serving it. It
checks that the folder is empty and that git is installed, prints the plan, and
asks before it changes anything (`--yes` skips the question; without a terminal
it is required). Then it runs `git init`, copies `dqlrs-web/examples` (bundled
into the binary without its ignored caches), and commits it as "Initialized
dqlrs web examples".

From a checkout, `just web` builds `dqlrs`, starts DynamoDB Local when port 8000
is down, and opens the UI on `dqlrs-web/examples`. Details:
[`dqlrs-web/README.md`](../dqlrs-web/README.md).

## Headless worker (`--serve`)

`dqlrs --serve` keeps one `Session` and speaks **JSON-lines** (one JSON object
per line). It is not a TTY REPL: no ratatui, no pager, no `~/.dql_history`.
`--json` is implied. Mutually exclusive with `-c`.

```bash
# stdio (tests, embedding)
DQL_BACKEND=memory dqlrs --serve

# loopback TCP (notebook kernels and other tools)
dqlrs --serve --bind 127.0.0.1:7400
dqlrs --serve --bind 127.0.0.1:0    # prints `dqlrs serve listen 127.0.0.1:<port>` on stderr
dqlrs --serve --bind 7400           # same as 127.0.0.1:7400
```

`--bind` requires `--serve` and **refuses non-loopback** addresses (`0.0.0.0`,
public IPs) before listen. One TCP client at a time; extra connections are
dropped with `dqlrs serve refuse: already connected` on stderr. Disconnecting
keeps the process and session (reconnect is allowed). `{"op":"shutdown"}` exits
0.

Client ops: `exec` (DQL or meta text), `ping`, `shutdown`, `interrupt` (no-op
in v1). Each reply is one envelope:

```json
{"id":"1","ok":true,"kind":"items","items":[{"id":"a"}],"affected":null,"message":null,"partial":false}
```

During `exec`, the worker may emit **progress** lines *before* the envelope
(no `ok` field — older clients skip them):

```json
{"id":"1","event":"progress","done":25,"total":1000,"phase":"write"}
```

`phase` is `write` (INSERT / LOAD / UPDATE / DELETE) or `read` (paged SCAN /
SELECT). INSERT/LOAD emit at least `0`, each 25-item chunk, and the final count.

`kind` is `none` / `items` / `affected` / `status` / `schema` / `text`. Errors
use `"ok": false`, `"kind": "error"`, and `"error": {"code":"...","message":"..."}`
with `code` of `parse` | `runtime` | `unsupported` | `protocol`. The process
stays up.

Unsupported meta (envelope `unsupported`, no crash): `watch`, `clear` / `cls` /
`c`, `exit` / `quit` (use `op: shutdown`), `shell`.

This is **Rust-only**. Python `dql` has no `--serve`; notebook Python cells set
`DQL_PROGRESS_JSON=1` so `dql -c` prints the same progress events on stderr.
`-c --json` stays concatenated item objects (not this envelope).

## Notebook (`dqlrs notebook`)

How install, configuration, and kernel launch fit together:
[notebook.html](notebook.html).

After `cargo install`, the same binary can start JupyterLab:

```bash
dqlrs notebook --local --no-browser
# same:
dqlrs --notebook --local
# from another terminal:
dqlrs notebook --shutdown
```

Ctrl-C in the terminal that started Lab shuts the server down. `--shutdown`
stops a notebook server this command left running. If several are running, it
lists them and asks which ones to stop.

This does **not** embed Jupyter (too large). On first run it creates
`~/.local/share/dqlrs/notebook` (override with `DQLRS_NOTEBOOK_HOME`), installs
JupyterLab + ipykernel into a venv (via `uv` or `python3 -m venv`), writes a
**DQL (Rust)** kernelspec with `DQLRS_BIN` set to this executable, and launches
Lab on `127.0.0.1`. Later runs reuse the venv.

Needs Python 3.10+ or [uv](https://docs.astral.sh/uv/). `--local` points cells
at DynamoDB Local (`localhost:8000`) and sets dummy AWS keys if they are unset.
Start Local yourself; this command does not bundle it.

Repo checkout alternative (Python + Rust kernels): [`../notebook/README.md`](../notebook/README.md)
`./notebook/start.sh`.

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
printf '%s\n' '{"op":"ping"}' '{"op":"shutdown"}' | DQL_BACKEND=memory dqlrs --serve
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
