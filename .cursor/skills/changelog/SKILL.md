---
name: changelog
description: Regenerate CHANGELOG.md from the commits on origin/v-rust. Use when the user asks to update, refresh, or check the changelog, after pull requests merge into v-rust, or after a version bump lands. The script rewrites the whole file from git, so never edit CHANGELOG.md by hand.
---

# Changelog

`CHANGELOG.md` at the repository root is generated from the first-parent history of
`origin/v-rust`, the target branch. Each merged pull request is one entry (its title and
number); each commit made on `v-rust` itself is one entry (its subject and short hash). Version
bump and lockfile sync commits are left out. Entries are grouped under the version in the tree
when they landed: `dqlrs` (from `rust-impl/.bumpversion.cfg`) once the Rust workspace exists,
`dql (Python)` before it. Newest first.

## Update it

1. Fetch the target branch so the changelog sees the latest merges:

   ```bash
   git fetch origin v-rust
   ```

2. Following the branching rule, start a branch for the update (never commit to `v-rust`):
   from the current branch, or from `origin/v-rust` when the user wants a fresh branch.

3. Regenerate:

   ```bash
   just changelog
   ```

   (the same as `python3 .cursor/skills/changelog/changelog.py`). It prints how many sections it
   wrote, or that the file was already up to date.

4. Commit `CHANGELOG.md` with a subject such as `Update the changelog through #<last PR>`, push,
   and open a pull request into the branch it was cut from.

## Check it

```bash
just changelog-check
```

exits 1 when `CHANGELOG.md` does not match `origin/v-rust`. It is not part of `just verify`:
every merge into `v-rust` makes the file stale until the next update, which is expected.

## Notes

- Pending work does not appear until it merges into `v-rust`. To preview a branch, print the
  changelog for it without writing: `python3 .cursor/skills/changelog/changelog.py --ref HEAD --stdout`.
- Change the wording or grouping in `changelog.py`, not in `CHANGELOG.md`. Then regenerate.
- The pull request title comes from the merge commit body, which GitHub fills with the title.
  A merge without a body falls back to the branch name.
