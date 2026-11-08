#!/usr/bin/env python3
"""Write one SVG of origin branches on top of origin/v-rust."""

from __future__ import annotations

import argparse
import re
import secrets
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

BASE = "origin/v-rust"
_DURATION = re.compile(r"^(\d+)([wdhms])?$", re.IGNORECASE)
_DURATION_UNITS = {
    "": 1,
    "s": 1,
    "m": 60,
    "h": 60 * 60,
    "d": 24 * 60 * 60,
    "w": 7 * 24 * 60 * 60,
}

_TAG = re.compile(r"<(/?)([A-Za-z][\w:.-]*)([^>]*)>")
_TITLE = re.compile(r"<title>([a-f0-9]{40})</title>")
_NUMBER = r"[+-]?\d+(?:\.\d+)?"
_SUMMARY_TEXT = re.compile(
    r'(<text\b[^>]*font-family:monospace[^>]*>)(\s*)(.*?)(\s*)(</text>)',
    re.DOTALL,
)
# git-graph sizes summary text as len(text) * font-size * 0.6.
_SUMMARY_CHAR_WIDTH = 12.0 * 0.6
_MESSAGE_MAX = 79


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        ["git", *args],
        cwd=repo,
        check=False,
        text=True,
        capture_output=True,
    )
    if check and proc.returncode != 0:
        sys.exit(proc.stderr.strip() or proc.stdout.strip() or f"git {' '.join(args)} failed")
    return proc


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
    proc = git(repo, "fetch", "origin", check=False)
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or proc.stdout.strip() or "git fetch origin failed")


def remote_branches(repo: Path) -> list[str]:
    proc = git(repo, "-c", "color.ui=never", "branch", "-r", "-l", "--no-merge")
    names: list[str] = []
    for line in proc.stdout.splitlines():
        name = line.strip()
        if not name or " -> " in name:
            continue
        names.append(name.split()[0])
    return names


def current_branch(repo: Path) -> str:
    return git(repo, "branch", "--show-current").stdout.strip()


def graph_refs(repo: Path) -> list[str]:
    if git(repo, "rev-parse", "--verify", "--quiet", BASE, check=False).returncode != 0:
        sys.exit(f"{BASE} does not exist after fetch")
    refs = [BASE]
    seen = {BASE, "origin/HEAD"}
    for name in remote_branches(repo):
        if name in seen:
            continue
        seen.add(name)
        refs.append(name)
    current = current_branch(repo)
    if current and current not in seen:
        refs.append(current)
    return refs


