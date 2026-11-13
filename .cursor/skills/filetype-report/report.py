#!/usr/bin/env python3
"""Write an HTML report and a PNG of the file types checked into git.

Every path from `git ls-files` is grouped by extension, or by file name when it has
none. For each group the report shows files, lines, and bytes, and splits vendored
code (paths under --vendor) from code written in this repository. The PNG is a
full-page screenshot rendered with headless Chrome.
"""

from __future__ import annotations

import argparse
import collections
import html
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

DEFAULT_TOP = 10
DEFAULT_VENDOR = ("dqlrs-web/static/vendor/",)
WIDTH = 1280

LABELS = {
    ".rs": "Rust", ".js": "JavaScript", ".mjs": "JS modules", ".ts": "TypeScript",
    ".py": "Python", ".md": "Markdown", ".mdc": "Cursor rules", ".html": "HTML",
    ".css": "CSS", ".lock": "Lock files", ".dql": "DQL", ".json": "JSON",
    ".rst": "reStructuredText", ".sh": "Shell", ".csv": "CSV", ".toml": "TOML",
    ".ipynb": "Notebooks", ".yml": "YAML", ".yaml": "YAML", ".svg": "SVG",
    ".png": "PNG", ".jpg": "JPEG", ".jpeg": "JPEG", ".gif": "GIF",
}
# Types counted as programming languages for the "largest hand-written language" note.
CODE = {".rs", ".js", ".mjs", ".ts", ".py", ".css", ".sh", ".go", ".java", ".c", ".h", ".cpp"}


@dataclass
class Group:
    files: int = 0
    lines: int = 0
    bytes: int = 0
    binary: int = 0
    vfiles: int = 0
    vlines: int = 0
    vbytes: int = 0


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True, text=True
    ).stdout


def repo_root() -> Path:
    try:
        return Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    except subprocess.CalledProcessError:
        sys.exit("Run this from inside a git repository.")


def group_key(path: str) -> str:
    base = os.path.basename(path)
    root, ext = os.path.splitext(base)
    # Dotfiles such as .gitignore and names such as Justfile keep their whole name.
    return ext.lower() if ext and root else base


def count_lines(data: bytes) -> int:
    return data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0)


def collect(repo: Path, vendor: tuple[str, ...]) -> tuple[dict[str, Group], int, dict]:
    paths = [p for p in git(repo, "ls-files", "-z").split("\0") if p]
    groups: dict[str, Group] = collections.defaultdict(Group)
    symlinks = 0
    largest = ("", 0)
    for rel in paths:
        full = repo / rel
        if full.is_symlink():
            symlinks += 1
            continue
        if not full.is_file():  # deleted in the working tree but still in the index
            continue
        data = full.read_bytes()
        group = groups[group_key(rel)]
        vendored = rel.startswith(vendor)
        group.files += 1
        group.bytes += len(data)
        if vendored:
            group.vfiles += 1
            group.vbytes += len(data)
        if len(data) > largest[1]:
            largest = (rel, len(data))
        if b"\0" in data[:8192]:
            group.binary += 1
            continue
        lines = count_lines(data)
        group.lines += lines
        if vendored:
            group.vlines += lines
    meta = {
        "head": git(repo, "log", "-1", "--format=%h · %ad", "--date=short").strip(),
        "branch": git(repo, "rev-parse", "--abbrev-ref", "HEAD").strip(),
        "name": repo_name(repo),
        "largest": largest,
    }
    return dict(groups), symlinks, meta


def repo_name(repo: Path) -> str:
    try:
        url = git(repo, "remote", "get-url", "origin").strip()
    except subprocess.CalledProcessError:
        return repo.name
    found = re.search(r"[:/]([^/:]+/[^/]+?)(?:\.git)?$", url)
    return found.group(1) if found else repo.name


def size(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f} MB"
    if n >= 1_000:
        return f"{n / 1_000:.0f} KB"
    return f"{n} B"


def num(n: int) -> str:
    return f"{n:,}"


def esc(text: str) -> str:
    return html.escape(text, quote=True)


def label(key: str) -> str:
    ext = f"<span class=ext>{esc(key)}</span>"
    return f"{esc(LABELS[key])} {ext}" if key in LABELS else ext


