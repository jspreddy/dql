# Notebook UI tests

Playwright checks that JupyterLab can run DQL. The suite starts Lab from
[`notebook/`](../notebook/) with the project kernels, opens the smoke notebook
in Chromium, and runs DQL (Rust) cells against DynamoDB Local.

## Run

Requires [uv](https://docs.astral.sh/uv/), a built `dqlrs`, and DynamoDB Local
on port 8000.

```bash
./notebook-tests/run.sh --start-local
./notebook-tests/run.sh --headed          # watch the browser
./notebook-tests/run.sh --skip-install    # envs and Chromium already installed
```

`run.sh` syncs the notebook env, registers the DQL kernels, installs the
Playwright Chromium build, and runs pytest. Screenshots and videos are kept
for failures under `notebook-tests/test-results/`.

## What a test can do

Helpers in [`notebook_ui.py`](notebook_ui.py):

- `open_lab` — JupyterLab launcher
- `open_smoke_notebook` — open `fixtures/smoke-dqlrs.ipynb` from the file browser and pick **DQL (Rust)** if Lab asks
- `run_code_cell` — focus a code cell, Shift+Enter, wait until the prompt shows an execution count, return the output text

The Lab process is one pytest session. It listens on 127.0.0.1 with a random port and token. Kernels inherit `DQLRS_BIN` / `DQL_BIN` and `DQL_HOST=localhost`.

Set `DQLRS_BIN` when the debug or release binary is not in the usual `rust-impl/target` path.
