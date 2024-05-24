""" Tests for the language parser """

from typing import Any, Dict, List, Tuple, Union
from unittest import TestCase

from pyparsing import ParseException, StringEnd

from dql.expressions import ConstraintExpression, SelectionExpression, UpdateExpression
from dql.expressions.base import Field, Value
from dql.expressions.constraint import (
    BetweenConstraint,
    Conjunction,
    FunctionConstraint,
    InConstraint,
    Invert,
    OperatorConstraint,
    SizeConstraint,
    TypeConstraint,
)
from dql.grammar import parser, statement_parser, update_expr
from dql.grammar.common import value
from dql.grammar.query import selection, where

TEST_CASES: Dict[str, List[Tuple[str, Union[str, List[Any]]]]] = {
    "create": [
        (
            "CREATE TABLE foobars (foo string hash key)",
            ["CREATE", "TABLE", "foobars", [["foo", "STRING", ["HASH", "KEY"]]]],
        ),
        (
            "CREATE TABLE foobars (foo string hash key, bar NUMBER)",
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]], ["bar", "NUMBER"]],
            ],
        ),
        (
            "CREATE TABLE foobars (foo string hash key, THROUGHPUT (1, 1))",
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                ["1"],
                ["1"],
            ],
        ),
        (
            "CREATE TABLE IF NOT EXISTS foobars (foo string hash key)",
            [
                "CREATE",
                "TABLE",
                ["IF", "NOT", "EXISTS"],
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
            ],
        ),
        (
            "CREATE TABLE foobars (foo string hash key, bar number range key)",
            [
                "CREATE",
                "TABLE",
                "foobars",
                [
                    ["foo", "STRING", ["HASH", "KEY"]],
                    ["bar", "NUMBER", ["RANGE", "KEY"]],
                ],
            ],
        ),
        ("CREATE TABLE foobars foo binary hash key", "error"),
        ("CREATE TABLE foobars (foo hash key)", "error"),
        ("CREATE TABLE foobars (foo binary hash key) garbage", "error"),
    ],
    "create_index": [
        (
            'CREATE TABLE foobars (foo binary index("foo-index"))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "BINARY", [["INDEX"], ['"foo-index"']]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo binary index("idxname"))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "BINARY", [["INDEX"], ['"idxname"']]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo binary keys index("idxname"))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "BINARY", [["KEYS", "INDEX"], ['"idxname"']]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo binary include index("idxname", ["foo"]))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "BINARY", [["INCLUDE", "INDEX"], ['"idxname"'], [['"foo"']]]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo binary INCLUDE INDEX("idxname", ["foo", "bar"]))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [
                    [
                        "foo",
                        "BINARY",
                        [["INCLUDE", "INDEX"], ['"idxname"'], [['"foo"'], ['"bar"']]],
                    ]
                ],
            ],
        ),
        ("CREATE foobars (foo binary index(idxname))", "error"),
    ],
    "create_global": [
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo)',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["INDEX"], ['"gindex"'], ["foo"]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar)',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["INDEX"], ['"gindex"'], ["foo"], ["bar"]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo) GLOBAL INDEX ("g2idx", bar, foo)',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [
                    [["INDEX"], ['"gindex"'], ["foo"]],
                    [["INDEX"], ['"g2idx"'], ["bar"], ["foo"]],
                ],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar, THROUGHPUT (2, 4))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["INDEX"], ['"gindex"'], ["foo"], ["bar"], [["2"], ["4"]]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, THROUGHPUT (2, 4))',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["INDEX"], ['"gindex"'], ["foo"], [["2"], ["4"]]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL KEYS INDEX ("gindex", foo)',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["KEYS", "INDEX"], ['"gindex"'], ["foo"]]],
            ],
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INCLUDE INDEX ("g2idx", bar, foo, ["baz"])',
            [
                "CREATE",
                "TABLE",
                "foobars",
                [["foo", "STRING", ["HASH", "KEY"]]],
                [[["INCLUDE", "INDEX"], ['"g2idx"'], ["bar"], ["foo"], [['"baz"']]]],
            ],
        ),
        ('CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex")', "error"),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar, baz)',
            "error",
        ),
        (
            'CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar),',
            "error",
        ),
    ],
    "insert": [
        (
            "INSERT INTO foobars (foo, bar) VALUES (1, 2)",
            ["INSERT", "INTO", "foobars", ["foo", "bar"], "VALUES", [[["1"], ["2"]]]],
        ),
        (
            "INSERT INTO foobars (foo, bar) VALUES (1, 2), (3, 4)",
            [
                "INSERT",
                "INTO",
                "foobars",
                ["foo", "bar"],
                "VALUES",
                [[["1"], ["2"]], [["3"], ["4"]]],
            ],
        ),
        (
            'INSERT INTO foobars (foo, bar) VALUES (b"binary", ("set", "of", "values"))',
            [
                "INSERT",
                "INTO",
                "foobars",
                ["foo", "bar"],
                "VALUES",
                [[['b"binary"'], [['"set"'], ['"of"'], ['"values"']]]],
            ],
        ),
        ("INSERT foobars (foo, bar) VALUES (1, 2)", "error"),
        ("INSERT INTO foobars foo, bar VALUES (1, 2)", "error"),
        ("INSERT INTO foobars (foo, bar) VALUES", "error"),
        ("INSERT INTO foobars (foo, bar) VALUES 1, 2", "error"),
        ("INSERT INTO foobars (foo, bar) VALUES (1, 2) garbage", "error"),
    ],
    "drop": [
        ("DROP TABLE foobars", ["DROP", "TABLE", "foobars"]),
        (
            "DROP TABLE IF EXISTS foobars",
            ["DROP", "TABLE", ["IF", "EXISTS"], "foobars"],
        ),
        ("DROP foobars", "error"),
        ("DROP TABLE foobars garbage", "error"),
    ],
    "alter": [
        (
            "ALTER TABLE foobars SET THROUGHPUT (3, 4)",
            ["ALTER", "TABLE", "foobars", ["3"], ["4"]],
        ),
        (
            "ALTER TABLE foobars SET THROUGHPUT (0, *)",
            ["ALTER", "TABLE", "foobars", ["0"], "*"],
        ),
        (
            "ALTER TABLE foobars SET INDEX foo THROUGHPUT (3, 4)",
            ["ALTER", "TABLE", "foobars", "foo", ["3"], ["4"]],
        ),
        ("ALTER TABLE foobars DROP INDEX foo", ["ALTER", "TABLE", "foobars", "foo"]),
        (
            'ALTER TABLE foobars CREATE GLOBAL INDEX ("gindex", foo string, bar number, TP (2, 3))',
            [
                "ALTER",
                "TABLE",
                "foobars",
                [
                    ["INDEX"],
                    ['"gindex"'],
                    ["foo", "STRING"],
                    ["bar", "NUMBER"],
                    [["2"], ["3"]],
                ],
            ],
        ),
        ("ALTER TABLE foobars SET foo = bar", "error"),
        ("ALTER TABLE foobars SET THROUGHPUT 1, 1", "error"),
    ],
    "dump": [
        ("DUMP SCHEMA", ["DUMP", "SCHEMA"]),
        ("DUMP SCHEMA foobars, wibbles", ["DUMP", "SCHEMA", ["foobars", "wibbles"]]),
        ("DUMP SCHEMA foobars wibbles", "error"),
    ],
    "multiple": [
        ("DUMP SCHEMA;DUMP SCHEMA", [["DUMP", "SCHEMA"], ["DUMP", "SCHEMA"]]),
        ("DUMP SCHEMA;\nDUMP SCHEMA", [["DUMP", "SCHEMA"], ["DUMP", "SCHEMA"]]),
        ("DUMP SCHEMA\n;\nDUMP SCHEMA", [["DUMP", "SCHEMA"], ["DUMP", "SCHEMA"]]),
    ],
    "variables": [
        ('"a"', [['"a"']]),
        ("1", [["1"]]),
        ("2.7", [["2.7"]]),
        ('b"hi"', [['b"hi"']]),
        ("null", [["NULL"]]),
        ("()", [["()"]]),
        ("(1, 2)", [[["1"], ["2"]]]),
        ('("a", "b")', [[['"a"'], ['"b"']]]),
        ("true", [["TRUE"]]),
        ("false", [["FALSE"]]),
        ('[1, "a"]', [[["1"], ['"a"']]]),
        ("[]", [[]]),
        ('[1, ["a", 2]]', [[["1"], [['"a"'], ["2"]]]]),
        ('{"a": 1}', [[[['"a"'], ["1"]]]]),
        ("{}", [[]]),
        ('{"a": {"b": true}}', [[[['"a"'], [[['"b"'], ["TRUE"]]]]]]),
        ('timestamp("2012")', [[["TIMESTAMP", '"2012"']]]),
        ('utctimestamp "2012" ', [[["UTCTIMESTAMP", '"2012"']]]),
        ('ts("2012")', [[["TS", '"2012"']]]),
        ("now()", [[["NOW"]]]),
        (
            'ms(now() + interval("1 day"))',
            [[["MS", [[["NOW"], "+", ["INTERVAL", ["1", "DAY"]]]]]]],
        ),
    ],
}


