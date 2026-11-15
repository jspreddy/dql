#!/usr/bin/env python3
"""Point the README install commands at a dqlrs release tag.

The Release dqlrs workflow runs this for each stable release. It rewrites every
`dqlrs-<version>` tag in the install docs and fails when a file has none, so a
rewritten README cannot silently stop pinning the release.

Usage:
  release_pins.py VERSION    such as 0.7.11
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILES = ("README.md", "rust-docs/README.md")
TAG = re.compile(r"\bdqlrs-\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?\b")
VERSION = re.compile(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?")


def main(argv: list[str]) -> int:
    if len(argv) != 1 or not VERSION.fullmatch(argv[0]):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    tag = f"dqlrs-{argv[0]}"
    for name in FILES:
        path = ROOT / name
        text = path.read_text(encoding="utf-8")
        new, count = TAG.subn(tag, text)
        if count == 0:
            print(f"error: no dqlrs-<version> tag in {name}", file=sys.stderr)
            return 1
        path.write_text(new, encoding="utf-8")
        print(f"{name}: {count} pin(s) now {tag}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
