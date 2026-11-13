---
name: filetype-report
description: Render one PNG report of the file types checked into git, with files, lines, and size per type, and vendored code split from code written here. Use when the user asks for file-type metrics, a language breakdown, lines of code by type, repository size by type, or what is checked into git. Run only when asked.
---

# File-type report

From the repository, run:

```bash
python3 .cursor/skills/filetype-report/report.py
```

The script prints `html:` and `png:` paths. Show the PNG in the chat window, always as the PNG: chat clients show a PNG inline, while an HTML file arrives as a download. Do not show the HTML unless the user asks for it.

Then give the user the headline numbers from the page in a few lines: tracked paths, file types, total lines and size, the top types by lines and by size, and the note under the charts.

## What it counts

- Every path from `git ls-files`, read from the working tree. Uncommitted edits to tracked files are counted; untracked files are not.
- Files are grouped by lowercase extension. A file with no extension, or a dotfile such as `.gitignore`, is grouped by its whole name (`Justfile`, `LICENSE`).
- Lines are newline counts. A file with a NUL byte in its first 8 KB is binary and counts bytes only.
- Symlinks count as tracked paths but are not read.
- Paths under `dqlrs-web/static/vendor/` are vendored. The bars split vendored (gray) from written here (teal).

## Options

Pass an option only when the user asks for it.

- `--top N`: types shown in each bar chart before the rest fold into Other. Default 10.
- `--vendor PREFIX`: a path prefix to count as vendored. Repeat it for more prefixes; it replaces the default. `--vendor ''` counts nothing as vendored.
- `--out DIR`: where to write the HTML and PNG. Default `/opt/cursor/artifacts` when it exists, else `<temp>/dql-filetype-report/`. Files are named `filetypes-<short sha>`.
- `--no-png`: write only the HTML.

## FAQ

Use this section only when `report.py` failed with the error named in the heading. After the fix, run the script again and show the PNG.

### No Chrome or Chromium found to render the PNG

The script renders the PNG with headless Chrome. Install Google Chrome or Chromium, or put an existing one on `PATH`. Do not add a browser or an image library to this repository.
