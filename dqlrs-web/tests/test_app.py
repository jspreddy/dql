"""Workspace files, statement splitting, and ls parsing."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine import _ls_keys, _ls_names, _safe_table_name, interpret_serve_value  # noqa: E402
from files import (  # noqa: E402
    PathError,
    create_file,
    delete_path,
    duplicate_path,
    make_dir,
    move_path,
    read_text,
    resolve_inside,
    tree,
    write_text,
)
from statements import split_statements  # noqa: E402


class ServeLineTests(unittest.TestCase):
    def test_progress_is_separate_from_the_result_envelope(self) -> None:
        progress = interpret_serve_value(
            {"id": "1", "event": "progress", "done": 25, "total": 30, "phase": "write"}
        )
        assert progress is not None
        self.assertEqual(progress["event"], "progress")
        self.assertEqual(progress["done"], 25)
        self.assertEqual(progress["total"], 30)
        self.assertNotIn("ok", progress)

        unknown = interpret_serve_value(
            {"id": "2", "event": "progress", "done": 33, "total": None, "phase": "read"}
        )
        assert unknown is not None
        self.assertIsNone(unknown["total"])
        self.assertEqual(unknown["done"], 33)

        envelope = interpret_serve_value({"ok": True, "kind": "affected", "affected": 30})
        assert envelope is not None
        self.assertEqual(envelope["event"], "envelope")
        self.assertTrue(envelope["envelope"]["ok"])
        self.assertIsNone(interpret_serve_value("nope"))
        mixed = interpret_serve_value({"event": "progress", "ok": False})
        assert mixed is not None
        self.assertEqual(mixed["event"], "envelope")


class StatementTests(unittest.TestCase):
    def test_splits_on_semicolons_and_keeps_quotes(self) -> None:
        text = "SELECT * FROM t WHERE name = 'a;b';\nSCAN * FROM t;"
        self.assertEqual(
            split_statements(text),
            ["SELECT * FROM t WHERE name = 'a;b';", "SCAN * FROM t;"],
        )

    def test_ignores_blank_input(self) -> None:
        self.assertEqual(split_statements("  \n-- just a comment\n"), [])


class FileTests(unittest.TestCase):
    def test_tree_nests_dql_files_and_rejects_escape(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "queries").mkdir()
            (root / "queries" / "posts.dql").write_text("SCAN * FROM t;\n", encoding="utf-8")
            (root / "empty").mkdir()
            (root / "notes.txt").write_text("nope", encoding="utf-8")
            (root / "src").mkdir()
            (root / "src" / "main.py").write_text("x", encoding="utf-8")
            node = tree(root)
            self.assertEqual([child["name"] for child in node["children"]], ["empty", "queries"])
            queries = next(child for child in node["children"] if child["name"] == "queries")
            self.assertEqual(queries["type"], "dir")
            self.assertEqual(queries["children"][0]["path"], "queries/posts.dql")
            with self.assertRaises(PathError):
                resolve_inside(root, "../secret.dql")

    def test_move_duplicate_and_delete(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            create_file(root, "queries/posts.dql")
            write_text(root, "queries/posts.dql", "SCAN * FROM t;\n")
            folder = make_dir(root, "archive")
            self.assertEqual(folder, "archive")
            self.assertIn("archive", [child["name"] for child in tree(root)["children"]])
            moved, kind = move_path(root, "queries/posts.dql", "archive/posts.dql")
            self.assertEqual((moved, kind), ("archive/posts.dql", "file"))
            self.assertEqual(read_text(root, moved), "SCAN * FROM t;\n")
            copy, copy_kind = duplicate_path(root, moved)
            self.assertEqual((copy, copy_kind), ("archive/posts copy.dql", "file"))
            again, _kind = duplicate_path(root, moved)
            self.assertEqual(again, "archive/posts copy 2.dql")
            folder_copy, folder_kind = duplicate_path(root, "archive")
            self.assertEqual((folder_copy, folder_kind), ("archive copy", "dir"))
            self.assertTrue((root / "archive copy" / "posts.dql").is_file())
            with self.assertRaises(PathError):
                move_path(root, "archive", "archive/nested")
            with self.assertRaises(PathError):
                move_path(root, "archive/posts.dql", "../out.dql")
            (root / "archive" / "notes.txt").write_text("keep", encoding="utf-8")
            with self.assertRaises(PathError):
                delete_path(root, "archive")
            (root / "archive" / "notes.txt").unlink()
            delete_path(root, "archive/posts copy.dql")
            self.assertFalse((root / "archive" / "posts copy.dql").exists())
            delete_path(root, "archive copy")
            self.assertFalse((root / "archive copy").exists())

    def test_create_and_roundtrip(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            path = create_file(root, "queries/new")
            self.assertEqual(path, "queries/new.dql")
            write_text(root, path, "ls;\n")
            self.assertEqual(read_text(root, path), "ls;\n")


class LsParseTests(unittest.TestCase):
    def test_names_and_keys(self) -> None:
        summary = "Tables\nName     Items\nnb_posts        2\nnb_users        1\n"
        self.assertEqual(_ls_names(summary), ["nb_posts", "nb_users"])
        detail = "Name: nb_posts\nHash Key: username (STRING)\nRange Key: postid (NUMBER)\n"
        self.assertEqual(_ls_keys(detail), "username HASH · postid RANGE")

    def test_table_name_shape(self) -> None:
        self.assertTrue(_safe_table_name("nb_posts"))
        self.assertTrue(_safe_table_name("nb-posts"))
        self.assertFalse(_safe_table_name("nb posts"))
        self.assertFalse(_safe_table_name(""))


if __name__ == "__main__":
    unittest.main()
