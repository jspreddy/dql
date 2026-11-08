---
name: git-graph
description: Render one SVG of remote branches on top of origin/v-rust. Use when the user asks for a git graph, a branch picture, or changes above v-rust. Pass --count (-n) and --cutoff when the user names a commit limit or a time window. Run only when asked.
---

# Git graph

From the repository, run:

```bash
python3 .cursor/skills/git-graph/snapshot.py --count <n> --cutoff <duration>
```

Pass only the limits the user named. The defaults are `--count 50` and `--cutoff 2w` (two weeks before `origin/v-rust`).

- `n` or `count` is `--count` (same as `-n`). `/git-graph n 50` is `--count 50`.
- `cutoff` is `--cutoff`. A duration is `2w`, `14d`, `48h`, `30m`, or a number of seconds. "2 weeks" is `2w`. "3 days" is `3d`.

Each run writes a new SVG. The filename has a random part, so it does not replace an earlier graph. The script prints `svg: <path>`. Show that path's SVG in Cursor chat. Do not reuse an older graph file.

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