class TestParser(TestCase):
    """Tests for the language parser"""

    def _run_tests(self, key, grammar=statement_parser):
        """Run a set of tests"""
        for string, result in TEST_CASES[key]:
            try:
                parse_result = grammar.parseString(string)
                if result == "error":
                    assert False, "Parsing '%s' should have failed.\nGot: %s" % (
                        string,
                        parse_result.asList(),
                    )
                else:
                    self.assertEqual(result, parse_result.asList())
            except ParseException as e:
                if result != "error":
                    print(string)
                    print(" " * e.loc + "^")
                    raise
            except AssertionError:
                print("Parsing : %s" % string)
                print("Expected: %s" % result)
                print("Got     : %s" % parse_result.asList())
                raise

    def test_create(self):
        """Run tests for CREATE statements"""
        self._run_tests("create")

    def test_create_index(self):
        """Run tests for CREATE statements with indexes"""
        self._run_tests("create_index")

    def test_create_global(self):
        """Run tests for CREATE statements with global indexes"""
        self._run_tests("create_global")

    def test_insert(self):
        """Run tests for INSERT statements"""
        self._run_tests("insert")

    def test_drop(self):
        """Run tests for DROP statements"""
        self._run_tests("drop")

    def test_alter(self):
        """Run tests for ALTER statements"""
        self._run_tests("alter")

    def test_dump(self):
        """Run tests for DUMP statements"""
        self._run_tests("dump")

    def test_multiple_statements(self):
        """Run tests for multiple-line statements"""
        self._run_tests("multiple", parser)

    def test_variables(self):
        """Run tests for parsing variables"""
        self._run_tests("variables", value)


