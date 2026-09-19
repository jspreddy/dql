# Notebook frontend

JupyterLab configured for this repo so you can run **DQL in notebook cells** against either implementation:

| Launcher tile | Cell language | Backend |
| --- | --- | --- |
| **DQL (Python)** | DQL | `dql -c` |
| **DQL (Rust)** | DQL | `dqlrs -c` |
| **Python (dql)** | Python | in-process `from dql import Engine`, plus `%%dql` / `%%dqlrs` magics |

Design notes: [`PLAN.md`](PLAN.md).

## Start from the CLI

Requires [uv](https://docs.astral.sh/uv/). The start script installs Python 3.11, JupyterLab, and project-local kernels (nothing is written to `~/.local/share/jupyter`).

```bash
# From the repository root
./notebook/start.sh
# same:
./scripts/notebook.sh
```

Useful flags:

```bash
./notebook/start.sh --local --start-local   # DynamoDB Local + dummy AWS keys
./notebook/start.sh --no-browser
./notebook/start.sh --dry-run               # sync, register kernels, list, exit
```

`aws-vault` works the same as the CLI:

```bash
aws-vault exec my-profile -- ./notebook/start.sh
```

### Binaries

The DQL kernels call the **installed programs**, not crate internals.

- `DQL_BIN` or `dql` on `PATH`
- `DQLRS_BIN` or `dqlrs` on `PATH`

Lab still starts if one is missing; cells for that kernel fail with an install hint.

From a clone:

```bash
cd py-impl && uv tool install --python 3.9 --editable .
cd rust-impl && cargo install --path crates/dql-cli --locked --root ~/.local
```

### Connection

| Variable | Default | Purpose |
| --- | --- | --- |
| `AWS_REGION` | `us-west-1` | Region passed as `-r` |
| `DQL_HOST` / `DQL_PORT` | unset / `8000` | When host is set, kernels add `-H` / `-p` |
| `DQL_NOTEBOOK_JSON` | `1` | Pass `--json` so SELECT results can render as a table |
| `DQL_NOTEBOOK_PORT` | Jupyter default | Lab port |

`--local` sets `DQL_HOST=localhost`, `DQL_PORT=8000`, and dummy `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` if those are unset.

## Examples

Unexecuted notebooks (no outputs in git):

- [`examples/getting-started-python.ipynb`](examples/getting-started-python.ipynb) — `Engine` + `%%dql`
- [`examples/getting-started-rust.ipynb`](examples/getting-started-rust.ipynb) — DQL cells on the Rust kernel (switch kernel to **DQL (Python)** to compare)

They assume DynamoDB Local. Do not commit `--json` dumps from a real account.

## Python magics

In a **Python (dql)** notebook:

```python
%load_ext dql_notebook_kernel
```

```python
%%dql
SCAN * FROM nb_posts;
```

```python
%%dqlrs
ls
```
