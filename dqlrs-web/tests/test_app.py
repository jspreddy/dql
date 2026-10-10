"""Workspace files, statement splitting, and ls parsing."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine import (  # noqa: E402
    Engine,
    TableShape,
    _ls_keys,
    _ls_names,
    _safe_table_name,
    column_order,
    explain_index,
    interpret_serve_value,
    parse_table_shape,
    read_query,
)
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

    def test_keeps_show_tables_and_ls_refresh_intact(self) -> None:
        text = "SHOW TABLES LIKE 'alpha%';\nls alpha* refresh=True"
        self.assertEqual(
            split_statements(text),
            ["SHOW TABLES LIKE 'alpha%';", "ls alpha* refresh=True;"],
        )


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
        self.assertEqual(
            _ls_keys(detail),
            [{"name": "username", "role": "hash"}, {"name": "postid", "role": "range"}],
        )

    def test_table_shape_includes_local_and_global_indexes(self) -> None:
        detail = "\n".join(
            [
                "Name: sample_cosmos",
                "Hash Key: pk (STRING)",
                "Range Key: sk (STRING)",
                "",
                "Global Indexes:",
                "  Name             Projection       Read    Write HashKey              RangeKey               Status",
                "  by-kind          ALL               N/A      N/A kind (STRING)        name (STRING)          ACTIVE",
                "",
                "CREATE TABLE sample_cosmos (pk STRING HASH KEY, sk STRING RANGE KEY);",
            ]
        )
        shape = parse_table_shape(detail)
        assert shape is not None
        self.assertEqual(shape.hash_key, "pk")
        self.assertEqual(shape.range_key, "sk")
        self.assertEqual(shape.indexes["by-kind"], ("kind", "name"))

        local = "\n".join(
            [
                "Hash Key: device_id (STRING)",
                "Range Key: ts (NUMBER)",
                "",
                "Local Indexes:",
                "  kind-index  hash=device_id  range=kind  projection=ALL",
            ]
        )
        local_shape = parse_table_shape(local)
        assert local_shape is not None
        self.assertEqual(local_shape.indexes["kind-index"], ("device_id", "kind"))

    def test_column_order_keys_then_selection_or_alpha(self) -> None:
        shape = TableShape("id", "sk", {"by-n": ("n", "sk")})
        rows = [
            {"zebra": "z", "id": "a", "n": 9, "sk": 2, "apple": "p"},
            {"id": "a", "sk": 1, "n": 3, "apple": "q", "zebra": "y"},
        ]
        plain = column_order(rows, shape, None, None)
        self.assertEqual([column["name"] for column in plain], ["id", "sk", "apple", "n", "zebra"])
        self.assertEqual(plain[0], {"name": "id", "table": "hash", "index": None})
        self.assertEqual(plain[1], {"name": "sk", "table": "range", "index": None})
        self.assertEqual(plain[2]["table"], None)
        indexed_star = column_order(rows, shape, "by-n", None)
        self.assertEqual(
            [column["name"] for column in indexed_star],
            ["id", "sk", "n", "apple", "zebra"],
        )
        self.assertEqual(indexed_star[1], {"name": "sk", "table": "range", "index": "range"})
        self.assertEqual(indexed_star[2], {"name": "n", "table": None, "index": "hash"})
        projected = [{"zebra": "z", "apple": "p", "id": "a"}]
        self.assertEqual(
            [column["name"] for column in column_order(projected, shape, None, ["zebra", "apple", "id"])],
            ["id", "zebra", "apple"],
        )
        indexed = [{"apple": "p", "zebra": "z", "n": 9}]
        self.assertEqual(
            [column["name"] for column in column_order(indexed, shape, "by-n", ["apple", "zebra", "n"])],
            ["n", "apple", "zebra"],
        )
        self.assertEqual(explain_index("query t {'index': \"by-n\", 'filter': \"n = :v1\"}"), "by-n")
        self.assertIsNone(explain_index("query t {'index': \"TABLE\"}"))
        self.assertIsNone(explain_index("scan t {'filter': \"habitable = :v1\"}"))
        self.assertEqual(read_query("SELECT zebra, apple AS fruit FROM posts WHERE id = 'a';"), ("posts", ["zebra", "fruit"]))
        self.assertEqual(read_query("-- note\nSELECT CONSISTENT * FROM posts WHERE id = 'a';"), ("posts", None))
        self.assertEqual(read_query("SCAN name, ts(updated) FROM posts;"), ("posts", ["name", "ts(updated)"]))

    def test_table_name_shape(self) -> None:
        self.assertTrue(_safe_table_name("nb_posts"))
        self.assertTrue(_safe_table_name("nb-posts"))
        self.assertFalse(_safe_table_name("nb posts"))
        self.assertFalse(_safe_table_name(""))


class ListTablesRefreshTests(unittest.TestCase):
    def test_refresh_rewrites_the_description_cache(self) -> None:
        engine = _RecordingEngine()
        engine._shapes["nb_posts"] = TableShape("id", "", {})
        tables = engine.list_tables("nb_*")
        self.assertEqual([table["name"] for table in tables], ["nb_posts"])
        self.assertEqual(engine.commands, ["ls", "ls nb_posts"])
        self.assertIn("nb_posts", engine._shapes)

        engine.commands.clear()
        engine.list_tables("nb_*", refresh=True)
        self.assertEqual(engine.commands, ["ls refresh=true", "ls nb_posts"])
        self.assertEqual(engine._shapes, {})


class _RecordingEngine(Engine):
    def __init__(self) -> None:
        super().__init__("dqlrs", "", "8000", "us-west-1")
        self.commands: list[str] = []

    def _exec(self, dql: str) -> dict:
        self.commands.append(dql)
        if dql.startswith("ls ") and not dql.startswith("ls refresh"):
            return {"ok": True, "message": "Hash Key: id (STRING)\n"}
        return {"ok": True, "message": "Tables\nName Items\nnb_posts 1\nother 1\n"}


if __name__ == "__main__":
    unittest.main()
