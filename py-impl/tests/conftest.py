import shutil
import tempfile

import pytest
import rich
from mock import patch

import dql.pyparsing_compat  # noqa: F401
from dql import DQLClient


@pytest.fixture(scope="session")
def cli():
    dql_client = DQLClient()
    confdir = tempfile.mkdtemp()
    print("TestStructure::BaseCLITest::setUpClass: Running the initialization")
    dql_client.initialize(
        host="localhost",
        port=8000,
        config_dir=confdir,
    )
    # Have to patch this so we don't make requests to CloudWatch
    patcher = patch("dql.engine.Engine._get_metric", spec=True)
    method = patcher.start()
    method.return_value = 0

    yield dql_client

    shutil.rmtree(confdir)
    patcher.stop()


@pytest.fixture(scope="session", autouse=True)
def cli_window_size():
    """
    Set the console size to 200x50 for test consistency.

    Rich ignores a width override unless height is set as well, and otherwise
    falls back to the real terminal width.
    """
    console = rich.get_console()
    console.width = 200
    console.height = 50
    yield
    console.width = None  # type: ignore[assignment]
    console.height = None  # type: ignore[assignment]
