# 08. Notebook: Jupyter tokens, kernel keys, and autosaved outputs

**Severity:** High. **Needs:** a shell as the person's OS user.

## What happens

`dqlrs notebook` starts JupyterLab on `127.0.0.1` with the DQL (Rust) kernel. Jupyter's own
credentials and its saved notebooks are files that any process of the user can read:

1. **The server token.** Jupyter writes `jpserver-<pid>.json` (with `token` and `port`) to its
   runtime folder, which `dqlrs` sets to `<data dir>/jupyter-runtime`. With it, an agent can
   use Jupyter's REST and WebSocket API: start a DQL kernel and execute `SCAN * FROM users`.
   `jupyter server list` prints the same URL with the token.
2. **The kernel connection files.** `kernel-<id>.json` in the same folder holds the ports and
   the HMAC key of each running kernel. With them, a client attaches to the person's running
   kernel (`jupyter console --existing kernel-<id>.json`) and executes cells in it.
3. **Autosaved outputs.** JupyterLab autosaves notebooks every two minutes by default, with
   cell outputs, as plaintext `.ipynb` in the notebook folder (the current directory, often the
   project). Every query result shown in a cell is then on disk, unencrypted, and so are copies in
   `.ipynb_checkpoints/`. The person did not explicitly save those rows.

## Reproduced

From the code and Jupyter's defaults; the notebook was not started in this review. `dqlrs
notebook` passes `--ip 127.0.0.1` and sets `JUPYTER_RUNTIME_DIR` (`notebook/mod.rs:96-111`). The
config (`notebook/jupyter_server_config.py`) changes neither the token handling nor autosave.

## Code

- `rust-impl/crates/dql-cli/src/notebook/mod.rs:88-115`: the `jupyter lab` command and its
  environment.
- `notebook/jupyter_server_config.py`: loopback only, no autosave or output settings.

## Fix

- Do not keep outputs on disk by default. Strip outputs on save with a contents-manager hook
  (`FileContentsManager.pre_save_hook` that clears outputs from DQL cells), or turn autosave off
  for DQL notebooks and say so in the banner.
- Keep the runtime folder `0700`. Note in the docs that this only stops other users.
- The token and kernel keys fall under 03's limit: a same-user process can read them. The real
  fix is the agent-identity separation in the README.
