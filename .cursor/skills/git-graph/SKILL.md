---
name: git-graph
description: Render one SVG of remote branches on top of origin/v-rust. Use when the user asks for a git graph, a branch picture, or changes above v-rust. Run only when asked.
---

# Git graph

From the repository, run:

```bash
python3 .cursor/skills/git-graph/snapshot.py
```

The script prints `svg: <path>`. Show that SVG in Cursor chat.

Stop there when the script succeeds. Do not install `git-graph` or change the Rust toolchain on a successful run.

## FAQ

Use this section only when `snapshot.py` failed with the error named in the heading. After the fix, run the script again and show the SVG.

### git-graph is not installed, or the command is not found

`git-graph` is a global Cargo binary. Do not add it to a `Cargo.toml` or `Cargo.lock` in this repository.

Install it from outside the repo:

```bash
cd /tmp
cargo install git-graph
```

### Cargo reports that the edition2024 feature is required

Install a newer stable toolchain and repeat the install from `/tmp`:

```bash
rustup toolchain install stable
cd /tmp
cargo +stable install git-graph
```
