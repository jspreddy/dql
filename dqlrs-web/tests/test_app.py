"""Workspace files, statement splitting, and ls parsing."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine import _ls_keys, _ls_names, _safe_table_name  # noqa: E402
from files import PathError, create_file, read_text, resolve_inside, tree, write_text  # noqa: E402
from statements import split_statements  # noqa: E402


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
            node = tree(root)
            self.assertEqual([child["name"] for child in node["children"]], ["queries"])
            queries = next(child for child in node["children"] if child["name"] == "queries")
            self.assertEqual(queries["type"], "dir")
            self.assertEqual(queries["children"][0]["path"], "queries/posts.dql")
            with self.assertRaises(PathError):
                resolve_inside(root, "../secret.dql")

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
