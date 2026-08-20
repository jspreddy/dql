import sys
from unittest import TestCase

from dql import readline_compat


class TestReadlineCompat(TestCase):
    def test_gnureadline_is_registered_as_readline(self):
        if readline_compat.readline is None:
            self.skipTest("readline is not available")

        self.assertIs(sys.modules["readline"], readline_compat.readline)

    def test_cmd_and_history_share_readline_buffer(self):
        if readline_compat.readline is None:
            self.skipTest("readline is not available")

        import readline

        readline.clear_history()
        readline.add_history("shared history entry")
        self.assertEqual(readline.get_current_history_length(), 1)
        self.assertIs(readline, readline_compat.readline)
