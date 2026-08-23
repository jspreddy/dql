"""Pytest options for the black-box suite. Cli fixtures land in a later commit."""

from __future__ import annotations

import pytest


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--bin",
        action="store",
        default="both",
        choices=("dql", "dqlrs", "both"),
        help="Which CLI binary to run against (default: both, skipping missing)",
    )
    parser.addoption(
        "--start-local",
        action="store_true",
        default=False,
        help="Start DynamoDB Local if the port is down",
    )
    parser.addoption(
        "--skip-teardown",
        action="store_true",
        default=False,
        help="Do not DROP TABLE after each test",
    )
    parser.addoption(
        "--group",
        action="store",
        default="",
        help="Numeric group (1xx, 11x, 2xx) or substring of the test name",
    )