def bar_rows(groups: dict[str, Group], metric: str, fmt, top: int) -> str:
    vmetric = "v" + metric
    ordered = sorted(groups.items(), key=lambda kv: -getattr(kv[1], metric))
    shown, rest = ordered[:top], ordered[top:]
    items = [(label(k), getattr(g, metric), getattr(g, vmetric)) for k, g in shown]
    if rest:
        items.append((
            f"Other <span class=ext>{len(rest)} types</span>",
            sum(getattr(g, metric) for _, g in rest),
            sum(getattr(g, vmetric) for _, g in rest),
        ))
    peak = max((value for _, value, _ in items), default=0) or 1
    rows = []
    for text, value, vendored in items:
        segments = f'<span class="seg own" style="width:{(value - vendored) / peak * 100:.2f}%"></span>'
        if vendored:
            segments += f'<span class="seg vend" style="width:{vendored / peak * 100:.2f}%"></span>'
        tip = f"{fmt(value)} total" + (f", {fmt(vendored)} vendored" if vendored else "")
        rows.append(
            f'<div class=row title="{esc(tip)}"><div class=lab>{text}</div>'
            f'<div class=track>{segments}</div><div class=val>{fmt(value)}</div></div>'
        )
    return "\n".join(rows)


def table_html(groups: dict[str, Group], total_lines: int, total_bytes: int) -> str:
    def row(key: str, g: Group) -> str:
        lines = '<span class=muted>binary</span>' if g.binary == g.files and g.files else num(g.lines)
        share = '<span class=muted>—</span>' if g.binary == g.files and g.files else f"{g.lines / max(total_lines, 1) * 100:.1f}%"
        return (
            f"<tr><td>{label(key)}</td><td>{num(g.files)}</td><td>{lines}</td><td>{share}</td>"
            f"<td>{size(g.bytes)}</td><td>{g.bytes / max(total_bytes, 1) * 100:.1f}%</td></tr>"
        )

    ordered = sorted(groups.items(), key=lambda kv: (-kv[1].lines, -kv[1].bytes, kv[0]))
    half = (len(ordered) + 1) // 2
    head = ("<table><thead><tr><th>Type</th><th>Files</th><th>Lines</th><th>% lines</th>"
            "<th>Size</th><th>% size</th></tr></thead><tbody>")
    return "".join(
        head + "\n".join(row(k, g) for k, g in part) + "</tbody></table>"
        for part in (ordered[:half], ordered[half:]) if part
    )


def notes(groups: dict[str, Group], meta: dict) -> str:
    parts = []
    vendored = [(k, g) for k, g in groups.items() if g.vlines]
    if vendored:
        key, g = max(vendored, key=lambda kv: kv[1].vlines)
        parts.append(
            f"<b>{esc(LABELS.get(key, key))}:</b> {num(g.vlines)} of its {num(g.lines)} lines "
            f"({g.vlines / g.lines * 100:.0f}%) and {size(g.vbytes)} of its {size(g.bytes)} are vendored."
        )
    code = [(k, g.lines - g.vlines) for k, g in groups.items() if k in CODE]
    if code:
        key, own = max(code, key=lambda kv: kv[1])
        parts.append(f"<b>Largest hand-written language:</b> {esc(LABELS.get(key, key))}, {num(own)} lines.")
    if meta["largest"][0]:
        path, n = meta["largest"]
        parts.append(f"<b>Largest file:</b> <code>{esc(path)}</code>, {size(n)}.")
    return " ".join(parts)


