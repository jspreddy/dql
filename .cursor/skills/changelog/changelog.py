#!/usr/bin/env python3
"""Regenerate or check CHANGELOG.md by running the just recipe."""

import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[3]
args = sys.argv[1:]
if args == ["--check"]:
    command = ["just", "changelog-check"]
else:
    command = ["just", "changelog"]
    if args:
        command.extend(["--", *args])
raise SystemExit(subprocess.call(command, cwd=root))
