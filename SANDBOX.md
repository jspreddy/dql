# Docker sandbox for maintainers

Run Claude Code or the Cursor CLI on this repository inside a [Docker sandbox](https://docs.docker.com/ai/sandboxes/):
a small VM with the toolchain `just verify` needs. The sandbox runs on the `dql-dev` template, built
from [`dev-env/Dockerfile`](dev-env/Dockerfile) and [`dev-env/init.sh`](dev-env/init.sh)
(see [MAINTAINER.md, "Dev environment"](MAINTAINER.md#dev-environment)).

This is for maintainers. People who install dql don't need it.

The template starts from Docker's Claude Code template (`docker/sandbox-templates:claude-code`):
Ubuntu, a non-root `agent` user with sudo, Node.js, Python, uv, Java, and Claude Code. On top of it,
`init.sh` adds Rust with rustfmt and clippy, just, cmake, cargo-release, git-graph, Google Chrome for
the Playwright tests, and the Cursor CLI.

The steps use clone mode (`--clone`): the sandbox gets its own Git clone of your checkout instead of
mounting your files. Symlinks such as `.claude/skills` stay symlinks, files keep LF line endings,
and Linux build output (`.venv`, `node_modules`, `rust-impl/target`) never mixes with a host build.

# Prerequisites

| Host | Needs |
| --- | --- |
| macOS | macOS 14 (Sonoma) or later on Apple silicon, [Homebrew](https://brew.sh/), and Docker Desktop to build the template |
| Windows | Windows 11 on an Intel or AMD processor, Windows Hypervisor Platform, [Git for Windows](https://git-scm.com/download/win) on the system PATH, and Docker Desktop to build the template |
| Linux | Ubuntu 24.04 or later on x86-64 or Arm64, KVM turned on, and Docker Engine to build the template |

On every host you also need:

- A Docker account, for `sbx login`.
- An Anthropic API key or a Claude subscription for Claude Code, or a Cursor account for the Cursor CLI.
- About 20 GB of free disk: the template, plus each sandbox's Rust build.

`sbx` runs sandboxes without Docker Desktop or Docker Engine. Docker is only needed to build the
`dql-dev` template.

# Install on macOS

## Install Docker Sandboxes

```bash
brew trust docker/tap
```

```bash
brew install docker/tap/sbx
```

## Configure it

```bash
sbx login
```

Balanced allows common development services (npm, PyPI, crates.io, GitHub) and asks before others:

```bash
sbx policy init balanced
```

The setup step downloads DynamoDB Local from a host that Balanced doesn't list. Allow it now, or
approve it when the sandbox asks:

```bash
sbx policy allow network d1ni2b6xgvw0s0.cloudfront.net:443
```

Store your keys on the host. The sandbox proxy adds them to requests, so they never enter the
sandbox. Skip the key for Claude Code to sign in with `/login` and your subscription instead.

```bash
sbx secret set anthropic
```

```bash
sbx secret set cursor
```

Apple silicon is Arm64, and Google has no Linux Chrome for Arm64. When the template is built,
`init.sh` installs Playwright's Chromium at the path Playwright's `chrome` channel looks for.

# Install on Windows

## Install Docker Sandboxes

Turn on Windows Hypervisor Platform once, in an administrator PowerShell, then restart Windows:

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All
```

Install `sbx` in a normal PowerShell:

```powershell
winget install -h Docker.sbx
```

## Configure it

```powershell
sbx login
```

```powershell
sbx policy init balanced
```

```powershell
sbx policy allow network d1ni2b6xgvw0s0.cloudfront.net:443
```

```powershell
sbx secret set anthropic
```

```powershell
sbx secret set cursor
```

Clone mode runs Git through the `sbx` daemon, a background process with its own PATH. If
`sbx create --clone` fails with `exec: "git": executable file not found in %PATH%`, Git is
missing from the saved PATH, even though PowerShell finds it. Install Git for Windows, which adds
itself to the saved PATH:

```powershell
winget install --id Git.Git -e
```

Then open a new PowerShell window and restart the daemon:

```powershell
sbx daemon restart
```

If the error persists, sign out of Windows and back in.

If the repository lives in WSL (`\\wsl.localhost\...`), Windows Git's ownership check makes
`--clone` fail with "not in a Git repository". Add the repository to `safe.directory`:

```powershell
git config --global --add safe.directory '%(prefix)///wsl.localhost/Ubuntu/home/<you>/dql'
```

# Install on Linux

## Install Docker Sandboxes

Check that KVM is available. The output should list `kvm_intel`, `kvm_amd`, `kvm_arm64`, or `kvm`:

```bash
lsmod | grep kvm
```

Add your user to the `kvm` group, then sign out and back in:

```bash
sudo usermod -aG kvm $USER
```

Docker's script installs Docker Engine (to build the template) and `sbx` together:

```bash
curl -fsSL https://get.docker.com | sudo SBX=1 sh
```

## Configure it

```bash
sbx login
```

```bash
sbx policy init balanced
```

```bash
sbx policy allow network d1ni2b6xgvw0s0.cloudfront.net:443
```

```bash
sbx secret set anthropic
```

```bash
sbx secret set cursor
```

On Arm64, the template gets Playwright's Chromium in place of Google Chrome, as on macOS.

# Prep the sandbox and run it

## 1. Build the template

On macOS and Linux, build from your checkout. `just dev-image` runs the same command.

```bash
docker build -t dql-dev dev-env
```

On Windows, build from the Git URL. Docker then clones the branch on Linux, so `init.sh` keeps
its LF line endings even if your Windows checkout converted them to CRLF:

```powershell
docker build -t dql-dev "https://github.com/jspreddy/dql.git#v-rust:dev-env"
```

The build takes about 5 minutes, most of it compiling `cargo-release` and `git-graph` from source.

Build arguments:

- `BASE_IMAGE` picks another base image.
- `INIT_ARGS` (default `--no-repo`) picks the `init.sh` steps, for example `--no-repo --skip browser`.

## 2. Load the template into `sbx`

`sbx` keeps its own image store and doesn't see Docker's images. Save the image to a file and load it:

```bash
docker image save dql-dev -o dql-dev.tar
```

```bash
sbx template load dql-dev.tar
```

Rebuild and load the template again after `dev-env/` changes. Sandboxes you already created
keep the template they started from.

## 3. Create the sandbox and run it

From the main checkout (not a Git worktree), check out the branch to start from. The sandbox's
clone follows whatever branch your checkout has when you create it.

```bash
git checkout v-rust
```

The first run creates the sandbox from the template, makes the clone, and starts Claude Code. Use
`cursor` in place of `claude` for the Cursor CLI.

```bash
sbx run --clone --template dql-dev:latest --name dql claude .
```

Later, attach to the same sandbox by name:

```bash
sbx run --name dql
```

On the first run, ask the agent to run `dev-env/init.sh repo`. That syncs the Python environments,
fetches crates, installs the Playwright packages, and downloads DynamoDB Local into the clone. After
that, `just verify` works in the sandbox.

To check the clone, ask the agent to run `ls -l .claude/skills`. It shows `.claude/skills -> ../.cursor/skills`.

## 4. Bring the work back

Edits stay in the sandbox until you fetch or push them. Ask the agent to work on its own branch,
then fetch from the `sandbox-<name>` remote that `sbx` adds to your checkout:

```bash
git fetch sandbox-dql
```

```bash
git log sandbox-dql/<branch>
```

Then check out or merge the branch and push from the host. The agent can also push from the
sandbox: the clone gets your checkout's remotes, but pushing needs credentials in the sandbox.

## Stop and remove

`sbx stop dql` keeps the sandbox and its clone. Fetching from `sandbox-dql` fails until you start
it again with `sbx run --name dql`.

`sbx rm dql` deletes the sandbox, its clone, unpushed commits, and the warmed-up environment. Fetch
or push first.

## Without `sbx`

To check the template with plain Docker, mount the checkout at the sandbox's workspace path:

```bash
docker run --rm -it -v "$PWD":/home/agent/workspace dql-dev bash
```

# Run multiple sandboxes

Create one sandbox per task, each with its own name and `--clone`. Check out the branch to start from before each one:

```bash
sbx run --clone --template dql-dev:latest --name dql-a claude .
```

```bash
sbx run --clone --template dql-dev:latest --name dql-b claude .
```

Run each in its own terminal. Attach later with `sbx run --name dql-a`.

Each sandbox is a separate VM with its own clone, files, processes, and network. One sandbox's
edits, builds, and tests don't touch another, and `sbx rm dql-a` removes only that one.

- **Separate sandboxes, not worktrees in one.** Worktrees inside one sandbox share its DynamoDB
  Local on port 8000, and tests must not run against it while `just verify` runs. Each sandbox
  starts its own DynamoDB Local, on its own port 8000, so two `just verify` runs don't collide.
- **Set up each one.** Run `dev-env/init.sh repo` once in every new sandbox. Each clone builds Rust
  from scratch the first time, which takes several minutes and disk space.
- **One branch per sandbox.** Ask each agent to create its own branch, and fetch from each remote:
  `git fetch sandbox-dql-a`, `git fetch sandbox-dql-b`.
- **CPU and memory are shared.** `just verify` builds Rust and runs Playwright. Two at once each
  run slower, and the timing-sensitive web tests flake more often. Run one heavy job at a time
  when a failure looks random.
- **Host ports can clash.** Ports inside sandboxes never collide. If you publish a port to open
  `just web` in a host browser, give each sandbox a different host port.

Shared between sandboxes: the template (each sandbox gets its own copy of its filesystem), the
secrets from `sbx secret set`, and the network policy, including approvals.

# References

- [Install Docker Sandboxes](https://docs.docker.com/ai/sandboxes/install/)
- [Claude Code in Docker Sandboxes](https://docs.docker.com/ai/sandboxes/agents/claude-code/)
- [Base images and templates](https://docs.docker.com/ai/sandboxes/customize/templates/)
- [Usage: clone mode and templates](https://docs.docker.com/ai/sandboxes/usage/)
- [Git workflows](https://docs.docker.com/ai/sandboxes/workflows/git/)
- [Troubleshooting](https://docs.docker.com/ai/sandboxes/troubleshooting/)
