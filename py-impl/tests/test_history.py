import os
import shutil
import tempfile
from pathlib import Path
from unittest import TestCase
from unittest.mock import patch

from dql import readline_compat

readline = readline_compat.readline

from dql.history import HistoryManager


class TestHistoryManager(TestCase):
    """Tests for HistoryManager"""

    historyManager: HistoryManager

    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.historyManager = HistoryManager()

    def setUp(self):
        super().setUp()
        self._histDir = tempfile.mkdtemp()
        self._histFile = os.path.join(self._histDir, HistoryManager.history_file_name)
        if readline is not None:
            readline.clear_history()

    def tearDown(self):
        super().tearDown()
        shutil.rmtree(self._histDir)

    def assertFileExists(self, file_path):
        self.assertTrue(os.path.isfile(file_path))

    def assertFileContents(self, file_path, expected_contents):
        actual = Path(file_path).read_text(encoding="utf-8")
        self.assertEqual(expected_contents, actual)

    def test_history_file_is_created_on_load(self):
        """Assert that a history file is created in the history directory on load"""
        expectedHistFilePath = self._histFile

        self.historyManager.try_to_load_history(self._histDir)
        self.assertFileExists(expectedHistFilePath)

    def test_history_file_is_created_on_write(self):
        """Assert that a history file is created in the history directory on write"""
        expectedHistFilePath = self._histFile

        self.historyManager.try_to_write_history(self._histDir)
        self.assertFileExists(expectedHistFilePath)

    def test_history_file_contains_history_from_readline(self):
        """Assert that a history file will be written with proper contents."""
        if readline is None:
            self.fail("readline is not available")

        expectedHistFilePath = self._histFile

        readline.add_history("this is a simulated cli input")
        self.historyManager.try_to_write_history(self._histDir)
        self.assertFileExists(expectedHistFilePath)
        self.assertFileContents(expectedHistFilePath, "this is a simulated cli input\n")

    def test_history_file_contains_proper_appended_history(self):
        """Assert that a history file will be appended to"""
        if readline is None:
            self.fail("readline is not available")

        expectedHistFilePath = self._histFile

        readline.add_history("this is a simulated cli input")
        self.historyManager.try_to_write_history(self._histDir)
        self.historyManager.try_to_load_history(self._histDir)
        readline.add_history("another simulated cli input")
        self.historyManager.try_to_write_history(self._histDir)

        self.assertFileExists(expectedHistFilePath)
        self.assertFileContents(
            expectedHistFilePath,
            "this is a simulated cli input\nanother simulated cli input\n",
        )

    def test_write_history_handles_append_failure(self):
        """Assert that append failures do not propagate to the caller."""
        if readline is None:
            self.fail("readline is not available")

        readline.add_history("this is a simulated cli input")

        with patch.object(
            readline, "append_history_file", side_effect=OSError("permission denied")
        ):
            self.historyManager.try_to_write_history(self._histDir)

    def test_write_history_handles_get_length_failure(self):
        """Assert that get_current_history_length failures do not propagate."""
        if readline is None:
            self.fail("readline is not available")

        with patch.object(
            readline,
            "get_current_history_length",
            side_effect=OSError("read error"),
        ):
            self.historyManager.try_to_write_history(self._histDir)

    def test_remove_items_handles_readline_failure(self):
        """Assert that readline removal failures do not propagate to the caller."""
        if readline is None:
            self.fail("readline is not available")

        readline.add_history("this is a simulated cli input")

        with patch.object(
            readline, "remove_history_item", side_effect=OSError("read error")
        ):
            self.historyManager.remove_items(n=1)
