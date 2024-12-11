import pytest
import tempfile
import shutil
from mock import patch
from dql import DQLClient
import rich

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
    Set the console width to 200 characters for test consistency.
    """
    rich.get_console().width = 200
    yield
    rich.get_console().width = None
