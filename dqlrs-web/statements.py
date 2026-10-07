"""Split a DQL selection into statements without breaking quoted text."""

from __future__ import annotations


def split_statements(text: str) -> list[str]:
    """Return semicolon-terminated statements. Comments that are whole lines are kept."""
    parts: list[str] = []
    buf: list[str] = []
    quote: str | None = None
    i = 0
    length = len(text)
    while i < length:
        ch = text[i]
        nxt = text[i + 1] if i + 1 < length else ""
        if quote is None and ch == "-" and nxt == "-":
            buf.append(ch)
            buf.append(nxt)
            i += 2
            while i < length and text[i] != "\n":
                buf.append(text[i])
                i += 1
            continue
        if quote is not None:
            buf.append(ch)
            if ch == quote and text[i - 1] != "\\":
                quote = None
            i += 1
            continue
        if ch in {"'", '"'}:
            quote = ch
            buf.append(ch)
            i += 1
            continue
        if ch == ";":
            statement = "".join(buf).strip()
            buf = []
            if _has_code(statement):
                parts.append(statement + ";")
            i += 1
            continue
        buf.append(ch)
        i += 1
    tail = "".join(buf).strip()
    if _has_code(tail):
        parts.append(tail if tail.endswith(";") else tail + ";")
    return parts


def _has_code(statement: str) -> bool:
    for line in statement.splitlines():
        stripped = line.strip()
        if stripped and not stripped.startswith("--"):
            return True
    return False