CSS = """
:root { --canvas:#0e141b; --paper:#17202b; --raised:#1c2836; --line:#2a3848; --line-soft:#243140;
  --text:#e7eef6; --strong:#f5f9fc; --secondary:#c5d2e0; --muted:#8b9eb3;
  --own:#0f9f8f; --vend:#4a5d72; --bar:#0c1117; --brand:#0f766e; }
* { box-sizing:border-box; margin:0 }
html, body { background:var(--canvas) }
body { color:var(--text); font:14px/1.45 "Noto Sans","DejaVu Sans",sans-serif; width:%(width)dpx; overflow:hidden }
code { font-family:ui-monospace,Menlo,"DejaVu Sans Mono",monospace; font-size:.92em }
.top { height:40px; display:flex; align-items:center; background:var(--bar) }
.brand { height:100%%; display:flex; align-items:center; padding:0 46px 0 22px; background:var(--brand); color:#fff;
  font-weight:750; letter-spacing:.02em; clip-path:polygon(0 0,100%% 0,calc(100%% - 30px) 100%%,0 100%%) }
.top .meta { margin-left:16px; color:var(--muted); font-size:13px }
main { padding:26px 32px 32px }
h1 { font-size:26px; font-weight:750; color:var(--strong); letter-spacing:-.01em }
.sub { color:var(--secondary); margin-top:4px }
.tiles { display:grid; grid-template-columns:repeat(4,1fr); gap:14px; margin-top:20px }
.tile, .card { background:var(--paper); border:1px solid var(--line); border-radius:10px }
.tile { padding:14px 16px }
.tile .k { color:var(--muted); font-size:12.5px; font-weight:650; text-transform:uppercase; letter-spacing:.05em }
.tile .v { font-size:30px; font-weight:750; color:var(--strong); margin-top:2px; font-variant-numeric:tabular-nums }
.tile .n { color:var(--secondary); font-size:12.5px }
.charts { display:grid; grid-template-columns:1fr 1fr; gap:14px; margin-top:14px }
.card { padding:16px 18px 14px }
.card h2 { font-size:15px; font-weight:750; color:var(--strong) }
.card .note { color:var(--muted); font-size:12.5px; margin:2px 0 12px }
.legend { display:flex; gap:16px; color:var(--secondary); font-size:12.5px; margin-bottom:10px }
.legend i { display:inline-block; width:10px; height:10px; border-radius:3px; margin-right:6px; vertical-align:-1px }
.row { display:grid; grid-template-columns:170px 1fr 74px; align-items:center; gap:10px; height:24px }
.lab { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; font-size:13px }
.ext { color:var(--muted); font-family:ui-monospace,Menlo,"DejaVu Sans Mono",monospace; font-size:12px }
.track { display:flex; gap:2px; height:12px; border-left:1px solid var(--line) }
.seg { height:100%%; display:block }
.seg.own { background:var(--own) } .seg.vend { background:var(--vend) }
.track .seg:last-child { border-radius:0 4px 4px 0 }
.val { text-align:right; color:var(--secondary); font-variant-numeric:tabular-nums; font-size:13px }
.callout { margin-top:14px; background:var(--raised); border:1px solid var(--line); border-left:3px solid var(--vend);
  border-radius:8px; padding:10px 14px; color:var(--secondary); font-size:13px }
.callout b { color:var(--strong) }
.tables { display:grid; grid-template-columns:1fr 1fr; gap:20px; align-items:start }
table { width:100%%; border-collapse:collapse; margin-top:4px; font-size:13px; font-variant-numeric:tabular-nums }
th { text-align:right; color:var(--muted); font-weight:650; font-size:12px; text-transform:uppercase; letter-spacing:.04em;
  padding:6px 8px; border-bottom:1px solid var(--line) }
td { text-align:right; padding:4px 8px; border-bottom:1px solid var(--line-soft); color:var(--secondary) }
th:first-child, td:first-child { text-align:left; color:var(--text) }
.muted { color:var(--muted) }
"""


def page(groups: dict[str, Group], symlinks: int, meta: dict, top: int, vendor: tuple[str, ...]) -> str:
    total = Group(
        files=sum(g.files for g in groups.values()),
        lines=sum(g.lines for g in groups.values()),
        bytes=sum(g.bytes for g in groups.values()),
        vlines=sum(g.vlines for g in groups.values()),
        vbytes=sum(g.vbytes for g in groups.values()),
    )
    vendor_text = ", ".join(v.rstrip("/") for v in vendor) or "none"
    legend = (
        '<div class=legend><span><i style="background:var(--own)"></i>Written here</span>'
        f'<span><i style="background:var(--vend)"></i>Vendored ({esc(vendor_text)})</span></div>'
    )
    callout = notes(groups, meta)
    return f"""<!doctype html><html lang=en><head><meta charset=utf-8><title>Repo file types</title>
<link rel=stylesheet href="https://fonts.googleapis.com/css2?family=Noto+Sans:wght@400;500;650;750&display=swap">
<style>{CSS % {"width": WIDTH}}</style></head><body>
<div class=top><div class=brand>{esc(meta["name"].split("/")[-1].upper())}</div>
<div class=meta>{esc(meta["name"])} · {esc(meta["branch"])} · {esc(meta["head"])}</div></div>
<main>
<h1>File types checked into git</h1>
<p class=sub>Every path from <code>git ls-files</code>, grouped by extension (or by name when there is none).
Lines are newline counts; binary files count bytes only.</p>
<div class=tiles>
 <div class=tile><div class=k>Tracked paths</div><div class=v>{num(total.files + symlinks)}</div>
  <div class=n>{num(total.files)} files · {num(symlinks)} symlinks</div></div>
 <div class=tile><div class=k>File types</div><div class=v>{len(groups)}</div>
  <div class=n>extensions and extensionless names</div></div>
 <div class=tile><div class=k>Lines</div><div class=v>{num(total.lines)}</div>
  <div class=n>{num(total.lines - total.vlines)} written here · {num(total.vlines)} vendored</div></div>
 <div class=tile><div class=k>Size</div><div class=v>{size(total.bytes)}</div>
  <div class=n>{size(total.bytes - total.vbytes)} written here · {size(total.vbytes)} vendored</div></div>
</div>
<div class=charts>
 <div class=card><h2>Lines by file type</h2><p class=note>Top {top} types by line count; the rest fold into Other.</p>
  {legend}{bar_rows(groups, "lines", num, top)}</div>
 <div class=card><h2>Size by file type</h2><p class=note>Top {top} types by bytes on disk; the rest fold into Other.</p>
  {legend}{bar_rows(groups, "bytes", size, top)}</div>
</div>
{f'<div class=callout>{callout}</div>' if callout else ''}
<div class=card style="margin-top:14px"><h2>All {len(groups)} file types</h2><p class=note>Sorted by lines, then size.</p>
<div class=tables>{table_html(groups, total.lines, total.bytes)}</div></div>
</main>
<script>document.body.dataset.height = Math.ceil(document.documentElement.scrollHeight);</script>
</body></html>"""


