#!/usr/bin/env python3
"""Write one SVG and one PNG of the local branches on top of v-rust."""

from __future__ import annotations

import html
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


def pad_svg(svg: str) -> tuple[str, int, int]:
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
    svg = svg.replace('style="', 'style="background:#ffffff;', 1)
    rect = (
        f'<rect x="{min_x:.2f}" y="{min_y:.2f}" width="{width:.2f}" height="{height:.2f}" fill="#ffffff"/>'
    )
    svg = svg.replace(">", f">{rect}", 1)
    return svg, int(round(width)), int(round(height))


def output_dir() -> Path:
    artifacts = Path("/opt/cursor/artifacts")
    if artifacts.is_dir():
        return artifacts
    path = Path("/tmp/dql-git-graph")
    path.mkdir(parents=True, exist_ok=True)
    return path


def screenshot(html_path: Path, png_path: Path, width: int, height: int) -> None:
    chrome = shutil.which("google-chrome") or shutil.which("chromium") or shutil.which("chromium-browser")
    if chrome is None:
        sys.exit(f"SVG is written, but google-chrome is not installed so {png_path} was not created")
    window_w = max(width + 70, 720)
    window_h = height + 96
    with tempfile.TemporaryDirectory(prefix="git-graph-chrome-") as profile:
        command = [
            chrome,
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--hide-scrollbars",
            "--disable-background-networking",
            "--disable-sync",
            "--no-first-run",
            "--disable-extensions",
            f"--user-data-dir={profile}",
            "--force-device-scale-factor=2",
            f"--window-size={window_w},{window_h}",
            f"--screenshot={png_path}",
            html_path.as_uri(),
        ]
        try:
            subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20)
        except subprocess.TimeoutExpired:
            if not png_path.is_file() or png_path.stat().st_size == 0:
                sys.exit("chrome timed out before writing the PNG")
    if not png_path.is_file() or png_path.stat().st_size == 0:
        sys.exit(f"chrome did not write {png_path}")


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
    detached = head not in tips and is_ancestor(repo, BASE, head)
    if detached:
        refs.append(head)
    svg, width, height = pad_svg(render_svg(repo, refs))
    dest = output_dir()
    svg_path = dest / "v-rust-branches.svg"
    png_path = dest / "v-rust-branches.png"
    svg_path.write_text(svg)
    names = ", ".join(refs[:-1] if detached else refs)
    page = f"""<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  html {{ background: #ffffff; }}
  body {{ margin: 0; background: #ffffff; }}
  .card {{ padding: 16px 20px 18px; }}
  h1 {{ font: 600 16px/1.3 ui-sans-serif, system-ui, sans-serif; color: #1c1917; margin: 0 0 4px; }}
  p {{ font: 13px/1.4 ui-sans-serif, system-ui, sans-serif; color: #57534e; margin: 0 0 12px; }}
  .frame {{ background: white; border: 1px solid #e7e5e4; border-radius: 10px; padding: 6px 10px 8px; display: inline-block; }}
</style>
</head>
<body>
<div class="card">
  <h1>Branches on top of v-rust</h1>
  <p>{html.escape(names)}</p>
  <div class="frame">{svg}</div>
</div>
</body>
</html>
"""
    with tempfile.TemporaryDirectory(prefix="git-graph-page-") as page_dir:
        html_path = Path(page_dir) / "v-rust-branches.html"
        html_path.write_text(page)
        screenshot(html_path, png_path, width, height)
    print(f"png: {png_path}")
    print(f"svg: {svg_path}")


if __name__ == "__main__":
    main()
