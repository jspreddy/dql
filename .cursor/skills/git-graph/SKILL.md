---
name: git-graph
description: Render one SVG of remote branches on top of origin/v-rust. Use when the user asks for a git graph, a branch picture, or changes above v-rust. The script defaults to 100 commits and 1 week, whichever comes first. Pass -n/--count when the user names a commit limit and -d/--cutoff when the user names a date or a cutoff. Both limits can be passed together. Run only when asked.
---

# Git graph

From the repository, run:

```bash
python3 .cursor/skills/git-graph/snapshot.py
```

That will create an svg file and print its path. Show the file in the chat window. Do not think, do not read, just show the file.

## Options

Pass an option only when the user provides values.

With no flags, the script uses `-n 100 -d 1w`. It stops at 100 commits or one week before `origin/v-rust`, whichever comes first.

- `-n` / `--count` is the commit count. It limits how many commits snapshot script draws.

- `-d` / `--cutoff` prunes commits before a date. A duration before `origin/v-rust` is `2w`, `14d`, `48h`, `30m`, or a number of seconds. One week is `-d 1w`. "2 weeks" is `-d 2w`. "3 days" is `--cutoff 3d`. An absolute date uses git's `--before` formats, such as `-d 2026-11-01`.

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
