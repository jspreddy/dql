# Maintainer notes

[![Check](https://github.com/jspreddy/dql/actions/workflows/check.yml/badge.svg)](https://github.com/jspreddy/dql/actions/workflows/check.yml)

| Path | Contents |
| --- | --- |
| [`py-impl/`](py-impl/) | Python package, tests, packaging, tooling |
| [`py-docs/`](py-docs/) | Sphinx docs for the Python client |
| [`rust-impl/`](rust-impl/) | Rust Cargo workspace (`dqlrs`) |
| [`rust-docs/`](rust-docs/) | User docs for the Rust client; plans and archive under [`rust-docs/plans/`](rust-docs/plans/README.md) |
| [`black-box-tests/`](black-box-tests/) | Black-box acceptance cases (binaries + DynamoDB Local) |
| [`notebook/`](notebook/) | JupyterLab UI and DQL kernels |
| [`dqlrs-web/`](dqlrs-web/) | Web UI assets and Playwright tests |
| [`screenshots/`](screenshots/) | README screenshots (`cli/`, `web/`, `notebook/`) |
| [`scripts/`](scripts/) | Shared installers, version bump, helpers |
| [`dev-env/`](dev-env/) | Maintainer environment: setup script and Docker sandbox template |
| [`.github/workflows/`](.github/workflows/) | Python and Rust CI |

## Local install from a clone

```bash
git clone https://github.com/jspreddy/dql.git
cd dql
git checkout v-rust
```

Python (editable):

```bash
cd py-impl
uv tool install --python 3.9 --editable .
```

Rust:

```bash
cd rust-impl
cargo install --path crates/dql-cli --locked --root ~/.local
```

## Dev environment

[`dev-env/`](dev-env/) sets up a machine for working on this repository: Rust (stable, rustfmt,
clippy), just, uv, Node.js, Java for DynamoDB Local, Google Chrome for the Playwright tests,
cargo-release and git-graph, and the Claude Code and Cursor CLIs. It is maintainer tooling; people
who install dql don't need it.

### Setup script

[`dev-env/init.sh`](dev-env/init.sh) is the one setup script for every place we develop. Each step
installs only what is missing, so running it again is cheap. Run it as root or as a user with
passwordless sudo.

```bash
dev-env/init.sh
```

| Command | Does |
| --- | --- |
| `dev-env/init.sh` | Every step. The repo step syncs the Python envs, fetches crates, installs the Playwright packages, and downloads DynamoDB Local |
| `dev-env/init.sh --no-repo` | Tools only, for image builds |
| `dev-env/init.sh rust browser` | Only the named steps: `system`, `node`, `rust`, `tools`, `agents`, `browser`, `repo` |
| `dev-env/init.sh --skip agents` | Every step except one |
| `dev-env/init.sh --check` | Lists what is installed and changes nothing |

`just dev-init` runs it from anywhere in the checkout (`just dev-init -- --check`).
`DQL_SKIP_CARGO_TOOLS=1` skips cargo-release and git-graph, which compile from source.
The `rust` step also updates an installed stable toolchain to the latest stable.

Where to call it:

| Place | Setting |
| --- | --- |
| Docker sandbox | Built into the template image below. In the sandbox, run `dev-env/init.sh repo` once to warm the checkout |
| Claude Code cloud environment | Setup script, which starts in the directory that holds the checkouts: `bash dql/dev-env/init.sh` |
| Cursor cloud agent | Install command: `bash dev-env/init.sh` |
| Ubuntu, Debian, or WSL | `dev-env/init.sh` from the checkout |

Cloud environments only reach hosts their network policy allows. The script downloads from
`sh.rustup.rs` and `static.rust-lang.org`, `astral.sh` and `github.com` (uv and the Python builds
uv installs), `claude.ai` and `downloads.claude.ai`, `cursor.com` and `downloads.cursor.com`,
`dl.google.com` (Chrome), `deb.nodesource.com` (only when Node.js is missing or old), the
DynamoDB Local download (`d1ni2b6xgvw0s0.cloudfront.net`), and the apt, npm, PyPI, and crates.io
registries. A step that can't download fails, the script names it, and the other steps still run.

### Docker sandbox template

[`dev-env/Dockerfile`](dev-env/Dockerfile) starts from Docker's Claude Code sandbox template
(`docker/sandbox-templates:claude-code`: Ubuntu, a non-root `agent` user with sudo, Node.js,
Python, uv, Java, and Claude Code) and runs `init.sh --no-repo` on top. The repository is mounted
when the sandbox starts; it isn't copied into the image.

```bash
just dev-image
```

`sbx` keeps its own image store, so load the local image into it before the first run:

```bash
docker image save dql-dev -o dql-dev.tar
```

```bash
sbx template load dql-dev.tar
```

```bash
sbx run --template dql-dev claude
```

Run `sbx run --template dql-dev cursor` for the Cursor CLI instead. Credentials stay on the
host: `sbx secret set` stores them, and the sandbox proxy adds them to requests.

To try the image without `sbx`:

```bash
docker run --rm -it -v "$PWD":/home/agent/workspace dql-dev bash
```

Build arguments: `BASE_IMAGE` picks another base, and `INIT_ARGS` (default `--no-repo`) sets the
steps, for example `--build-arg INIT_ARGS="--no-repo --skip browser"`.

## Verification

[`VERIFICATION.md`](VERIFICATION.md). From the repo root,
[`just`](https://just.systems/) lists tasks; `just verify` runs the full check. `just flake web-test 10 2 -- lint` builds once and repeats a test recipe on fewer cores to find flaky tests, with a log per run. `just changelog` regenerates [CHANGELOG.md](CHANGELOG.md) from `Bump version` commits on `v-rust` (`scripts/changelog.py`).

Notebook from a checkout: `./notebook/start.sh --local --start-local`.
Details: [`notebook/README.md`](notebook/README.md).