_CHROME_NAMES = ("google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome")
_CHROME_PATHS = (
    "/opt/pw-browsers/chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
)


def find_chrome() -> str:
    for name in _CHROME_NAMES:
        found = shutil.which(name)
        if found:
            return found
    for path in _CHROME_PATHS:
        if Path(path).is_file():
            return path
    sys.exit("No Chrome or Chromium found to render the PNG. Install one, or put it on PATH.")


def chrome(*args: str) -> str:
    # No --user-data-dir: a blank profile makes headless Chrome hang on first-run work.
    command = [find_chrome(), "--headless", "--disable-gpu", "--hide-scrollbars",
               "--virtual-time-budget=5000", *args]
    # Chrome refuses to run as root unless its sandbox is off (containers, cloud sessions).
    if hasattr(os, "geteuid") and os.geteuid() == 0:
        command.insert(1, "--no-sandbox")
    # Log to a file, not a pipe: Chrome helper processes can hold a pipe open after exit.
    with tempfile.TemporaryFile(mode="w+") as out, tempfile.TemporaryFile(mode="w+") as err:
        try:
            proc = subprocess.run(command, check=False, stdout=out, stderr=err, timeout=120)
        except subprocess.TimeoutExpired:
            sys.exit("Chrome did not finish within 120 seconds")
        out.seek(0)
        err.seek(0)
        stdout, stderr = out.read(), err.read()
    if proc.returncode != 0:
        sys.exit(stderr[-2000:] or "Chrome failed")
    return stdout


def render_png(html_path: Path) -> Path:
    """Measure the page in one headless pass, then screenshot it at that exact height."""
    uri = html_path.resolve().as_uri()
    dom = chrome(f"--window-size={WIDTH},1000", "--dump-dom", uri)
    found = re.search(r'data-height="(\d+)"', dom)
    height = int(found.group(1)) if found else 3000
    png_path = html_path.with_suffix(".png")
    png_path.unlink(missing_ok=True)
    chrome("--force-device-scale-factor=2", f"--window-size={WIDTH},{height}",
           f"--screenshot={png_path}", uri)
    if not png_path.is_file():
        sys.exit("Chrome did not write the PNG")
    return png_path


def output_dir(value: str | None) -> Path:
    if value:
        path = Path(value)
    elif Path("/opt/cursor/artifacts").is_dir():
        path = Path("/opt/cursor/artifacts")
    else:
        path = Path(tempfile.gettempdir()) / "dql-filetype-report"
    path.mkdir(parents=True, exist_ok=True)
    return path


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--top", type=int, default=DEFAULT_TOP,
                        help="Types shown in each bar chart before the rest fold into Other (default: %(default)s).")
    parser.add_argument("--vendor", action="append", metavar="PREFIX",
                        help="Path prefix counted as vendored. Repeat for more. "
                             f"Default: {', '.join(DEFAULT_VENDOR)}. Pass --vendor '' to count nothing as vendored.")
    parser.add_argument("--out", metavar="DIR", help="Folder for the HTML and PNG (default: a temp folder).")
    parser.add_argument("--no-png", action="store_true", help="Write only the HTML.")
    args = parser.parse_args(argv)
    if args.top < 1:
        parser.error("--top must be a positive integer")
    return args


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    vendor = tuple(v for v in (args.vendor if args.vendor is not None else DEFAULT_VENDOR) if v)
    repo = repo_root()
    groups, symlinks, meta = collect(repo, vendor)
    if not groups:
        sys.exit("git ls-files returned no files.")
    out = output_dir(args.out)
    stem = f"filetypes-{meta['head'].split(' ')[0]}"
    html_path = out / f"{stem}.html"
    html_path.write_text(page(groups, symlinks, meta, args.top, vendor), encoding="utf-8")
    print(f"html: {html_path}")
    if not args.no_png:
        print(f"png: {render_png(html_path)}")


if __name__ == "__main__":
    main()
