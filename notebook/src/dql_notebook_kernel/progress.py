"""HTML progress bars and JSON progress-line parsing for notebook cells."""

from __future__ import annotations

import html
import json
from typing import Any, Optional


def parse_progress_line(line: str) -> Optional[dict[str, Any]]:
    """Return a progress event dict, or None if the line is not progress JSON."""
    stripped = (line or "").strip()
    if not stripped or stripped[0] != "{":
        return None
    try:
        value = json.loads(stripped)
    except json.JSONDecodeError:
        return None
    if not isinstance(value, dict):
        return None
    if value.get("event") != "progress":
        return None
    if "done" not in value:
        return None
    return value


def progress_text(done: int, total: Optional[int], phase: str) -> str:
    if total:
        pct = min(100, int(100 * done / total)) if total else 0
        return f"{phase} {done:,}/{total:,} ({pct}%)"
    return f"{phase} {done:,}"


def progress_html(done: int, total: Optional[int], phase: str) -> str:
    label = html.escape(progress_text(done, total, phase), quote=True)
    if total:
        return (
            '<div class="dql-progress">'
            f'<div style="margin-bottom:4px;font-family:var(--jp-code-font-family,monospace);font-size:13px">{label}</div>'
            f'<progress value="{int(done)}" max="{int(total)}" style="width:100%;height:12px"></progress>'
            "</div>"
        )
    return (
        '<div class="dql-progress">'
        f'<div style="margin-bottom:4px;font-family:var(--jp-code-font-family,monospace);font-size:13px">{label}</div>'
        '<progress style="width:100%;height:12px"></progress>'
        "</div>"
    )


def progress_bundle(done: int, total: Optional[int], phase: str) -> dict[str, str]:
    return {
        "text/plain": progress_text(done, total, phase) + "\n",
        "text/html": progress_html(done, total, phase),
    }
