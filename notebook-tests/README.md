# Notebook UI tests

Playwright checks that JupyterLab can run DQL. The suite starts Lab from
[`notebook/`](../notebook/) with the project kernels, opens the smoke notebook
in Chrome, and runs DQL (Rust) cells against DynamoDB Local.

## Run

Requires [uv](https://docs.astral.sh/uv/), a built `dqlrs`, and DynamoDB Local
on port 8000.

```bash
./notebook-tests/run.sh --start-local
./notebook-tests/run.sh --list            # print each test id
./notebook-tests/run.sh --headed          # show the Chrome window
./notebook-tests/run.sh --slowmo 400      # headed, pause 400ms between actions
./notebook-tests/run.sh --debug           # Playwright inspector; pauses until you resume
./notebook-tests/run.sh --skip-install    # envs and browser already installed
```

Run one test by passing its node id after `--`:

```bash
./notebook-tests/run.sh --headed --skip-install -- tests/test_dql_notebook.py::test_launcher_lists_dql_kernels
./notebook-tests/run.sh --headed --skip-install -- tests/test_dql_notebook.py::test_rust_kernel_runs_select
```

`./notebook-tests/run.sh --list` prints the ids. `-k launcher` also selects by name.

`run.sh` syncs the notebook env, registers the DQL kernels, and runs pytest
through Playwright. When Google Chrome is installed (including
`/Applications/Google Chrome.app` on macOS), tests use that browser
(`--browser-channel chrome`). Otherwise `run.sh` downloads Playwright's
Chromium build (`uv run playwright install chromium`). Failure screenshots
are kept under `notebook-tests/test-results/`. Video needs
`uv run playwright install ffmpeg` and `--video=retain-on-failure`.

## What a test can do

Helpers in [`notebook_ui.py`](notebook_ui.py):

- `open_lab` — JupyterLab launcher
- `open_smoke_notebook` — open `fixtures/smoke-dqlrs.ipynb` from the file browser and pick **DQL (Rust)** if Lab asks
- `run_code_cell` — focus a code cell, Shift+Enter, wait until the prompt shows an execution count, return the output text

The Lab process is one pytest session. It listens on 127.0.0.1 with a random port and token. Kernels inherit `DQLRS_BIN` / `DQL_BIN` and `DQL_HOST=localhost`.

Set `DQLRS_BIN` when the debug or release binary is not in the usual `rust-impl/target` path.