CONSTRAINTS = [
    ("WHERE bar = 1", OperatorConstraint("bar", "=", Value(1))),
    (
        "WHERE foo != 1 or bar > 0",
        Conjunction(
            False,
            OperatorConstraint("foo", "<>", Value(1)),
            OperatorConstraint("bar", ">", Value(0)),
        ),
    ),
    ("WHERE foo != bar", OperatorConstraint("foo", "<>", Field("bar"))),
    ("WHERE NOT foo > 3", Invert(OperatorConstraint("foo", ">", Value(3)))),
    ("WHERE size(foo) < 3", SizeConstraint("foo", "<", 3)),
    (
        'WHERE begins_with(foo, "bar")',
        FunctionConstraint("begins_with", "foo", "bar"),
    ),
    ("WHERE attribute_exists(foo)", FunctionConstraint("attribute_exists", "foo")),
    (
        "WHERE attribute_not_exists(foo)",
        FunctionConstraint("attribute_not_exists", "foo"),
    ),
    (
        "WHERE attribute_type(foo, N)",
        TypeConstraint("attribute_type", "foo", "N"),
    ),
    (
        'WHERE contains(foo, "test")',
        FunctionConstraint("contains", "foo", "test"),
    ),
    ("WHERE foo between 1 and 5", BetweenConstraint("foo", 1, 5)),
    ("WHERE foo in (1, 5, 7)", InConstraint("foo", [1, 5, 7])),
    (
        'WHERE foo > utcts("2015-12-5")',
        OperatorConstraint("foo", ">", Value(1449273600.0)),
    ),
    (
        'WHERE foo > ms(utcts "2015-12-5")',
        OperatorConstraint("foo", ">", Value(1449273600000.0)),
    ),
    (
        'WHERE foo > utcts "2015-12-5" + interval "1 minute 1s"',
        OperatorConstraint("foo", ">", Value(1449273661.0)),
    ),
    (
        'WHERE foo > ms(utcts "2015-12-5" + interval "1 minute 1s")',
        OperatorConstraint("foo", ">", Value(1449273661000.0)),
    ),
    (
        'WHERE foo > utcts "2015-12-5" - interval "1y -2d 1month -3 weeks 8 day 2h 3ms 10us"',
        OperatorConstraint("foo", ">", Value(1416434399.99699)),
    ),
    (
        'WHERE foo < 1 AND (bar >= 0 OR baz < "str" OR qux = 1)',
        Conjunction(
            True,
            OperatorConstraint("foo", "<", Value(1)),
            Conjunction(
                False,
                OperatorConstraint("bar", ">=", Value(0)),
                OperatorConstraint("baz", "<", Value("str")),
                OperatorConstraint("qux", "=", Value(1)),
            ),
        ),
    ),
    (
        "WHERE foo < 1 and ((bar > 0 or baz < 2) or qux > 4)",
        Conjunction(
            True,
            OperatorConstraint("foo", "<", Value(1)),
            Conjunction(
                False,
                OperatorConstraint("bar", ">", Value(0)),
                OperatorConstraint("baz", "<", Value(2)),
                OperatorConstraint("qux", ">", Value(4)),
            ),
        ),
    ),
    (
        'WHERE foo < 1 AND (bar >= 0 AND baz < "str")',
        Conjunction(
            True,
            OperatorConstraint("foo", "<", Value(1)),
            OperatorConstraint("bar", ">=", Value(0)),
            OperatorConstraint("baz", "<", Value("str")),
        ),
    ),
]

