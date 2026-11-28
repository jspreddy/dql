# 12. Notebook: Jupyter tokens, kernel keys, mutable kernel code, and autosaved outputs

**Severity:** High. **Found by:** both; mutable kernel code by Cursor. **Needs:** a shell as the
person's OS user, or workspace read access for the autosaved outputs. **Under `aws-vault exec`:**
worse. JupyterLab, its kernels, and its terminals inherit the credentials.

## What happens

`dqlrs notebook` runs JupyterLab on `127.0.0.1` with `JUPYTER_RUNTIME_DIR` set to
`~/.local/share/dqlrs/notebook/jupyter-runtime`.

1. **Server token** (both): `jpserver-<pid>.json` holds the URL and `token`. With it, an agent uses
   Jupyter's API to run DQL in a kernel. `jupyter server list` prints the same.
2. **Kernel keys** (both): `kernel-<id>.json` holds the ZeroMQ ports and HMAC `key`. A client
   attaches to the person's running kernel and executes cells. The kernel runs `dqlrs --serve`
   over stdio and executes each cell as `{"op":"exec","dql":…}`.
3. **Terminals with credentials** (both): under `aws-vault exec … -- dqlrs notebook`, JupyterLab
   holds the credentials, and every terminal it opens inherits them. With the server token, an
   agent opens one through `/api/terminals` and runs `env` or `aws`.
4. **Mutable kernel code** (Cursor): `layout::materialize` writes the Python kernel
   (`src/dql_notebook_kernel/*.py`) and `jupyter-config/jupyter_server_config.py` under
   `~/.local/share/dqlrs/notebook/` with the user's permissions. Editing them makes the next kernel
   start run the agent's code with the person's credentials. `runner.py` also honors `DQLRS_BIN`,
   `DQL_BIN`, and `PATH`.
5. **Autosaved outputs** (both): the kernel returns rows as `text/plain` and `text/html`.
   JupyterLab autosaves every two minutes by default, outputs included, into `.ipynb` and
   `.ipynb_checkpoints/` in the notebook folder, in plaintext. The person did not save those rows.

## Code

- `rust-impl/crates/dql-cli/src/notebook/mod.rs:88-115`: the `jupyter lab` command and environment.
- `notebook/layout.rs` `materialize`, `write_kernel_sources`, `write_server_config`.
- `notebook/src/dql_notebook_kernel/kernel.py`, `serve_client.py`, `runner.py`.
- `notebook/jupyter_server_config.py`: loopback only; no autosave or output settings.

## Status

From the code and Jupyter's defaults (both). The notebook was not started in either review.

## Fix

- Strip outputs before writing notebooks (a `pre_save_hook`), or turn autosave off for DQL
  notebooks and say so (both).
- Turn off Jupyter terminals (`c.ServerApp.terminals_enabled = False`); keep `AWS_*` out of
  JupyterLab and pass credentials to the kernel over a pipe; make the kernel non-dumpable (14)
  (Claude).
- Install kernel code read-only, or verify it before start (Cursor).
- `0700` on the notebook root and runtime folder (both). That only stops other users; the rest
  needs the agent-identity separation in the README.
