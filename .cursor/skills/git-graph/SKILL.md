---
name: git-graph
description: Render one SVG of remote branches on top of origin/v-rust. Use when the user asks for a git graph, a branch picture, or changes above v-rust. Pass -n/--count when the user names a commit limit and -d/--cutoff when the user names a date or a cutoff. Both limits can be passed together. Run only when asked.
---

# Git graph

From the repository, run:

```bash
python3 .cursor/skills/git-graph/snapshot.py -n <count> -d <date>
```

`--count` is the same option as `-n`. `--cutoff` is the same option as `-d`. Pass either form, and pass both limits when the user names both. Pass only the limits the user named.

- `-n` / `--count` is the commit count. It limits how many commits git-graph draws. The picture may be restyled. Commits and lines stay. `/git-graph n 50` is `-n 50`.
- `-d` / `--cutoff` prunes commits before a date. A duration before `origin/v-rust` is `2w`, `14d`, `48h`, `30m`, or a number of seconds. "2 weeks" is `-d 2w`. "3 days" is `--cutoff 3d`. An absolute date uses git's `--before` formats, such as `-d 2026-11-01`.

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
