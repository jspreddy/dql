---
name: git-graph
description: Render one PNG of every local branch and commit on top of v-rust with the global git-graph tool. Use when the user asks for a git graph, a branch picture, or a screenshot of changes above v-rust. Run only when asked. Do not run it after a commit, a new branch, or a push.
---

# Git graph

Render one picture of the commits and local branches on top of `v-rust`. Do this only when the user asks for the graph. Do not render it after a commit, a branch creation, or a push, and do not install a git hook.

## Install

`git-graph` is a global Cargo binary. Do not add it to a `Cargo.toml` or `Cargo.lock` in this repository.

If `git-graph` is not on `PATH`, install it from outside the repo:

```bash
cd /tmp
cargo install git-graph
```

If Cargo reports that the `edition2024` feature is required, install a newer stable toolchain and repeat the install from `/tmp`:

```bash
rustup toolchain install stable
cd /tmp
cargo +stable install git-graph
```

## Render

From the repository, run:

```bash
python3 .cursor/skills/git-graph/snapshot.py
```

The script writes one SVG and one PNG. It prints:

```text
png: /opt/cursor/artifacts/v-rust-branches.png
svg: /opt/cursor/artifacts/v-rust-branches.svg
```

When `/opt/cursor/artifacts` is missing, both files go to `/tmp/dql-git-graph/`.

Show the user that one PNG. Do not make a picture per commit or per branch.

Before showing the SVG, set the main `html` element's background to white. The snapshot page does this with `html { background: #ffffff; }`.

## What the picture includes

- The `v-rust` commit, as the base.
- Every local branch whose tip is a descendant of `v-rust`.
- The commits reachable from those tips and not reachable from `v-rust`.
- A detached `HEAD`, when it is a descendant of `v-rust` and is not already a branch tip.

Remote-tracking branches are left out, so each branch name appears once. Uncommitted files are not part of the graph.

## Check

Confirm the command printed one `png:` path, and that the image shows `v-rust` at the base with every descendant local branch labeled. There should be a single image, not a sequence.
