"""Rich transcripts for black-box pytest -v. No dql import."""

from __future__ import annotations

import inspect
import json
import os
from typing import Any

from rich.console import Console
from rich.padding import Padding
from rich.syntax import Syntax
from rich.text import Text

_no_color = bool(os.environ.get("NO_COLOR"))
console = Console(
    no_color=_no_color,
    force_terminal=not _no_color,
    highlight=False,
    soft_wrap=True,
)


def print_test_header(name: str, description: str | None, binary: str) -> None:
    desc = inspect.cleandoc(description) if description else ""
    if not desc:
        desc = "(no description)"
    console.print()
    console.rule(Text(name, style="bold yellow"), style="yellow", align="left")
    console.print()
    console.print(Text(desc, style="dim"))
    console.print()
    console.print(Text(binary, style="bold magenta"))
    console.print()


def print_test_footer() -> None:
    console.print()
    console.print()
    console.print()
    console.rule(style="yellow")


def print_step(title: str, code: str | None = None, lexer: str = "text", notes: str = "") -> None:
    console.print(Text("• %s" % title, style="bold blue"))
    if notes:
        console.print(Padding(Text(notes, style="dim"), (0, 0, 0, 4)))
    if code is None:
        return
    body = code.strip("\n")
    if body.strip() == "":
        console.print(Padding(Text("(empty)", style="dim"), (0, 0, 0, 4)))
        return
    syntax = Syntax(
        body + "\n",
        lexer,
        theme="ansi_dark",
        word_wrap=True,
        background_color="default",
    )
    console.print(Padding(syntax, (0, 0, 0, 4)))


def print_note_step(title: str, notes: str) -> None:
    print_step(title, code=None, notes=notes)


def lexer_for_output(text: str, json_mode: bool) -> str:
    if json_mode:
        return "json"
    stripped = text.strip()
    if stripped.startswith("{") or stripped.startswith("["):
        try:
            json.loads(stripped)
            return "json"
        except ValueError:
            pass
    return "text"


def pretty_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True, default=str)


def pretty_json_text(text: str) -> str:
    stripped = text.strip()
    if not stripped:
        return text
    try:
        return json.dumps(json.loads(stripped), indent=2, sort_keys=True, default=str)
    except ValueError:
        return text
