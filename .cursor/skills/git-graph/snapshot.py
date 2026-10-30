#!/usr/bin/env python3
"""Write one SVG of origin branches on top of origin/v-rust."""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

BASE = "origin/v-rust"
CUTOFF_SECONDS = 2 * 24 * 60 * 60

_COMMIT_BLOCK = re.compile(
    r"<g>\s*(?:<path[^>]*/>\s*)+<circle[^>]*>\s*<title>(?P<hash>[a-f0-9]{40})</title>\s*</circle>\s*</g>\s*<text[\s\S]*?</text>\s*",
    re.MULTILINE,
)


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


def fetch_origin(repo: Path) -> None:
    proc = git(repo, "fetch", "origin", "--prune", check=False)
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or proc.stdout.strip() or "git fetch origin failed")


def is_ancestor(repo: Path, ancestor: str, descendant: str) -> bool:
    return git(repo, "merge-base", "--is-ancestor", ancestor, descendant, check=False).returncode == 0


def merge_base(repo: Path, left: str, right: str) -> str:
    return git(repo, "merge-base", left, right).stdout.strip()


def commits_above(repo: Path, base: str, tip: str) -> int:
    proc = git(repo, "rev-list", "--count", f"{base}..{tip}", check=False)
    if proc.returncode != 0:
        return 0
    return int(proc.stdout.strip() or 0)


def base_cutoff(repo: Path) -> tuple[int, str]:
    epoch = int(git(repo, "log", "-1", "--format=%ct", BASE).stdout.strip())
    cutoff = epoch - CUTOFF_SECONDS
    since = datetime.fromtimestamp(cutoff, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    return cutoff, since


def branch_on_top(repo: Path, name: str) -> bool:
    if name in {BASE, "origin/HEAD"}:
        return False
    merge = merge_base(repo, BASE, name)
    if not is_ancestor(repo, merge, BASE):
        return False
    return is_ancestor(repo, BASE, name) or commits_above(repo, BASE, name) > 0


def has_recent_commits(repo: Path, name: str, since: str) -> bool:
    proc = git(repo, "rev-list", "-1", f"--since={since}", f"{BASE}..{name}", check=False)
    return proc.returncode == 0 and bool(proc.stdout.strip())


def branches_on_top(repo: Path, since: str) -> list[str]:
    if git(repo, "rev-parse", "--verify", "--quiet", BASE, check=False).returncode != 0:
        sys.exit(f"{BASE} does not exist after fetch")
    listed = git(
        repo,
        "for-each-ref",
        "--sort=-committerdate",
        "--format=%(refname:short)",
        "refs/remotes/origin",
    ).stdout.splitlines()
    selected = [BASE]
    for name in listed:
        if name and branch_on_top(repo, name) and has_recent_commits(repo, name, since):
            selected.append(name)
    return selected


def allowed_commits(repo: Path, tips: list[str], since: str) -> set[str]:
    allowed = {git(repo, "rev-parse", BASE).stdout.strip()}
    for tip in tips:
        proc = git(repo, "rev-list", f"--since={since}", f"{BASE}..{tip}", check=False)
        if proc.returncode != 0:
            continue
        allowed.update(line for line in proc.stdout.splitlines() if line)
        allowed.add(git(repo, "rev-parse", tip).stdout.strip())
    return allowed


def filter_svg_by_commits(svg: str, allowed: set[str]) -> str:
    def repl(match: re.Match[str]) -> str:
        return match.group(0) if match.group("hash") in allowed else ""

    return _COMMIT_BLOCK.sub(repl, svg)


# git-graph draws 12px monospace. Branch labels sit in a translated group.
_CHAR_WIDTH = 7.8
_SVG_TOKEN = re.compile(
    r'<g\b(?P<gattrs>[^>]*)>|</g>|<text\b(?P<tattrs>[^>]*)>\s*(?P<body>.*?)\s*</text>|<circle\b(?P<cattrs>[^>]*?)/?>',
    re.S,
)


def fit_viewbox(svg: str) -> str:
    xs: list[float] = []
    ys: list[float] = []
    stack = [(0.0, 0.0)]
    for match in _SVG_TOKEN.finditer(svg):
        if match.group(0) == "</g>":
            if len(stack) > 1:
                stack.pop()
            continue
        if match.group("gattrs") is not None:
            found = re.search(
                r'transform="translate\(([-\d.]+),\s*([-\d.]+)\)"',
                match.group("gattrs"),
            )
            parent_x, parent_y = stack[-1]
            if found:
                stack.append((parent_x + float(found.group(1)), parent_y + float(found.group(2))))
            else:
                stack.append((parent_x, parent_y))
            continue
        origin_x, origin_y = stack[-1]
        if match.group("cattrs") is not None:
            cx = re.search(r'\bcx="(-?[0-9.]+)"', match.group("cattrs"))
            cy = re.search(r'\bcy="(-?[0-9.]+)"', match.group("cattrs"))
            if cx and cy:
                xs.append(origin_x + float(cx.group(1)))
                ys.append(origin_y + float(cy.group(1)))
            continue
        attrs = match.group("tattrs") or ""
        body = " ".join((match.group("body") or "").split())
        x_match = re.search(r'\bx="(-?[0-9.]+)"', attrs)
        y_match = re.search(r'\by="(-?[0-9.]+)"', attrs)
        if x_match is None or y_match is None or not body:
            continue
        x = origin_x + float(x_match.group(1))
        y = origin_y + float(y_match.group(1))
        xs.extend((x, x + len(body) * _CHAR_WIDTH))
        ys.extend((y - 12.0, y + 4.0))
    if not xs or not ys:
        return svg
    min_x = min(xs) - 16.0
    max_x = max(xs) + 16.0
    min_y = min(ys) - 12.0
    max_y = max(ys) + 12.0
    width = max_x - min_x
    height = max_y - min_y
    svg = re.sub(
        r'viewBox="[^"]+"',
        f'viewBox="{min_x:.2f} {min_y:.2f} {width:.2f} {height:.2f}"',
        svg,
        count=1,
    )
    svg = re.sub(r'\bwidth="[^"]+"', f'width="{width:.2f}"', svg, count=1)
    svg = re.sub(r'\bheight="[^"]+"', f'height="{height:.2f}"', svg, count=1)
    return svg


def render_svg(repo: Path, refs: list[str]) -> str:
    tips = [ref for ref in refs if ref != BASE]
    command = ["git-graph", "--svg", "-m", "none", "-s", "round"]
    if tips:
        command.extend([BASE, *tips])
    else:
        # A lone base ref would otherwise draw the full ancestor history.
        command.extend(["-n", "1", BASE])
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


def main() -> None:
    if shutil.which("git-graph") is None:
        sys.exit(
            "git-graph is not installed. From /tmp, run: cargo install git-graph\n"
            "If Cargo rejects edition2024, run: rustup toolchain install stable "
            "&& cargo +stable install git-graph\n"
            "Do not add git-graph to this repository."
        )
    repo = repo_root()
    fetch_origin(repo)
    _cutoff, since = base_cutoff(repo)
    refs = branches_on_top(repo, since)
    tips = [ref for ref in refs if ref != BASE]
    allowed = allowed_commits(repo, tips, since)
    svg = filter_svg_by_commits(render_svg(repo, refs), allowed)
    svg = fit_viewbox(svg)
    svg_path = output_dir() / "v-rust-branches.svg"
    svg_path.write_text(pad_svg(svg))
    print(f"svg: {svg_path}")


if __name__ == "__main__":
    main()
