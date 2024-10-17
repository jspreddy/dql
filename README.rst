DQL
===
:Build: |build|_ |coverage|_
:Documentation: http://dql.readthedocs.org/
:Downloads: http://pypi.python.org/pypi/dql
:Source: https://github.com/stevearc/dql

.. |build| image:: https://github.com/stevearc/dql/actions/workflows/code-workflows.yml/badge.svg
.. _build: https://github.com/stevearc/dql/actions/workflows/code-workflows.yml
.. |coverage| image:: https://coveralls.io/repos/stevearc/dql/badge.png?branch=master
.. _coverage: https://coveralls.io/r/stevearc/dql?branch=master

A simple, SQL-ish language for DynamoDB

As of November 2020, Amazon has released `PartiQL
support <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ql-reference.html>`__
for DynamoDB. You should investigate that first to see if it addresses your
needs.

Getting Started
---------------
Installation can be done in a variety of ways

* An executable `pex <https://github.com/pantsbuild/pex>`__ file is available on `the release page <https://github.com/stevearc/dql/releases>`__.
* You can run a script to generate the pex file yourself: ``curl -o- install.py https://raw.githubusercontent.com/stevearc/dql/master/bin/install.py | python``
* With pip: ``pip install dql`` (To get the official version)
* If you want my development version, see branch `v-next`_

Prerequisites for development version: ``pipx, pyenv``

Installing from remote source code::

    # 1. Install python
    pyenv install 3.9.19
    # 2. Set active python environment
    pyenv shell 3.9.19
    # 3. Install dql
    pipx install --python python3.9 git+https://github.com/jspreddy/dql.git@v-next

Install from local source code::

    # 1. Clone repo,
    git clone https://github.com/jspreddy/dql.git
    # 2. checkout branch `v-next`
    git checkout v-next
    # 3. init python environment
    pyenv install 3.9.19
    pyenv shell 3.9.19
    # 4. editable install
    pipx install --python python3.9 -e .


Examples
--------

Here are some basic DQL examples to get you going:

Start the REPL::

    $ dql
    us-west-1>

Creating a table::

    us-west-1> CREATE TABLE forum_threads (name STRING HASH KEY,
             >                             subject STRING RANGE KEY,
             >                             THROUGHPUT (4, 2));

Inserting data::

    us-west-1> INSERT INTO forum_threads (name, subject, views, replies)
             > VALUES ('Self Defense', 'Defense from Banana', 67, 4),
             > ('Self Defense', 'Defense from Strawberry', 10, 0),
             > ('Cheese Shop', 'Anyone seen the camembert?', 16, 1);

Queries::

    us-west-1> SCAN * FROM forum_threads;
    us-west-1> SELECT count(*) FROM forum_threads WHERE name = 'Self Defense';
    us-west-1> SELECT * FROM forum_threads WHERE name = 'Self Defense';

Mutations::

    us-west-1> UPDATE forum_threads ADD views 1 WHERE
             > name = 'Self Defense' AND subject = 'Defense from Banana';
    us-west-1> DELETE FROM forum_threads WHERE name = 'Cheese Shop';

Changing tables::

    us-west-1> ALTER TABLE forum_threads SET THROUGHPUT (8, 4);
    us-west-1> DROP TABLE forum_threads;

And don't forget to use ``help``!


Developer/Maintainer Guide:
---------------------------
`See here for developer guide for v-next branch. <https://github.com/jspreddy/dql/blob/v-next/doc/topics/develop.rst>`
