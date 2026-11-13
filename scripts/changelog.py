#!/usr/bin/env python3
"""Write CHANGELOG.md from the first-parent history of the target branch.

The changelog is generated, never edited by hand: run this again and it rewrites the
file from git. Entries are the first-parent history, newest first:

- a merged pull request starts with a link to the merge commit, then its title and a link to the pull request;
- a commit made on the branch starts with a link to that commit, then its subject;
- "Bump version:" commits and lockfile sync commits are left out, since the section
  heading names the version;
- changelog updates ("Update the changelog ...") are left out, so the workflow that
  merges the update does not make the file stale again.

Sections follow "Bump version: OLD → NEW" commits. Changes after a bump belong to the
next bump's version. Changes after the latest bump are "Unreleased". A bump that lands
inside a pull request counts when that pull request merges.

Usage:
  changelog.py                 rewrite CHANGELOG.md at the repository root
  changelog.py --check         exit 1 if CHANGELOG.md is out of date
  changelog.py --ref REF       use REF instead of origin/v-rust (falls back to v-rust)
  changelog.py --stdout        print instead of writing
  changelog.py --commit-title  print the workflow's commit subject and write nothing
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_REFS = ("origin/v-rust", "v-rust")
# Newest layout first. The first file that exists at a commit names the product.
VERSION_FILES = (
    ("dqlrs", "rust-impl/.bumpversion.cfg"),
    ("dql", "py-impl/.bumpversion.cfg"),
    ("dql", ".bumpversion.cfg"),
    ("dql", "dql/__init__.py"),
)
BUMP_SUBJECT = re.compile(r"^Bump version:\s*(\S+)\s*(?:→|->)\s*(\S+)\s*$")
SKIP_SUBJECTS = (
    BUMP_SUBJECT,
    re.compile(r"^Sync (Cargo\.lock|lockfile) with "),
)
# Merged pull requests whose title matches are not entries.
SKIP_TITLES = (re.compile(r"^Update the changelog\b"),)
MERGE_SUBJECT = re.compile(r"^Merge pull request #(\d+) from [^/\s]+/(\S+)")
HEADER = """# Changelog

Generated from the first-parent history of `{ref}` by `scripts/changelog.py`.
Do not edit by hand: changes are rewritten the next time it runs. Newest first.
A "Bump version" commit closes a section. Changes after that bump belong to the
next bump, or to Unreleased when no later bump exists.
"""


@dataclass
class Record:
    sha: str
    short: str
    date: str
    subject: str
    body: str


@dataclass
class Entry:
    date: str
    text: str


@dataclass
class Section:
    heading: str
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


def history(root: Path, ref: str) -> list[Record]:
    # %x1e separates records and %x1f fields, so subjects and bodies can hold anything else.
    raw = git(
        root,
        "log",
        "--first-parent",
        "--date=short",
        "--format=%H%x1f%h%x1f%ad%x1f%s%x1f%b%x1e",
        ref,
    )
    records: list[Record] = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        sha, short, date, subject, body = (record.split("\x1f") + ["", "", "", "", ""])[:5]
        records.append(Record(sha, short, date, subject, body))
    return records


def is_ancestor(root: Path, ancestor: str, descendant: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def earliest_index(root: Path, commits: list[str], sha: str) -> int | None:
    """Index of the oldest commit in `commits` (oldest first) that contains `sha`."""
    lo, hi = 0, len(commits) - 1
    found = None
    while lo <= hi:
        mid = (lo + hi) // 2
        if is_ancestor(root, sha, commits[mid]):
            found = mid
            hi = mid - 1
        else:
            lo = mid + 1
    return found


def product_at(root: Path, sha: str) -> str:
    for product, path in VERSION_FILES:
        result = subprocess.run(
            ["git", "cat-file", "-e", f"{sha}:{path}"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return product
    return "dql"


def tip_version(bumps: list[tuple[str, str, str]]) -> str:
    """The version a set of bumps that landed together released.

    `bumps` is (old, new, committer date). The released version is a "new" side that
    is not the "old" side of a different bump in the same set.
    """
    olds = {old for old, new, _date in bumps if old != new}
    tips = [item for item in bumps if item[1] not in olds]
    if not tips:
        tips = bumps
    tips.sort(key=lambda item: item[2])
    return tips[-1][1]


def bump_headings(root: Path, ref: str, oldest_first: list[str]) -> dict[str, str]:
    """Map the first-parent commit where a bump lands to its section heading."""
    raw = git(root, "log", "--format=%H%x1f%cI%x1f%s", ref, "--grep=^Bump version:")
    by_index: dict[int, list[tuple[str, str, str]]] = {}
    for line in raw.splitlines():
        sha, date, subject = (line.split("\x1f") + ["", "", ""])[:3]
        match = BUMP_SUBJECT.match(subject)
        if not match:
            continue
        index = earliest_index(root, oldest_first, sha)
        if index is None:
            continue
        by_index.setdefault(index, []).append((match.group(1), match.group(2), date))
    headings: dict[str, str] = {}
    for index, bumps in by_index.items():
        sha = oldest_first[index]
        product = "dqlrs" if product_at(root, sha) == "dqlrs" else "dql (Python)"
        headings[sha] = f"{product} {tip_version(bumps)}"
    return headings


def github_base(root: Path) -> str:
    """https://github.com/owner/repo from origin, or empty when origin is not GitHub."""
    try:
        url = git(root, "remote", "get-url", "origin").strip()
    except RuntimeError:
        return ""
    # origin may be git@github.com:owner/repo.git or https://<token>@github.com/owner/repo.
    # Keep only owner/repo. Never write credentials into the changelog.
    match = re.search(r"github\.com[:/]([^/\s]+/[^/\s]+?)(?:\.git)?$", url)
    return f"https://github.com/{match.group(1)}" if match else ""


