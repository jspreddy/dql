#!/usr/bin/env python3
"""Write one SVG of the local branches on top of v-rust."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

BASE = "v-rust"


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=repo,
        check=check,
        text=True,
        capture_output=True,
    )


def repo_root() -> Path:
    proc = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or "not a git repository")
    return Path(proc.stdout.strip())


def is_ancestor(repo: Path, ancestor: str, descendant: str) -> bool:
    return git(repo, "merge-base", "--is-ancestor", ancestor, descendant, check=False).returncode == 0


def branches_on_top(repo: Path) -> list[str]:
    if git(repo, "rev-parse", "--verify", "--quiet", f"refs/heads/{BASE}", check=False).returncode != 0:
        sys.exit(f"local branch {BASE} does not exist")
    listed = git(
        repo,
        "for-each-ref",
        "--sort=-committerdate",
        "--format=%(refname:short)",
        "refs/heads",
    ).stdout.splitlines()
    selected = [BASE]
    for name in listed:
        if name and name != BASE and commits_since_base(repo, name):
            selected.append(name)
    return selected


def commits_since_base(repo: Path, branch: str) -> list[str]:
    """Commits reachable from `branch` and not reachable from v-rust, oldest first."""
    listed = git(repo, "rev-list", "--reverse", "--topo-order", branch, "--not", BASE).stdout.splitlines()
    return [line for line in listed if line]


def render_svg(repo: Path, refs: list[str]) -> str:
    command = ["git-graph", "--svg", "-l", "-m", "none", "-s", "round"]
    # A lone v-rust has no upstream, so git-graph walks every ancestor.
    # One commit is the whole "since v-rust" picture when nothing is ahead.
    if refs == [BASE]:
        command.extend(["--max-count", "1"])
    command.extend(refs)
    proc = subprocess.run(command, cwd=repo, text=True, capture_output=True)
    if proc.returncode != 0 and "owned by current user" in proc.stderr:
        command.insert(1, "--skip-repo-owner-validation")
        proc = subprocess.run(command, cwd=repo, text=True, capture_output=True)
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or proc.stdout.strip() or "git-graph failed")
    svg = proc.stdout[proc.stdout.find("<svg") :]
    if not svg.startswith("<svg"):
        sys.exit(proc.stderr.strip() or "git-graph did not write an SVG")
    return svg


def pad_svg(svg: str) -> str:
    match = re.search(r'viewBox="([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+)"', svg)
    if match is None:
        sys.exit("git-graph SVG has no viewBox")
    min_x, min_y, width, height = (float(part) for part in match.groups())
    pad_x, pad_y = 18.0, 12.0
    min_x -= pad_x
    min_y -= pad_y
    width += pad_x * 2 + 16
    height += pad_y * 2
    svg = re.sub(
        r'viewBox="[^"]+"',
        f'viewBox="{min_x:.2f} {min_y:.2f} {width:.2f} {height:.2f}"',
        svg,
        count=1,
    )
    svg = re.sub(r'\bwidth="[^"]+"', f'width="{width:.2f}"', svg, count=1)
    svg = re.sub(r'\bheight="[^"]+"', f'height="{height:.2f}"', svg, count=1)
    return color_head(paint_white(svg, min_x, min_y, width, height))


_HEAD_LABEL = re.compile(
    r"<text(?P<text_attrs>[^>]*)>\s*HEAD\s*</text>\s*<path(?P<path_attrs>[^>]*)/>",
)


def color_head(svg: str) -> str:
    def repl(match: re.Match[str]) -> str:
        text_attrs = match.group("text_attrs")
        path_attrs = re.sub(r'\bfill="[^"]*"', 'fill="#93c5fd"', match.group("path_attrs"))
        if re.search(r'\bfill="', text_attrs):
            text_attrs = re.sub(r'\bfill="[^"]*"', 'fill="#1d4ed8"', text_attrs)
        else:
            text_attrs += ' fill="#1d4ed8"'
        return f"<path{path_attrs}/><text{text_attrs}>\nHEAD\n</text>"

    return _HEAD_LABEL.sub(repl, svg, count=1)


def paint_white(svg: str, min_x: float, min_y: float, width: float, height: float) -> str:
    if 'style="' in svg:
        svg = svg.replace('style="', 'style="background:#ffffff;', 1)
    else:
        svg = svg.replace("<svg ", '<svg style="background:#ffffff" ', 1)
    rect = (
        f'<rect x="{min_x:.2f}" y="{min_y:.2f}" width="{width:.2f}" height="{height:.2f}" fill="#ffffff"/>'
    )
    return svg.replace(">", f">{rect}", 1)


def output_dir() -> Path:
    artifacts = Path("/opt/cursor/artifacts")
    if artifacts.is_dir():
        return artifacts
    path = Path("/tmp/dql-git-graph")
    path.mkdir(parents=True, exist_ok=True)
    return path


def graph_repo(repo: Path, refs: list[str]) -> tuple[Path, list[str]]:
    """Repository whose history below v-rust is not walked.

    git-graph stops at the merge-base of the refs it is given. A branch that
    forked from an ancestor of v-rust would drag that older history into the
    picture. Reparent those commits onto v-rust in a temporary clone so the
    merge-base is v-rust and only the new commits remain.
    """
    branches = [ref for ref in refs if ref != BASE]
    if not branches or all(is_ancestor(repo, BASE, branch) for branch in branches):
        return repo, refs
    clone = Path(tempfile.mkdtemp(prefix="dql-git-graph-"))
    subprocess.run(
        ["git", "clone", "--shared", "--quiet", str(repo), str(clone)],
        check=True,
    )
    git(clone, "checkout", "--detach")
    git(clone, "branch", "-f", BASE, git(repo, "rev-parse", BASE).stdout.strip())
    mapping = reparent_onto_base(repo, clone, branches)
    point_head(repo, clone, mapping)
    rewritten = [BASE]
    for ref in branches:
        tip = git(repo, "rev-parse", ref).stdout.strip()
        rewritten.append(ref if git(repo, "show-ref", "--verify", "--quiet", f"refs/heads/{ref}", check=False).returncode == 0 else mapping.get(tip, tip))
    return clone, rewritten


def reparent_onto_base(repo: Path, clone: Path, branches: list[str]) -> dict[str, str]:
    listed = git(
        repo,
        "rev-list",
        "--reverse",
        "--topo-order",
        *branches,
        "--not",
        BASE,
    ).stdout.splitlines()
    base = git(repo, "rev-parse", BASE).stdout.strip()
    pending = {oid for oid in listed if oid}
    mapping: dict[str, str] = {}
    for oid in listed:
        if not oid or oid in mapping:
            continue
        parents = git(repo, "rev-list", "--parents", "-n", "1", oid).stdout.split()[1:]
        new_parents: list[str] = []
        for parent in parents:
            mapped = mapping.get(parent)
            if mapped is None and parent in pending:
                sys.exit(f"commit {oid} lists {parent} before it was copied")
            if mapped is None:
                mapped = base
            if mapped not in new_parents:
                new_parents.append(mapped)
        if not new_parents:
            new_parents = [base]
        mapping[oid] = commit_copy(repo, clone, oid, new_parents)
    for branch in branches:
        if git(repo, "show-ref", "--verify", "--quiet", f"refs/heads/{branch}", check=False).returncode != 0:
            continue
        tip = git(repo, "rev-parse", branch).stdout.strip()
        git(clone, "branch", "-f", branch, mapping.get(tip, tip))
    return mapping


def commit_copy(repo: Path, clone: Path, oid: str, parents: list[str]) -> str:
    tree = git(repo, "rev-parse", f"{oid}^{{tree}}").stdout.strip()
    message = git(repo, "log", "-1", "--format=%B", oid).stdout
    author = git(repo, "log", "-1", "--format=%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI", oid).stdout.split(
        "\0"
    )
    env = os.environ.copy()
    if len(author) == 6:
        env.update(
            {
                "GIT_AUTHOR_NAME": author[0],
                "GIT_AUTHOR_EMAIL": author[1],
                "GIT_AUTHOR_DATE": author[2],
                "GIT_COMMITTER_NAME": author[3],
                "GIT_COMMITTER_EMAIL": author[4],
                "GIT_COMMITTER_DATE": author[5],
            }
        )
    command = ["git", "commit-tree", tree]
    for parent in parents:
        command.extend(["-p", parent])
    proc = subprocess.run(
        command,
        cwd=clone,
        input=message,
        text=True,
        capture_output=True,
        check=False,
        env=env,
    )
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or f"git commit-tree failed for {oid}")
    return proc.stdout.strip()


def point_head(repo: Path, clone: Path, mapping: dict[str, str]) -> None:
    name = git(repo, "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if name != "HEAD":
        git(clone, "symbolic-ref", "HEAD", f"refs/heads/{name}")
        return
    head = git(repo, "rev-parse", "HEAD").stdout.strip()
    git(clone, "checkout", "--detach", mapping.get(head, head))


def main() -> None:
    if shutil.which("git-graph") is None:
        sys.exit(
            "git-graph is not installed. From /tmp, run: cargo install git-graph\n"
            "If Cargo rejects edition2024, run: rustup toolchain install stable "
            "&& cargo +stable install git-graph\n"
            "Do not add git-graph to this repository."
        )
    repo = repo_root()
    refs = branches_on_top(repo)
    head_name = git(repo, "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    head = git(repo, "rev-parse", "HEAD").stdout.strip()
    tips = {git(repo, "rev-parse", ref).stdout.strip() for ref in refs}
    if head not in tips and head_name == "HEAD" and commits_since_base(repo, head):
        refs.append(head)
    graph, refs = graph_repo(repo, refs)
    try:
        svg = pad_svg(render_svg(graph, refs))
    finally:
        if graph != repo:
            shutil.rmtree(graph, ignore_errors=True)
    svg_path = output_dir() / "v-rust-branches.svg"
    svg_path.write_text(svg)
    print(f"svg: {svg_path}")


if __name__ == "__main__":
    main()
