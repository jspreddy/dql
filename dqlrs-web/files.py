"""Workspace tree of .dql files. Paths cannot escape the root."""

from __future__ import annotations

import shutil
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
    """Nested directories plus .dql files. Empty folders stay visible."""
    root.mkdir(parents=True, exist_ok=True)
    node, _hidden = _dir_node(root, root, is_root=True)
    return node


def make_dir(root: Path, relative: str) -> str:
    path = _dir_path(root, relative)
    if path.exists():
        raise PathError(f"{_display(relative)} already exists")
    path.mkdir(parents=True)
    return _rel(root, path)


def move_path(root: Path, source: str, dest: str) -> tuple[str, str]:
    """Rename or move a file or folder. Returns the new relative path and kind."""
    src = _existing(root, source)
    kind = "dir" if src.is_dir() else "file"
    dst = _dir_path(root, dest) if kind == "dir" else _dql_path(root, dest)
    if dst.resolve() == src.resolve():
        return _rel(root, src), kind
    if kind == "dir" and _is_inside(src, dst):
        raise PathError("cannot move a folder into itself")
    if dst.exists():
        raise PathError(f"{_rel(root, dst)} already exists")
    dst.parent.mkdir(parents=True, exist_ok=True)
    src.rename(dst)
    return _rel(root, dst), kind


def duplicate_path(root: Path, source: str) -> tuple[str, str]:
    src = _existing(root, source)
    kind = "dir" if src.is_dir() else "file"
    dest = _unique_copy(src)
    if kind == "file":
        dest.write_bytes(src.read_bytes())
    else:
        _copy_tree(src, dest)
    return _rel(root, dest), kind


def delete_path(root: Path, relative: str) -> None:
    path = _existing(root, relative)
    if path.is_file():
        if path.suffix.lower() != ".dql":
            raise PathError("only .dql files can be deleted")
        path.unlink()
        return
    blocked = _foreign_entry(path)
    if blocked:
        raise PathError(f"folder contains {blocked}, which is not a .dql file")
    shutil.rmtree(path)


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


def _dir_node(root: Path, directory: Path, is_root: bool = False) -> tuple[dict | None, bool]:
    """Return the visible node and whether this folder holds non-.dql files."""
    children = []
    hidden = False
    entries = sorted(directory.iterdir(), key=lambda path: (not path.is_dir(), path.name.lower()))
    for entry in entries:
        if entry.name.startswith(".") or (entry.is_dir() and entry.name in SKIP_DIRS):
            continue
        if entry.is_dir():
            node, child_hidden = _dir_node(root, entry)
            hidden = hidden or child_hidden
            if node is not None:
                children.append(node)
            continue
        if entry.is_file() and entry.suffix.lower() == ".dql":
            children.append({"type": "file", "name": entry.name, "path": _rel(root, entry)})
        elif entry.is_file():
            hidden = True
    if not children and hidden and not is_root:
        return None, True
    rel = "." if directory.resolve() == root.resolve() else _rel(root, directory)
    return {
        "type": "dir",
        "name": directory.name if rel != "." else root.name,
        "path": rel,
        "children": children,
    }, hidden


def _existing(root: Path, relative: str) -> Path:
    path = resolve_inside(root, relative)
    if path.resolve() == root.resolve() or not path.exists():
        raise PathError("choose a file or folder in the workspace")
    if not path.is_file() and not path.is_dir():
        raise PathError("choose a file or folder in the workspace")
    return path


def _dir_path(root: Path, relative: str) -> Path:
    return resolve_inside(root, _clean_relative(relative, file=False))


def _clean_relative(relative: str, file: bool) -> str:
    cleaned = (relative or "").strip().replace("\\", "/").strip("/")
    if not cleaned:
        raise PathError("choose a name")
    parts = cleaned.split("/")
    if any(part in {"", ".", ".."} or part.startswith(".") for part in parts):
        raise PathError("invalid name")
    if file and not cleaned.lower().endswith(".dql"):
        cleaned += ".dql"
    return cleaned


def _rel(root: Path, path: Path) -> str:
    return str(path.resolve().relative_to(root.resolve())).replace("\\", "/")


def _display(relative: str) -> str:
    return (relative or "").strip().replace("\\", "/").strip("/") or "that path"


def _is_inside(parent: Path, child: Path) -> bool:
    parent_resolved = parent.resolve()
    child_resolved = child.resolve()
    return child_resolved == parent_resolved or parent_resolved in child_resolved.parents


def _unique_copy(src: Path) -> Path:
    stem = src.stem if src.is_file() else src.name
    suffix = src.suffix if src.is_file() else ""
    parent = src.parent
    candidate = parent / f"{stem} copy{suffix}"
    number = 2
    while candidate.exists():
        candidate = parent / f"{stem} copy {number}{suffix}"
        number += 1
    return candidate


def _copy_tree(src: Path, dest: Path) -> None:
    dest.mkdir(parents=True)
    for entry in src.iterdir():
        if entry.name.startswith(".") or entry.name in SKIP_DIRS:
            continue
        target = dest / entry.name
        if entry.is_dir():
            _copy_tree(entry, target)
        elif entry.is_file() and entry.suffix.lower() == ".dql":
            target.write_bytes(entry.read_bytes())


def _foreign_entry(path: Path) -> str:
    for entry in path.iterdir():
        if entry.name.startswith(".") or entry.name in SKIP_DIRS:
            return entry.name
        if entry.is_dir():
            found = _foreign_entry(entry)
            if found:
                return found
        elif not entry.is_file() or entry.suffix.lower() != ".dql":
            return entry.name
    return ""
