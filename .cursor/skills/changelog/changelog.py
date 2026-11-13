#!/usr/bin/env python3
"""Run the changelog script. See scripts/changelog.py and this skill's SKILL.md."""

import runpy
from pathlib import Path

runpy.run_path(str(Path(__file__).resolve().parents[3] / "scripts" / "changelog.py"), run_name="__main__")
