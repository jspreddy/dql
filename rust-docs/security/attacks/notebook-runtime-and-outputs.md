# JupyterLab Runtime Tokens, Kernel Sockets, and Persisted Outputs (`dqlrs notebook`)

## Summary

`dqlrs notebook` is intended solely for humans, yet its runtime architecture creates three local circumvention channels: plaintext Jupyter server tokens and ZeroMQ kernel connection files in `~/.local/share/dqlrs/notebook/jupyter-runtime/`, user-writable kernel Python modules under `~/.local/share/dqlrs/notebook/src/`, and automatic persistence of query result rows inside `.ipynb` cell outputs and `.ipynb_checkpoints/` in the workspace.

## Violated boundary

- **Intended:** `dqlrs notebook` is a human-only interface; agents cannot run notebook queries or read ungranted query outputs produced during notebook sessions.
- **Actual:** Local agents with home-directory access can hijack the live Jupyter server or ZeroMQ kernel to run arbitrary DQL (or tamper with the Python kernel), and workspace-reading agents can read autosaved query results from `.ipynb` files.

## Code paths

1. **Runtime directory and connection files:** `rust-impl/crates/dql-cli/src/notebook/mod.rs` (`launch`) sets `JUPYTER_RUNTIME_DIR` to `~/.local/share/dqlrs/notebook/jupyter-runtime` (`notebook/layout.rs`). JupyterLab writes:
   - `jpserver-<pid>.json` containing the HTTP `url` and bearer `token` in cleartext (parsed directly by tests in `notebook/mod.rs`), and
   - `kernel-<uuid>.json` containing the ZeroMQ shell/iopub/stdin ports and HMAC signing `key` for each active `DqlKernel`.
2. **Unauthenticated `dqlrs --serve` backend inside the kernel:** `notebook/src/dql_notebook_kernel/kernel.py` (`DqlKernel`) and `serve_client.py` (`ServeSession`) spawn `dqlrs --serve` over stdio and execute every incoming cell payload via `{"op": "exec", "dql": code}`.
3. **Mutable Python sources and binary resolution on disk:**
   - `rust-impl/crates/dql-cli/src/notebook/layout.rs` (`materialize`, `write_kernel_sources`, `write_server_config`) unpacks `src/dql_notebook_kernel/*.py` and `jupyter-config/jupyter_server_config.py` into `~/.local/share/dqlrs/notebook/` with default user permissions. While `dqlrs notebook` is running—or if a file is edited before a kernel restart—Jupyter executes those files directly from disk.
   - `notebook/src/dql_notebook_kernel/runner.py` (`resolve_binary`) honors `DQLRS_BIN`, `DQL_BIN`, and `PATH` when spawning the CLI.
4. **Autosaved cell outputs and checkpoints in the workspace:** `notebook/src/dql_notebook_kernel/runner.py` (`format_display`, `format_envelope`) returns query rows as both `text/plain` (pretty-printed JSON) and `text/html` (`<table>` markup) in the Jupyter `display_data` bundle. JupyterLab automatically writes those `outputs` into the open `.ipynb` file and `.ipynb_checkpoints/<name>-checkpoint.ipynb` inside `--notebook-dir` (which defaults to the current workspace directory in `notebook/mod.rs`).

## Circumvention mechanics

1. **Executing queries (and extracting `aws-vault` credentials) via `jpserver-<pid>.json` or `kernel-<uuid>.json`:** When the human starts Jupyter with `aws-vault exec <profile> -- dqlrs notebook`, both `jupyter-lab` and its `DqlKernel` (`dqlrs --serve`) inherit the vended STS credentials. Any same-user agent that reads `~/.local/share/dqlrs/notebook/jupyter-runtime/` can:
   - connect to the ZeroMQ `shell_port` signed with `key` from `kernel-<uuid>.json` (or Jupyter's `/api/kernels` WebSocket using `token` from `jpserver-<pid>.json`) to run arbitrary DQL against live AWS, or
   - open a shell via Jupyter's `/api/terminals` endpoint using `token` from `jpserver-<pid>.json`, spawning a child shell **inside** the `aws-vault` process tree where `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` (or `AWS_CONTAINER_AUTHORIZATION_TOKEN`) can be read directly from `env`.
2. **Modifying materialized kernel Python files:** While `dqlrs notebook` is running, `layout::materialize` has already finished; editing `~/.local/share/dqlrs/notebook/src/dql_notebook_kernel/kernel.py` or `serve_client.py` causes the next started or restarted kernel (or the next `aws-vault exec <profile> -- dqlrs notebook` run if `runner.py` / `DQLRS_BIN` targets are modified after materialization) to run agent-controlled Python under the human's vended `aws-vault` credentials.
3. **Reading autosaved `.ipynb` and `.ipynb_checkpoints` outputs:** Even when the user never runs `SAVE`, JupyterLab's periodic autosave writes rendered result tables into `.ipynb` and `.ipynb_checkpoints/*.ipynb` inside the workspace. Unlike `.dqlrs-cache`, those JSON structures are unencrypted on disk, so an agent with workspace file access (though not MCP-only `.dql` tools) can read all rendered rows directly.

## Remediation

- **Isolate the notebook runtime directory and process principal:** Run `dqlrs notebook` under the human-only security domain so agent sandboxes have no read or write access to `~/.local/share/dqlrs/notebook/`, and enforce `0700` on `NotebookLayout::root` and `jupyter-runtime`.
- **Load kernel code from read-only or verified locations:** Verify integrity or install kernel modules in a root/daemon-owned read-only path rather than executing mutable Python files from a shared user data directory.
- **Strip or seal cell outputs in workspace notebooks:** Configure Jupyter (`jupyter_server_config.py` pre-save hook) to strip `outputs` before writing `.ipynb` and checkpoint files inside the workspace—or seal output payloads into `.dqlrs-cache`—so normal autosaves do not leave plaintext query rows in the project tree.
