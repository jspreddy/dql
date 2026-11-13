---
name: changelog
description: Regenerate, preview, or check CHANGELOG.md, which is generated from the commits on origin/v-rust. The Changelog workflow updates it after every merge into v-rust; use this skill when the user asks to update, refresh, preview, or check the changelog, or when that workflow failed. The script rewrites the whole file from git, so never edit CHANGELOG.md by hand.
---

# Changelog

`CHANGELOG.md` at the repository root is generated from the first-parent history of
`origin/v-rust`, the target branch. Each merged pull request is one entry (its title and
number); each commit made on `v-rust` itself is one entry (its subject and short hash). Version
bump and lockfile sync commits are left out, and so are merged pull requests titled
`Update the changelog ...`. Entries are grouped under the version in the tree
when they landed: `dqlrs` (from `rust-impl/.bumpversion.cfg`) once the Rust workspace exists,
`dql (Python)` before it. Newest first.

## Automatic updates

`.github/workflows/changelog.yml` runs after every push to `v-rust`. When the regenerated file
differs, it opens a pull request titled `Update the changelog through #<last PR>` from
`bot/changelog-<sha>` and merges it (v-rust requires a pull request, so it cannot push). It
needs Settings > Actions > General > "Allow GitHub Actions to create and approve pull requests".
It never approves a pull request.

So after a merge there is nothing to do. Update by hand only when the user asks, or when the
workflow failed (look at its run in the Actions tab).

## Update it by hand

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
a merge into `v-rust` makes the file stale until the workflow's update merges a minute later.

## Notes

- Pending work does not appear until it merges into `v-rust`. To preview a branch, print the
  changelog for it without writing: `python3 .cursor/skills/changelog/changelog.py --ref HEAD --stdout`.
- Change the wording or grouping in `changelog.py`, not in `CHANGELOG.md`. Then regenerate.
- The pull request title comes from the merge commit body, which GitHub fills with the title.
  A merge without a body falls back to the branch name.