UPDATES = [
    ("set foo = 1", "SET foo = 1"),
    ("set foo = foo + 1", "SET foo = foo + 1"),
    ("set foo = 1 + foo", "SET foo = 1 + foo"),
    ("set foo = foo + foo", "SET foo = foo + foo"),
    ("set foo = 1 + 2", "SET foo = 1 + 2"),
    ("set foo = foo - 2", "SET foo = foo - 2"),
    (
        "set foo = foo - 2, bar = 3, baz = qux + 4",
        "SET foo = foo - 2, bar = 3, baz = qux + 4",
    ),
    ("SET foo[2] = 4", "SET foo[2] = 4"),
    ("SET foo.bar = 4", "SET foo.bar = 4"),
    ("SET foo = if_not_exists(foo, 2)", "SET foo = if_not_exists(foo, 2)"),
    ("SET foo = list_append(foo, 2)", "SET foo = list_append(foo, 2)"),
    ("SET foo = list_append(2, foo)", "SET foo = list_append(2, foo)"),
    ("REMOVE foo", "REMOVE foo"),
    ("REMOVE foo, bar", "REMOVE foo, bar"),
    ("REMOVE foo[0]", "REMOVE foo[0]"),
    ("REMOVE foo.bar", "REMOVE foo.bar"),
    ("ADD foo 1", "ADD foo 1"),
    ('ADD foo 1, bar "a"', "ADD foo 1, bar 'a'"),
    ("DELETE foo 1", "DELETE foo 1"),
    ("DELETE foo 1, bar 2", "DELETE foo 1, bar 2"),
]

SELECTION = [
    ("foo", "foo"),
    ("foo + bar", "(foo + bar)"),
    ("foo + bar * baz", "(foo + (bar * baz))"),
    ("foo - (bar - baz)", "(foo - (bar - baz))"),
    ("foo + bar AS baz", "(foo + bar) AS baz"),
    ("foo + 2", "(foo + 2)"),
    ("*", ""),
    ("count(*)", ""),
    ("timestamp(foo)", "TIMESTAMP(foo)"),
    ("utcts(foo - bar)", "UTCTIMESTAMP((foo - bar))"),
    ("now() - now()", "(NOW() - NOW())"),
]


class TestExpressions(TestCase):
    """Tests for expression parsing and building"""

    def _run_test(self, expression, expected, grammar, key, factory):
        """Parse an expression, build it, and compare"""
        grammar = grammar + StringEnd()
        try:
            parse_result = grammar.parseString(expression)
            const = factory(parse_result[key])
            self.assertEqual(const, expected)
        except AssertionError:
            print("Expression: %s" % expression)
            print("Expected  : %s" % expected)
            print("Got       : %s" % const)
            self.fail("Parsing expression produced incorrect result")
        except ParseException as e:
            print(expression)
            print(" " * e.loc + "^")
            self.fail("Failed to parse expression")
        except Exception:
            print(expression)
            raise

    def test_constraints(self):
        """Test parsing constraint expressions (WHERE ...)"""
        for expression, expected in CONSTRAINTS:
            self._run_test(expression, expected, where, "where", lambda x: x)

    def test_updates(self):
        """Test parsing update expressions (SET ...)"""
        for expression, expected in UPDATES:
            self._run_test(
                expression,
                expected,
                update_expr,
                "update",
                lambda x: str(UpdateExpression.from_update(x)),
            )

    def test_selection(self):
        """Test parsing selection expressions (SELECT ...)"""
        for expression, expected in SELECTION:
            self._run_test(
                expression,
                expected,
                selection,
                "attrs",
                lambda x: str(SelectionExpression.from_selection(x)),
            )
