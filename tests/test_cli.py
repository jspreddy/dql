""" Tests for the CLI """

import json
import shutil
import tempfile
import unittest
from base64 import b64encode
from collections.abc import Iterable
from io import BytesIO, StringIO, TextIOWrapper
from typing import Any, List

import pytest
from mock import patch
from rich.pretty import pprint as print  # pylint: disable=W0622

from dql.cli import DQLClient, repl_command


class UniqueCollection(object):
    """Wrapper to make equality tests simpler"""

    def __init__(self, items):
        self._items = set(items)

    def __repr__(self):
        return repr(self._items)

    def __eq__(self, other):
        return isinstance(other, Iterable) and self._items == set(other)

    def __ne__(self, other):
        return not self.__eq__(other)


class BaseCLITest:
    """Base class for CLI tests"""

    @pytest.fixture(autouse=True)
    def run_around_tests(self, cli):
        # Clear out any pre-existing tables
        conn = cli.engine.connection
        for tablename in conn.list_tables():
            conn.delete_table(tablename, wait=True)

        yield

        conn = cli.engine.connection
        for tablename in conn.list_tables():
            conn.delete_table(tablename, wait=True)


class TestCli(BaseCLITest):
    """Tests for the CLI"""

    def assert_prints(self, cli, command, message):
        """Assert that a cli command will print a message to the console"""
        out = StringIO()
        with patch("sys.stdout", out):
            cli.onecmd(command)
        assert out.getvalue().strip() == message.strip()

    def test_repl_command_args(self):
        """The @repl_command decorator parses arguments and passes them in"""

        @repl_command
        def testfunc(zelf, first, second):
            """Test cli command"""
            assert zelf == self
            assert first == "a"
            assert second == "b"

        testfunc(self, "a b")  # pylint: disable=E1120

    def test_repl_command_kwargs(self):
        """The @repl_command decorator parses kwargs and passes them in"""

        @repl_command
        def testfunc(zelf, first, second=None):
            """Test cli command"""
            assert zelf == self
            assert first == "a"
            assert second == "b"

        testfunc(self, "a second=b")

    def test_help_docs(self, cli):
        """There is a help command for every DQL query type"""
        import dql.help

        for name in dir(dql.help):
            # Options is not a query type
            if name == "OPTIONS":
                continue
            if not name.startswith("_"):
                self.assert_prints(
                    cli, "help %s" % name.lower(), getattr(dql.help, name)
                )


class TestCliCommands(BaseCLITest):
    """Tests that run the 'dql --command'"""

    def _run_command_raw_output(self, cli: DQLClient, command: str) -> str:
        stream = BytesIO()
        out = TextIOWrapper(stream)
        with patch("sys.stdout", out):
            cli.run_command(command, use_json=True, raise_exceptions=True)
        assert cli.engine.partial == False, "Command was not terminated properly"
        out.seek(0)
        return out.read()

    def _run_command_and_parse_output(self, cli: DQLClient, command: str) -> List[Any]:
        output = self._run_command_raw_output(cli, command)
        ret: List[Any] = []
        for line in output.split("\n"):
            if not line:
                continue
            try:
                ret.append(json.loads(line))
            except json.JSONDecodeError:
                print("Output:")
                print(output)
                print("Error decoding json: %r" % line)
                assert False
        return ret

    def test_scan_table(self, cli: DQLClient, snapshot) -> None:
        """Can create, insert, and scan from table"""
        lines = self._run_command_raw_output(
            cli,
            """
                CREATE TABLE foobar (id STRING HASH KEY);
                INSERT INTO foobar (id='a', num=1, bin=b'a',
                    string_set=('a1', 'a2'), number_set=(1, 2), binary_set=(b'a1', b'a2'),
                    list=[1, 'a'],
                    dict={'a': 1, 'b': 'c'},
                    bool=TRUE
                );
                SCAN * FROM foobar;
            """,
        )
        assert lines == snapshot

    def test_ls(self, cli: DQLClient, snapshot) -> None:
        """Snapshot test for ls format"""
        self._run_command_and_parse_output(
            cli,
            """
                CREATE TABLE foobar (
                    id STRING HASH KEY,
                    range NUMBER RANGE KEY,
                    foo STRING INDEX('foo-index')
                ) GLOBAL INDEX ('bar-index', bar STRING);
            """,
        )
        output = self._run_command_raw_output(cli, "ls foobar")
        assert output == snapshot

    def test_ls_with_multiple_tables(self, cli, snapshot):
        """Snapshot test for ls format"""
        self._run_command_and_parse_output(
            cli,
            """
                CREATE TABLE foo (
                    id STRING HASH KEY,
                    range NUMBER RANGE KEY,
                    foo STRING INDEX('foo-index')
                );
                CREATE TABLE bar (
                    id STRING HASH KEY,
                    range NUMBER RANGE KEY,
                    bar STRING INDEX('bar-index')
                );
            """,
        )
        output = self._run_command_raw_output(cli, "ls")
        assert output == snapshot