def cutoff_before(repo: Path, value: str) -> str:
    """Return a git --before date for a duration before origin/v-rust, or the date itself."""
    match = _DURATION.fullmatch(value.strip())
    if match is None:
        return value
    amount = int(match.group(1))
    if amount < 1:
        sys.exit("cutoff must be positive")
    seconds = amount * _DURATION_UNITS[(match.group(2) or "").lower()]
    epoch = int(git(repo, "log", "-1", "--format=%ct", BASE).stdout.strip())
    cutoff = epoch - seconds
    return datetime.fromtimestamp(cutoff, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def commits_before(repo: Path, date: str, refs: list[str]) -> set[str]:
    proc = git(repo, "rev-list", f"--before={date}", *refs, check=False)
    if proc.returncode != 0:
        sys.exit(proc.stderr.strip() or proc.stdout.strip() or f"invalid date {date!r}")
    return {line.strip() for line in proc.stdout.splitlines() if line.strip()}


def _element_end(svg: str, start: int) -> int:
    close = svg.find(">", start)
    if close < 0:
        raise ValueError("unclosed tag")
    opening = svg[start : close + 1]
    if opening.startswith("</") or opening.endswith("/>"):
        return close + 1
    match = re.match(r"<([A-Za-z][\w:.-]*)", opening)
    if match is None:
        return close + 1
    name = match.group(1)
    depth = 1
    for found in _TAG.finditer(svg, close + 1):
        slash, tag, attrs = found.group(1), found.group(2), found.group(3)
        if tag != name:
            continue
        if slash:
            depth -= 1
            if depth == 0:
                return found.end()
        elif not attrs.rstrip().endswith("/"):
            depth += 1
    raise ValueError(f"unclosed <{name}>")


def _top_level_elements(inner: str) -> list[str]:
    items: list[str] = []
    index = 0
    length = len(inner)
    while index < length:
        if inner[index] != "<":
            nxt = inner.find("<", index)
            if nxt < 0:
                nxt = length
            items.append(inner[index:nxt])
            index = nxt
            continue
        end = _element_end(inner, index)
        items.append(inner[index:end])
        index = end
    return items


def _commit_hash(element: str) -> str | None:
    if not element.startswith("<g"):
        return None
    match = _TITLE.search(element)
    return match.group(1) if match else None


def prune_svg_commits(svg: str, remove: set[str]) -> str:
    """Drop commits in `remove`, including the lines drawn with those commits."""
    if not remove:
        return svg
    start = svg.find("<svg")
    open_end = svg.find(">", start)
    close = svg.rfind("</svg>")
    if start < 0 or open_end < 0 or close < 0:
        sys.exit("git-graph SVG has no svg element")
    items = _top_level_elements(svg[open_end + 1 : close])
    kept: list[str] = []
    index = 0
    while index < len(items):
        digest = _commit_hash(items[index])
        if digest is None:
            kept.append(items[index])
            index += 1
            continue
        nxt = index + 1
        while nxt < len(items) and _commit_hash(items[nxt]) is None:
            nxt += 1
        if digest not in remove:
            kept.extend(items[index:nxt])
        index = nxt
    return svg[: open_end + 1] + "".join(kept) + svg[close:]


def fit_viewbox(svg: str) -> str:
    """Shrink the vertical frame after commits are removed.

    The horizontal frame stays as git-graph drew it, so branch labels and
    commit summaries keep the width already reserved for them.
    """
    match = re.search(r'viewBox="([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+)"', svg)
    if match is None:
        return svg
    min_x, _min_y, width, _height = (float(part) for part in match.groups())
    ys = [float(value) for value in re.findall(rf'\b(?:cy|y1|y2)="({_NUMBER})"', svg)]
    if not ys:
        return svg
    min_y = min(ys) - 20.0
    max_y = max(ys) + 20.0
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


def _clip_message(text: str) -> str:
    if len(text) <= _MESSAGE_MAX:
        return text
    return text[: _MESSAGE_MAX - 3].rstrip() + "..."


def _shrink_width(svg: str, delta: float) -> str:
    if delta <= 0:
        return svg
    match = re.search(r'viewBox="([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+)"', svg)
    if match is None:
        return svg
    min_x, min_y, width, height = (float(part) for part in match.groups())
    width = max(width - delta, 1.0)
    svg = re.sub(
        r'viewBox="[^"]+"',
        f'viewBox="{min_x:.2f} {min_y:.2f} {width:.2f} {height:.2f}"',
        svg,
        count=1,
    )

    def width_repl(found: re.Match[str]) -> str:
        return f'width="{max(float(found.group(1)) - delta, 1.0):.2f}"'

    return re.sub(r'\bwidth="([-\d.]+)"', width_repl, svg, count=1)


def shorten_commit_messages(svg: str) -> str:
    """Keep each commit subject under 80 characters and drop the unused width."""
    longest = 0
    clipped_longest = 0

    def repl(match: re.Match[str]) -> str:
        nonlocal longest, clipped_longest
        body = match.group(3).strip()
        clipped = _clip_message(body)
        longest = max(longest, len(body))
        clipped_longest = max(clipped_longest, len(clipped))
        if clipped == body:
            return match.group(0)
        return f"{match.group(1)}\n{clipped}\n{match.group(5)}"

    svg = _SUMMARY_TEXT.sub(repl, svg)
    return _shrink_width(svg, (longest - clipped_longest) * _SUMMARY_CHAR_WIDTH)


def render_svg(repo: Path, refs: list[str], count: int | None) -> str:
    command = ["git-graph", "--svg", "-m", "none", "-s", "round"]
    if count is not None:
        command.extend(["-n", str(count)])
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


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-n",
        "--count",
        type=int,
        metavar="count",
        help="Limit git-graph to this many commits. Visual styling may change; commits and lines stay.",
    )
    parser.add_argument(
        "-d",
        "--cutoff",
        metavar="date",
        help=(
            "Prune commits before this date. A git --before date, or a duration before "
            f"{BASE} such as 2w, 14d, 48h, or 30m. Can be combined with --count."
        ),
    )
    args = parser.parse_args(argv)
    if args.count is not None and args.count < 1:
        parser.error("count must be a positive integer")
    return args


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    if shutil.which("git-graph") is None:
        sys.exit(
            "git-graph is not installed. From /tmp, run: cargo install git-graph\n"
            "If Cargo rejects edition2024, run: rustup toolchain install stable "
            "&& cargo +stable install git-graph\n"
            "Do not add git-graph to this repository."
        )
    repo = repo_root()
    fetch_origin(repo)
    refs = graph_refs(repo)
    svg = render_svg(repo, refs, args.count)
    if args.cutoff is not None:
        svg = prune_svg_commits(svg, commits_before(repo, cutoff_before(repo, args.cutoff), refs))
    svg = shorten_commit_messages(svg)
    if args.cutoff is not None:
        svg = fit_viewbox(svg)
    svg_path = output_dir() / f"v-rust-branches-{secrets.token_hex(4)}.svg"
    svg_path.write_text(pad_svg(svg))
    print(f"svg: {svg_path}")


if __name__ == "__main__":
    main()
