---
name: PR viz
description: Show the files changed in a pull request, grouped by area of the application. A folder with many changed files is one line with a count. A file that stands alone is listed by name. Use when the user asks what files a pull request changed, for a PR file list, or for changed files grouped by folder. Run only when asked.
---

# PR viz

From the repository, run:

```bash
python3 ".cursor/skills/visualize PR/report.py" PR
```

`PR` is the pull request number the user named. With no number, the script uses the pull request for the current branch.

The script prints the grouped list. Show that text as the answer. Do not turn the groups into a list of your own.

## How files are grouped

Each file is placed in one area by its path. Areas follow the crates and apps in this repo: Engine, Parser, Expressions, Models, Output, Web server, CLI, Web UI, Web tests, Web unit tests, Examples, Python, Notebook, Docs, Skills, Scripts, and Repository for anything else.

Inside an area, files that share a folder are counted together. Three or more files in one folder become one line, the folder path and the count. One or two files in a folder are listed by name. Files at the repository root are always listed by name.

## Options

Pass an option only when the user asks for it.

- `--threshold N`: how many files in one folder become a folder line. Default 3.
- `--self-test`: check the grouping rules and print nothing about a pull request.
