#!/usr/bin/env python3
"""Write CHANGELOG.md from the history of the target branch (origin/v-rust).

The changelog is generated, never edited by hand: run this again and it rewrites the file
from git. Entries are the first-parent history of the branch, newest first:

- a merged pull request is one entry, its title from the merge commit body, with its number;
- a commit made on the branch itself is one entry, its subject and short hash;
- version bump and lockfile sync commits are left out, since each section names its version;
- changelog updates ("Update the changelog ...") are left out, so the update the changelog
  workflow merges after each pull request does not make the file stale again.

Sections group entries by the version in the tree at that commit: dqlrs (rust-impl) once the
Rust workspace exists, dql (py-impl) before it. A version bump that arrives inside a pull
request opens a new section at that merge.

Usage:
  changelog.py                 rewrite CHANGELOG.md at the repository root
  changelog.py --check         exit 1 if CHANGELOG.md is out of date
  changelog.py --ref REF       use REF instead of origin/v-rust (falls back to v-rust)
  changelog.py --stdout        print instead of writing
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_REFS = ("origin/v-rust", "v-rust")
# Where each product kept its version, newest layout first.
VERSION_FILES = (
    ("dqlrs", "rust-impl/.bumpversion.cfg", r"^current_version\s*=\s*(\S+)"),
    ("dql", "py-impl/.bumpversion.cfg", r"^current_version\s*=\s*(\S+)"),
    ("dql", ".bumpversion.cfg", r"^current_version\s*=\s*(\S+)"),
    ("dql", "dql/__init__.py", r"^__version__\s*=\s*['\"]([^'\"]+)['\"]"),
)
SKIP_SUBJECTS = (
    re.compile(r"^Bump version: "),
    re.compile(r"^Sync (Cargo\.lock|lockfile) with "),
)
# Merged pull requests whose title matches are not entries.
SKIP_TITLES = (re.compile(r"^Update the changelog\b"),)
MERGE_SUBJECT = re.compile(r"^Merge pull request #(\d+) from [^/\s]+/(\S+)")
HEADER = """# Changelog

Generated from the first-parent history of `{ref}` by the `changelog` skill
(`python3 .cursor/skills/changelog/changelog.py`). Do not edit by hand: changes are
rewritten the next time it runs. Newest first. Each section is the version in the tree
when those changes landed.
"""


@dataclass
class Entry:
    sha: str
    date: str
    text: str


@dataclass
class Section:
    product: str
    version: str
    entries: list[Entry] = field(default_factory=list)


def git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=root, check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def repo_root() -> Path:
    return Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())


def resolve_ref(root: Path, wanted: str | None) -> str:
    for ref in [wanted] if wanted else DEFAULT_REFS:
        try:
            git(root, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}")
            return ref
        except RuntimeError:
            continue
    raise SystemExit(f"changelog: no such ref: {wanted or ' or '.join(DEFAULT_REFS)}")


def version_at(root: Path, sha: str) -> tuple[str, str]:
    """('dqlrs', version) when rust-impl exists at `sha`, else ('dql', version)."""
    for product, path, pattern in VERSION_FILES:
        try:
            text = git(root, "show", f"{sha}:{path}")
        except RuntimeError:
            continue
        match = re.search(pattern, text, re.MULTILINE)
        if match:
            return product, match.group(1)
    return "dql", "unversioned"


def entry_text(subject: str, body: str, sha: str) -> str | None:
    merge = MERGE_SUBJECT.match(subject)
    if merge:
        number, branch = merge.groups()
        title = next((line.strip() for line in body.splitlines() if line.strip()), "")
        if any(pattern.match(title) for pattern in SKIP_TITLES):
            return None
        return f"{title or branch} (#{number})"
    if any(pattern.match(subject) for pattern in (*SKIP_SUBJECTS, *SKIP_TITLES)):
        return None
    return f"{subject} ({sha})"


def collect(root: Path, ref: str) -> list[Section]:
    # %x1e separates records and %x1f fields, so subjects and bodies can hold anything else.
    raw = git(
        root,
        "log",
        "--first-parent",
        "--date=short",
        "--format=%h%x1f%ad%x1f%s%x1f%b%x1e",
        ref,
    )
    sections: list[Section] = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        sha, date, subject, body = (record.split("\x1f") + ["", "", "", ""])[:4]
        product, version = version_at(root, sha)
        if not sections or (sections[-1].product, sections[-1].version) != (product, version):
            sections.append(Section(product, version))
        text = entry_text(subject, body, sha)
        if text:
            sections[-1].entries.append(Entry(sha, date, text))
    return sections


def render(ref: str, sections: list[Section]) -> str:
    out = [HEADER.format(ref=ref)]
    for section in sections:
        if not section.entries:
            continue
        dates = [entry.date for entry in section.entries]
        span = dates[-1] if dates[-1] == dates[0] else f"{dates[-1]} to {dates[0]}"
        name = "dqlrs" if section.product == "dqlrs" else "dql (Python)"
        out.append(f"\n## {name} {section.version} ({span})\n\n")
        out.extend(f"- {entry.text}\n" for entry in section.entries)
    return "".join(out)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ref", help="branch or commit to read (default: origin/v-rust, else v-rust)")
    parser.add_argument("--check", action="store_true", help="exit 1 if CHANGELOG.md is out of date")
    parser.add_argument("--stdout", action="store_true", help="print the changelog instead of writing it")
    args = parser.parse_args(argv)

    root = repo_root()
    ref = resolve_ref(root, args.ref)
    sections = collect(root, ref)
    text = render(ref, sections)
    target = root / "CHANGELOG.md"
    if args.stdout:
        sys.stdout.write(text)
        return 0
    current = target.read_text(encoding="utf-8") if target.exists() else ""
    if args.check:
        if current != text:
            print(f"CHANGELOG.md is out of date with {ref}; run .cursor/skills/changelog/changelog.py", file=sys.stderr)
            return 1
        print(f"CHANGELOG.md is up to date with {ref}")
        return 0
    if current == text:
        print(f"CHANGELOG.md already up to date with {ref}")
        return 0
    target.write_text(text, encoding="utf-8")
    written = sum(1 for section in sections if section.entries)
    print(f"Wrote CHANGELOG.md from {ref}: {written} sections")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
