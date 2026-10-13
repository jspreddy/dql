#!/usr/bin/env python3
"""Write one SVG of the local branches on top of v-rust."""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
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
        if name and name != BASE and is_ancestor(repo, BASE, name):
            selected.append(name)
    return selected


def render_svg(repo: Path, refs: list[str]) -> str:
    command = ["git-graph", "--svg", "-l", "-m", "none", "-s", "round", *refs]
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
    return paint_white(svg, min_x, min_y, width, height)


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
    head = git(repo, "rev-parse", "HEAD").stdout.strip()
    tips = {git(repo, "rev-parse", ref).stdout.strip() for ref in refs}
    if head not in tips and is_ancestor(repo, BASE, head):
        refs.append(head)
    svg_path = output_dir() / "v-rust-branches.svg"
    svg_path.write_text(pad_svg(render_svg(repo, refs)))
    print(f"svg: {svg_path}")


if __name__ == "__main__":
    main()
