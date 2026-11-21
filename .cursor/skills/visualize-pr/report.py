#!/usr/bin/env python3
"""Print the files changed in a pull request, grouped for a quick read.

Files land in an area of the application (engine, web UI, examples, and so on).
Inside an area, a folder with at least --threshold changed files is one line
with a count. A file that stands alone is listed by its path.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass

# Longest matching prefix wins. More specific paths come first.
AREAS: tuple[tuple[str, str], ...] = (
    ("rust-impl/crates/dql-engine/", "Engine"),
    ("rust-impl/crates/dql-parser/", "Parser"),
    ("rust-impl/crates/dql-expr/", "Expressions"),
    ("rust-impl/crates/dql-models/", "Models"),
    ("rust-impl/crates/dql-output/", "Output"),
    ("rust-impl/crates/dql-cli/src/web/", "Web server"),
    ("rust-impl/crates/dql-cli/", "CLI"),
    ("dqlrs-web/static/vendor/", "Vendored web"),
    ("dqlrs-web/static/", "Web UI"),
    ("dqlrs-web/playwright/", "Web tests"),
    ("dqlrs-web/tests/", "Web unit tests"),
    ("dqlrs-web/examples/", "Examples"),
    ("dqlrs-web/", "Web"),
    ("py-impl/", "Python"),
    ("notebook/", "Notebook"),
    ("rust-docs/", "Docs"),
    (".cursor/skills/", "Skills"),
    (".cursor/", "Agent"),
    ("scripts/", "Scripts"),
)

STATUS = {
    "ADDED": "added",
    "MODIFIED": "modified",
    "DELETED": "deleted",
    "RENAMED": "renamed",
    "COPIED": "copied",
}


@dataclass(frozen=True)
class Change:
    path: str
    status: str


@dataclass(frozen=True)
class Row:
    sort_key: str
    text: str


def area_of(path: str) -> str:
    for prefix, name in AREAS:
        if path.startswith(prefix):
            return name
    return "Repository"


def area_order(name: str) -> int:
    for index, (_, label) in enumerate(AREAS):
        if label == name:
            return index
    return len(AREAS)


def parent_dir(path: str) -> str:
    if "/" not in path:
        return ""
    return path.rsplit("/", 1)[0]


def status_word(changes: list[Change]) -> str:
    kinds = {change.status for change in changes}
    if len(kinds) == 1:
        return next(iter(kinds))
    return "files"


def rows_for(changes: list[Change], threshold: int) -> list[Row]:
    """Roll a folder up when it holds enough files. Keep the rest by name.

    Files at the repository root stay listed by name. A folder line counts only
    the files directly in that folder, so a busy subfolder stays its own line.
    """
    by_dir: dict[str, list[Change]] = defaultdict(list)
    for change in changes:
        by_dir[parent_dir(change.path)].append(change)
    rows: list[Row] = []
    for directory, group in by_dir.items():
        if directory and len(group) >= threshold:
            word = status_word(group)
            noun = word if word != "files" else "files"
            rows.append(Row(f"{directory}/", f"{directory}/  ({len(group)} {noun})"))
            continue
        for change in group:
            rows.append(Row(change.path, f"{change.path}  {change.status}"))
    rows.sort(key=lambda row: row.sort_key)
    return rows


def render(title: str, changes: list[Change], threshold: int) -> str:
    lines = [title, f"{len(changes)} files", ""]
    if not changes:
        lines.append("No files changed.")
        return "\n".join(lines)
    by_area: dict[str, list[Change]] = defaultdict(list)
    for change in changes:
        by_area[area_of(change.path)].append(change)
    for name in sorted(by_area, key=area_order):
        lines.append(name)
        for row in rows_for(by_area[name], threshold):
            lines.append(f"  {row.text}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def load_pr(number: str | None) -> tuple[str, list[Change]]:
    command = ["gh", "pr", "view", "--json", "number,title,files"]
    if number:
        command.insert(3, number)
    try:
        raw = subprocess.run(command, check=True, capture_output=True, text=True).stdout
    except FileNotFoundError:
        sys.exit("gh is not installed.")
    except subprocess.CalledProcessError as err:
        message = (err.stderr or err.stdout or "could not read the pull request").strip()
        sys.exit(message)
    payload = json.loads(raw)
    changes = [
        Change(item["path"], STATUS.get(item.get("changeType", ""), "changed"))
        for item in payload.get("files") or []
    ]
    title = f"#{payload['number']}  {payload.get('title') or ''}".rstrip()
    return title, changes


def self_test() -> None:
    changes = [
        Change("dqlrs-web/examples/joins/a.dql", "added"),
        Change("dqlrs-web/examples/joins/b.dql", "added"),
        Change("dqlrs-web/examples/joins/c.dql", "added"),
        Change("dqlrs-web/examples/other.dql", "modified"),
        Change("rust-impl/crates/dql-engine/src/engine.rs", "modified"),
        Change("rust-impl/crates/dql-engine/src/lib.rs", "modified"),
        Change("CHANGELOG.md", "modified"),
        Change("Justfile", "modified"),
        Change("README.md", "modified"),
    ]
    text = render("#1  sample", changes, 3)
    expected = """#1  sample
9 files

Engine
  rust-impl/crates/dql-engine/src/engine.rs  modified
  rust-impl/crates/dql-engine/src/lib.rs  modified

Examples
  dqlrs-web/examples/joins/  (3 added)
  dqlrs-web/examples/other.dql  modified

Repository
  CHANGELOG.md  modified
  Justfile  modified
  README.md  modified
"""
    if text != expected:
        sys.stderr.write(text)
        sys.exit("grouping self-test failed")
    lone = rows_for([Change("src/a.rs", "added"), Change("src/b.rs", "added")], 3)
    if [row.text for row in lone] != ["src/a.rs  added", "src/b.rs  added"]:
        sys.exit("two files in a folder should stay listed by name")
    print("ok")


def main() -> None:
    parser = argparse.ArgumentParser(description="Group the files changed in a pull request.")
    parser.add_argument("pr", nargs="?", help="pull request number; default is the current branch")
    parser.add_argument("--threshold", type=int, default=3, help="files in one folder before it rolls up")
    parser.add_argument("--self-test", action="store_true", help="check the grouping rules")
    args = parser.parse_args()
    if args.threshold < 2:
        sys.exit("--threshold must be at least 2")
    if args.self_test:
        self_test()
        return
    title, changes = load_pr(args.pr)
    sys.stdout.write(render(title, changes, args.threshold))


if __name__ == "__main__":
    main()