def commit_link(base: str, record: Record) -> str:
    if not base:
        return record.short
    return f"[{record.short}]({base}/commit/{record.sha})"


def pull_link(base: str, number: str) -> str:
    label = f"#{number}"
    if not base:
        return label
    return f"[{label}]({base}/pull/{number})"


def entry_text(record: Record, base: str) -> str | None:
    link = commit_link(base, record)
    merge = MERGE_SUBJECT.match(record.subject)
    if merge:
        number, branch = merge.groups()
        title = next((line.strip() for line in record.body.splitlines() if line.strip()), "")
        if any(pattern.match(title) for pattern in SKIP_TITLES):
            return None
        return f"{link} {title or branch} ({pull_link(base, number)})"
    if any(pattern.match(record.subject) for pattern in (*SKIP_SUBJECTS, *SKIP_TITLES)):
        return None
    return f"{link} {record.subject}"


def collect(root: Path, ref: str) -> list[Section]:
    records = history(root, ref)
    oldest_first = [record.sha for record in reversed(records)]
    headings = bump_headings(root, ref, oldest_first)
    base = github_base(root)
    sections: list[Section] = []
    # Newer than the latest bump until a bump commit says otherwise.
    current = Section("Unreleased")
    for record in records:
        if record.sha in headings:
            # Everything already collected is after this bump, so it stays in the
            # section opened by the next bump (or Unreleased). This bump, and the
            # older changes that follow, belong to the version it introduced.
            if current.entries:
                sections.append(current)
            current = Section(headings[record.sha])
        text = entry_text(record, base)
        if text:
            current.entries.append(Entry(record.date, text))
    if current.entries:
        sections.append(current)
    return sections


def render(ref: str, sections: list[Section]) -> str:
    out = [HEADER.format(ref=ref)]
    for section in sections:
        dates = [entry.date for entry in section.entries]
        span = dates[-1] if dates[-1] == dates[0] else f"{dates[-1]} to {dates[0]}"
        out.append(f"\n## {section.heading} ({span})\n\n")
        out.extend(f"- {entry.text}\n" for entry in section.entries)
    return "".join(out)


def commit_title(root: Path, ref: str) -> str:
    """Subject for the workflow commit: the newest merged pull request that is not itself an update."""
    for record in history(root, ref):
        merge = MERGE_SUBJECT.match(record.subject)
        if not merge:
            continue
        number = merge.group(1)
        title = next((line.strip() for line in record.body.splitlines() if line.strip()), "")
        if any(pattern.match(title) for pattern in SKIP_TITLES):
            continue
        return f"Update the changelog through #{number}"
    return "Update the changelog"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ref", help="branch or commit to read (default: origin/v-rust, else v-rust)")
    parser.add_argument("--check", action="store_true", help="exit 1 if CHANGELOG.md is out of date")
    parser.add_argument("--stdout", action="store_true", help="print the changelog instead of writing it")
    parser.add_argument(
        "--commit-title",
        action="store_true",
        help="print the changelog update's commit subject and write nothing",
    )
    args = parser.parse_args(argv)

    root = repo_root()
    ref = resolve_ref(root, args.ref)
    if args.commit_title:
        print(commit_title(root, ref))
        return 0
    sections = collect(root, ref)
    text = render(ref, sections)
    target = root / "CHANGELOG.md"
    if args.stdout:
        sys.stdout.write(text)
        return 0
    current = target.read_text(encoding="utf-8") if target.exists() else ""
    if args.check:
        if current != text:
            print(f"CHANGELOG.md is out of date with {ref}; run scripts/changelog.py", file=sys.stderr)
            return 1
        print(f"CHANGELOG.md is up to date with {ref}")
        return 0
    if current == text:
        print(f"CHANGELOG.md already up to date with {ref}")
        return 0
    target.write_text(text, encoding="utf-8")
    print(f"Wrote CHANGELOG.md from {ref}: {len(sections)} sections")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
