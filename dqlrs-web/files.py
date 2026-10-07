"""Workspace tree of .dql files. Paths cannot escape the root."""

from __future__ import annotations

from pathlib import Path

SKIP_DIRS = {
    ".git",
    ".venv",
    "venv",
    "node_modules",
    "target",
    "__pycache__",
    ".dynamo-local",
    "dist",
    ".pytest_cache",
    "mockups",
}


class PathError(ValueError):
    """The requested path is missing or outside the workspace."""


def resolve_inside(root: Path, relative: str) -> Path:
    raw = (relative or "").strip().replace("\\", "/")
    if raw.startswith("/") or raw.startswith("~"):
        raise PathError("path must stay inside the workspace")
    candidate = (root / raw).resolve()
    root_resolved = root.resolve()
    if candidate != root_resolved and root_resolved not in candidate.parents:
        raise PathError("path must stay inside the workspace")
    return candidate


def tree(root: Path) -> dict:
    """Nested directories plus .dql files."""
    root.mkdir(parents=True, exist_ok=True)
    return _dir_node(root, root)


def read_text(root: Path, relative: str) -> str:
    path = resolve_inside(root, relative)
    if not path.is_file():
        raise PathError(f"{relative} is not a file")
    if path.suffix.lower() != ".dql":
        raise PathError("only .dql files can be opened")
    return path.read_text(encoding="utf-8")


def write_text(root: Path, relative: str, text: str) -> None:
    path = _dql_path(root, relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def create_file(root: Path, relative: str) -> str:
    path = _dql_path(root, relative)
    if path.exists():
        raise PathError(f"{relative} already exists")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("", encoding="utf-8")
    return str(path.relative_to(root.resolve())).replace("\\", "/")


def _dql_path(root: Path, relative: str) -> Path:
    cleaned = (relative or "").strip().replace("\\", "/").lstrip("/")
    if not cleaned or cleaned.endswith("/"):
        raise PathError("choose a file name ending in .dql")
    if not cleaned.lower().endswith(".dql"):
        cleaned += ".dql"
    path = resolve_inside(root, cleaned)
    return path


def _dir_node(root: Path, directory: Path) -> dict:
    children = []
    entries = sorted(directory.iterdir(), key=lambda path: (not path.is_dir(), path.name.lower()))
    for entry in entries:
        if entry.name.startswith("."):
            continue
        if entry.is_dir():
            if entry.name in SKIP_DIRS:
                continue
            node = _dir_node(root, entry)
            if node["children"]:
                children.append(node)
            continue
        if entry.is_file() and entry.suffix.lower() == ".dql":
            children.append(
                {
                    "type": "file",
                    "name": entry.name,
                    "path": str(entry.relative_to(root.resolve())).replace("\\", "/"),
                }
            )
    rel = "." if directory.resolve() == root.resolve() else str(directory.relative_to(root.resolve())).replace("\\", "/")
    return {
        "type": "dir",
        "name": directory.name if rel != "." else root.name,
        "path": rel,
        "children": children,
    }
