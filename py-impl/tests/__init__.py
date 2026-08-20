""" Testing tools for DQL """

import unittest

import pytest
from dynamo3 import DynamoDBConnection

from dql import Engine


class BaseSystemTest(unittest.TestCase):
    """Base class for system tests"""

    dynamo: DynamoDBConnection
    engine: Engine

    @pytest.fixture(autouse=True)
    def setup_test(self):
        self.dynamo = DynamoDBConnection.connect(
            region="us-west-1",
            host="localhost",
            port=8000,
            is_secure=False,
            access_key="test",
            secret_key="test",
        )
        self.engine = Engine(self.dynamo)
        # Clear out any pre-existing tables
        for tablename in self.dynamo.list_tables():
            self.dynamo.delete_table(tablename)

        yield  # come back here for teardown

        # Teardown code
        for tablename in self.dynamo.list_tables():
            self.dynamo.delete_table(tablename)

    def query(self, command):
        """Shorthand because I'm lazy"""
        return self.engine.execute(command)

    def make_table(self, name="foobar", hash_key="id", range_key="bar", index=None):
        """Shortcut for making a simple table"""
        rng = ""
        if range_key is not None:
            rng = ",%s NUMBER RANGE KEY" % range_key
        idx = ""
        if index is not None:
            idx = ",{0} NUMBER INDEX('{0}-index')".format(index)
        self.query(
            "CREATE TABLE %s (%s STRING HASH KEY %s%s)" % (name, hash_key, rng, idx)
        )
        return name
