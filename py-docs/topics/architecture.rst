.. _architecture:

Architecture
============
This document describes how DQL is put together internally. It is aimed at
contributors and anyone who wants to understand how a line of DQL becomes a call
to DynamoDB. For the user-facing query syntax, see :ref:`queries`.

Overview
--------
DQL is a SQL-ish language for Amazon DynamoDB. Running a statement flows through
four stages:

.. code-block:: text

    ┌──────────┐   ┌───────────┐   ┌───────────────┐   ┌──────────┐
    │  Parse   │──▶│  Plan &   │──▶│  Call         │──▶│  Format  │
    │ (grammar)│   │  Execute  │   │  DynamoDB     │   │ & Display│
    │          │   │ (engine)  │   │  (dynamo3)    │   │ (output) │
    └──────────┘   └───────────┘   └───────────────┘   └──────────┘
         ▲               │                                    ▲
         │               ▼                                    │
    text input     build DynamoDB                        REPL / -c
    from REPL      expressions from                      output sink
    or -c flag     the AST (expressions/)

- **Parse** – :mod:`dql.grammar` turns the SQL-ish text into a ``pyparsing``
  parse tree. WHERE, SELECT and UPDATE fragments are converted into an
  expression AST (:mod:`dql.expressions`) during parsing.
- **Plan & execute** – :class:`dql.engine.Engine` dispatches on the statement
  type, chooses an index (query vs. scan), builds the DynamoDB expression
  strings, and calls the connection.
- **Call DynamoDB** – all network I/O goes through ``dynamo3`` /
  ``botocore``.
- **Format & display** – for the REPL, :class:`dql.cli.DQLClient` renders the
  results with a formatter from :mod:`dql.output` and pipes them to a display
  sink (stdout or ``less``).

Component map
-------------
The package layers, from the top (user) down to the wire:

.. code-block:: text

    dql/
    ├── __init__.py          entry point (main), logging config, public API
    ├── cli.py               DQLClient – the cmd.Cmd REPL and -c runner
    │
    ├── engine.py            Engine / FragmentEngine – execution + planning
    ├── models.py            TableMeta / QueryIndex – schema metadata + index picking
    │
    ├── grammar/             pyparsing grammar (text -> parse tree)
    │   ├── common.py            lexical tokens and literals
    │   ├── parsed_primitives.py evaluated literals for WHERE
    │   ├── query.py             SELECT/WHERE/KEYS IN clauses
    │   └── __init__.py          top-level statements + parser singletons
    │
    ├── expressions/         AST for query fragments (parse tree -> DynamoDB expr)
    │   ├── base.py              Expression / Field / Value
    │   ├── visitor.py           Visitor – placeholder encoding (#fN / :vN)
    │   ├── constraint.py        WHERE conditions + query planning
    │   ├── selection.py         SELECT projection + client-side evaluation
    │   └── update.py            UPDATE SET/ADD/REMOVE/DELETE
    │
    ├── output.py            result formatters + pager / display sinks
    ├── monitor.py           curses capacity dashboard (`watch`)
    ├── throttle.py          session-level rate limiting (`throttle`)
    ├── history.py           persistent REPL history
    ├── help.py              static help text for statements and options
    ├── util.py              terminal sizing, literal resolution, file I/O
    ├── exceptions.py        EngineRuntimeError, ExplainSignal
    ├── pyparsing_compat.py  shim for pyparsing 2.1.4 on Python 3.10+
    └── readline_compat.py   cross-platform readline setup

Only ``Engine``, ``FragmentEngine`` and ``DQLClient`` are exported from the
package (:mod:`dql`); everything else is internal infrastructure.

The grammar layer
-----------------
:mod:`dql.grammar` is a ``pyparsing`` grammar (pinned to ``pyparsing==2.1.4``).
Grammars are built once at import time into module-level singletons — there is
no per-request parser construction.

Modules
~~~~~~~
- ``common.py`` – lexical primitives: keywords (via ``upkey``), identifiers
  (``var``, ``table``, ``index_name``), types, and *syntactic* literals
  (``number``, ``string``, ``set_``, ``list_``, ``dict_``, timestamp
  expressions). These leave values as parse tokens.
- ``parsed_primitives.py`` – the same literals but with ``setParseAction``
  handlers that evaluate them to real Python types (``Decimal``, ``Binary``,
  ``set``, timestamps via :func:`dql.util.dt_to_ts`). Used where a concrete
  value is needed at parse time.
- ``query.py`` – query clauses shared by reads: the ``selection`` list
  (``*``, ``count(*)``, arithmetic, ``AS`` aliases), the ``where`` clause
  (``NOT`` / ``AND`` / ``OR`` with precedence via ``infixNotation``),
  ``KEYS IN``, and ``LIMIT`` / ``SCAN LIMIT``. Each WHERE predicate is wired to
  a :mod:`dql.expressions.constraint` class through its ``from_parser`` factory.
- ``__init__.py`` – assembles the top-level statements and exports the parser
  singletons.

Dual literal pipeline
~~~~~~~~~~~~~~~~~~~~~~~
There are two literal pipelines by design. ``INSERT`` values and ``KEYS IN``
tuples keep their *syntactic* parse trees (from ``common``) and are resolved
later by :func:`dql.util.resolve`. ``WHERE`` constraints are *evaluated* at
parse time (from ``parsed_primitives``) because the engine needs concrete
values to choose an index and to build the API call.

Exported parsers
~~~~~~~~~~~~~~~~~
- ``parser`` – one or more ``;``-separated statements; used by
  :meth:`Engine.execute`.
- ``statement_parser`` – a single statement with an optional trailing ``;``.
- ``line_parser`` – a completeness detector for the REPL: it only succeeds when
  the buffered input forms a complete statement ending in ``;`` (see
  ``FragmentEngine`` below).

Supported statements
~~~~~~~~~~~~~~~~~~~~~~
``SELECT``, ``SCAN``, ``INSERT``, ``UPDATE``, ``DELETE``, ``CREATE TABLE``,
``DROP TABLE``, ``ALTER TABLE``, ``DUMP SCHEMA``, ``LOAD``, plus the
``EXPLAIN`` and ``ANALYZE`` wrappers. ``THROTTLE(...)`` and ``THROUGHPUT``/``TP``
are clauses, not top-level statements. Comments use ``-- ...`` to end of line.

Each statement tags its leading keyword with ``.setResultsName("action")`` and
attaches named child fields (``table``, ``where``, ``using``, ``limit``,
``throttle``, ...). The engine dispatches on ``tree.action`` and reads those
named fields.

The expression AST and the Visitor
----------------------------------
:mod:`dql.expressions` is an AST for the query fragments (WHERE, SELECT,
UPDATE). Every node implements ``build(visitor)`` to render itself as a
DynamoDB expression string, and a parallel :class:`~dql.expressions.Visitor`
accumulates the placeholder maps DynamoDB requires.

- ``base.py`` – :class:`Expression` (abstract), :class:`Field` (an attribute
  path) and :class:`Value` (a literal). ``Field``/``Value`` also implement
  ``evaluate(item)`` for *client-side* computation used by selections.
- ``visitor.py`` – :class:`Visitor` encodes field path segments as
  ``ExpressionAttributeNames`` (``#f1``, ``#f2`` ...) when they are reserved
  words, contain ``-``, or start with ``_``, and always encodes literals as
  ``ExpressionAttributeValues`` (``:v1``, ``:v2`` ...). ``DummyVisitor`` is an
  identity encoder used by ``Expression.__str__`` for tests and debugging.

.. note::

   A single ``Visitor`` instance must be used for one complete statement build
   so the ``#fN`` / ``:vN`` numbering stays consistent across all
   sub-expressions.

The three expression roots map onto DynamoDB parameters:

======================  ==================================  =========================================
DQL construct           Class                               DynamoDB parameter
======================  ==================================  =========================================
``WHERE`` (query keys)  ``ConstraintExpression``            ``KeyConditionExpression``
``WHERE`` (remainder)   ``ConstraintExpression``            ``FilterExpression``
``WHERE`` (update/del)  ``ConstraintExpression``            ``ConditionExpression``
``SELECT attrs``        ``SelectionExpression``             ``ProjectionExpression``
``SELECT count(*)``     ``SelectionExpression(is_count)``   ``Select=COUNT``
``UPDATE SET/ADD/...``  ``UpdateExpression``                ``UpdateExpression``
all of the above        ``Visitor``                         ``ExpressionAttributeNames/Values``
======================  ==================================  =========================================

Constraints and query planning
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
``constraint.py`` holds the WHERE AST (``OperatorConstraint``,
``FunctionConstraint``, ``BetweenConstraint``, ``InConstraint``,
``SizeConstraint``, ``TypeConstraint``, and the ``Invert`` / ``Conjunction``
combinators). Beyond rendering, these classes drive **query planning**:

- ``possible_hash_fields()`` / ``possible_range_fields()`` report which
  attributes could satisfy an index hash/range key.
- ``remove_index(index)`` splits an AND-conjunction into the part that becomes
  the ``KeyConditionExpression`` and the remainder that becomes the
  ``FilterExpression``.

Selection: projection vs. client-side evaluation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
``selection.py`` serves two roles. ``build()`` collects the attribute names for
the ``ProjectionExpression``; but arithmetic, timestamp functions and aliases
are **not** translated to DynamoDB — DQL fetches the underlying attributes and
then evaluates the expression locally in ``SelectionExpression.convert()``. If
a chosen index does not project all the needed attributes, the engine issues a
follow-up ``batch_get`` to fill them in.

The execution engine
---------------------
:class:`dql.engine.Engine` is the core. It owns the DynamoDB connection,
caches table metadata, plans queries, translates the AST into API kwargs, and
handles throttling and EXPLAIN/ANALYZE.

Dispatch
~~~~~~~~
:meth:`Engine.execute` runs ``parser.parseString`` and then calls the private
``_run(tree)`` for each statement. ``_run`` first peels off any ``THROTTLE``
clause, then dispatches on ``tree.action`` to a handler:

======================  ====================================================
``tree.action``         Handler
======================  ====================================================
``SELECT`` / ``SCAN``   ``_select`` (``_scan`` is ``_select(tree, True)``)
``INSERT``              ``_insert``
``UPDATE``              ``_update`` (via ``_query_and_op``)
``DELETE``              ``_delete`` (via ``_query_and_op``)
``CREATE``              ``_create``
``DROP``                ``_drop``
``ALTER``               ``_alter``
``DUMP``                ``_dump``
``LOAD``                ``_load``
``EXPLAIN``             ``_explain``
``ANALYZE``             sets ``_analyzing`` then re-runs the inner statement
======================  ====================================================

Connection and metadata cache
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
``Engine.connect()`` proxies to ``dynamo3.DynamoDBConnection.connect``. On
connect the engine subscribes ``_on_capacity_data`` to the connection's
``capacity`` event, enables ``default_return_capacity``, and clears the caches.
``describe()`` returns a :class:`dql.models.TableMeta` from
``cached_descriptions`` (or fetches it), optionally enriching it with
CloudWatch consumed-capacity metrics.

Query planning / index selection
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
``_build_query`` decides between a DynamoDB *query* and a *scan*:

1. ``USING index`` forces a specific index; ``USING -`` disables auto-selection.
2. Otherwise the WHERE ``ConstraintExpression`` reports its possible hash/range
   fields, and :meth:`TableMeta.get_matching_indexes` finds candidate indexes:

   - **0 matches** → scan with the whole WHERE as a filter.
   - **1 match** (the base ``TABLE`` wins ties) → query; ``add_query_kwargs``
     splits the key condition from the filter via ``remove_index``.
   - **>1 match** → ``SyntaxError`` asking the user to disambiguate with
     ``USING``.

3. No WHERE → scan.

By default a ``SELECT`` that resolves to a scan raises an error (unless
``allow_select_scan`` is set); ``SCAN`` bypasses that guard. This is why the
same grammar backs both keywords — only the engine behavior differs.

Reads, writes, and pagination
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
- ``KEYS IN`` reads become ``batch_get`` using resolved primary keys.
- ``UPDATE`` / ``DELETE`` share ``_query_and_op``: first resolve the target
  keys (via KEYS IN or a query/scan for PK attributes only), then fan out
  ``update_item`` / ``delete_item`` calls across a thread pool with a progress
  bar. A statement with no WHERE triggers the ``caution_callback`` (the CLI
  prompts for confirmation).
- Pagination is handled by ``dynamo3`` — ``query`` / ``scan`` return iterables
  that transparently follow ``LastEvaluatedKey``.

EXPLAIN and ANALYZE
~~~~~~~~~~~~~~~~~~~~~
``EXPLAIN`` monkey-patches ``connection.call`` with a fake that logs each
``(command, kwargs)`` and raises :class:`~dql.exceptions.ExplainSignal`; the
engine catches it in ``execute`` and returns the formatted call list without
touching data. ``ANALYZE`` actually runs the statement but accumulates the
consumed capacity reported through ``_on_capacity_data`` for the CLI to print.

Throttling
~~~~~~~~~~
There are two independent throttling mechanisms, both funneling through the
connection's ``capacity`` event and the ``_on_throttle`` sleep callback:

- **Statement-level** – a ``THROTTLE(...)`` clause becomes a per-statement
  ``dynamo3.RateLimit`` (``Engine._query_rate_limit``).
- **Session-level** – the REPL ``throttle`` command configures a
  :class:`dql.throttle.TableLimits`, which the CLI turns into
  ``engine.rate_limit`` before each command.

Schema metadata (models)
------------------------
:mod:`dql.models` is the in-memory model of a table's schema and the hub for
query planning and schema round-tripping.

- :class:`TableMeta` – built from a ``dynamo3`` description via
  ``from_description``. It exposes the attributes, local and global indexes,
  consumed capacity, ``primary_key`` helpers, aggregate throughput, and a
  ``schema`` property that regenerates ``CREATE TABLE`` DDL (used by
  ``DUMP SCHEMA``). Its ``get_matching_indexes`` and ``iter_query_indexes``
  drive index selection, and ``pformat`` renders the ``ls`` output.
- :class:`QueryIndex` – a lightweight descriptor (name, global/local,
  hash/range keys, projected attributes) used to reason about whether an index
  is usable and whether it ``projects_all_attributes`` needed by a selection.
- ``TableField`` / ``IndexField`` / ``GlobalIndexMeta`` model individual
  attributes and indexes and emit their own DDL fragments.

The CLI / REPL
--------------
:class:`dql.cli.DQLClient` subclasses :class:`cmd.Cmd`. The package entry point
``dql.main`` (in ``dql/__init__.py``) parses arguments, builds a ``DQLClient``,
calls ``initialize()`` and then either ``start()`` (interactive REPL) or
``run_command()`` (the ``-c`` one-shot mode).

- DQL statements are **not** ``do_*`` methods — they fall through
  ``cmd.Cmd``'s ``default()`` into ``_run_cmd``. Only the REPL meta-commands
  (``opt``, ``ls``, ``use``, ``watch``, ``throttle``, ``file``, ``local``,
  ``whoami``, ``exit`` ...) are ``do_*`` methods.
- ``_run_cmd`` applies the session rate limit, calls ``engine.execute``, and
  routes the result: ``None`` means an incomplete fragment (wait for more
  input), a ``str`` is a pretty status line printed directly, and anything else
  is rendered by a formatter inside a display context manager.
- Options are persisted to ``~/.config/dql.json``; history to ``~/.dql/history``.

Fragment handling
~~~~~~~~~~~~~~~~~~
The CLI uses :class:`dql.engine.FragmentEngine`, a subclass of ``Engine`` that
lets multi-line statements accumulate until they are complete. Each input line
is appended to a buffer and tested with ``line_parser``; while the buffer is not
a complete statement, ``execute`` returns ``None`` and the prompt switches to
the ``| `` continuation form. Once a terminating ``;`` completes the statement,
it is handed to the normal ``Engine.execute``.

Output and formatting
---------------------
:mod:`dql.output` provides the result formatters and the display sinks used by
the CLI (the engine only borrows its ``console`` for throttle log messages).

- **Formatters** (chosen by the ``format`` option): ``SmartFormat`` (picks
  ``ColumnFormat`` or falls back to ``ExpandedFormat`` by width),
  ``ColumnFormat``, ``ExpandedFormat``, ``JsonFormat``, and ``RichFormat``.
- **Display sinks** (the ``display`` option): ``stdout_display`` writes to
  stdout; ``less_display`` writes to a temp file piped through ``less -FXR``.
- ``RichFormat`` reads ``engine.parsed_information`` (the parsed tree, chosen
  ``TableMeta``, ``QueryIndex`` and the DynamoDB query kwargs) to highlight and
  order primary-key and index columns.

Supporting modules
------------------
- :mod:`dql.monitor` – ``Monitor`` renders a curses capacity dashboard for the
  ``watch`` command, refreshing consumed vs. provisioned capacity every 30s via
  ``engine.describe(..., metrics=True)``.
- :mod:`dql.history` – ``HistoryManager`` loads and appends readline history and
  trims noise commands (like ``exit``) from the saved history.
- :mod:`dql.util` – shared helpers: terminal sizing (``getmaxyx``), the central
  :func:`resolve` that turns parse trees into Python values, timestamp/interval
  evaluation, and gzip-aware file I/O (``open_file_smart_mode``) for ``SAVE`` /
  ``LOAD``.
- :mod:`dql.help` – static help strings surfaced by the ``help_*`` REPL methods.
- :mod:`dql.exceptions` – ``EngineRuntimeError`` (missing table / unknown
  index) and ``ExplainSignal`` (the EXPLAIN control-flow signal).
- :mod:`dql.pyparsing_compat` – restores ``collections.MutableMapping`` /
  ``collections.Sequence`` so ``pyparsing==2.1.4`` runs on Python 3.10+; it must
  be imported before ``pyparsing`` (done in ``cli``, ``engine``, ``grammar``
  and the test ``conftest``).
- :mod:`dql.readline_compat` – prefers ``gnureadline``, wires up tab completion
  across GNU/libedit, and keeps ``-`` out of the completer delimiters so
  hyphenated table names complete correctly.

End-to-end example: a SELECT
----------------------------
Tracing ``SELECT id, count FROM posts WHERE username = 'steve';``:

1. **Parse** – ``parser.parseString`` produces a tree with
   ``action="SELECT"``, ``table="posts"``, an ``attrs`` selection, and a
   ``where`` that is already a ``ConstraintExpression`` (an
   ``OperatorConstraint`` for ``username = 'steve'``).
2. **Dispatch** – ``_run`` sees ``SELECT`` and calls ``_select``.
3. **Plan** – ``_build_query`` asks the constraint for possible hash fields
   (``username``) and calls ``TableMeta.get_matching_indexes``. Suppose the
   base table hash key is ``username`` — it matches, so this becomes a *query*.
4. **Build** – a ``Visitor`` encodes the key condition
   (``#f1 = :v1`` with ``count`` also encoded because it is a reserved word),
   and ``SelectionExpression.build`` produces the projection.
5. **Call** – ``connection.query`` is invoked with the key condition,
   projection, and the ``ExpressionAttributeNames`` / ``ExpressionAttributeValues``
   maps; ``dynamo3`` handles pagination.
6. **Convert & format** – ``SelectionExpression.convert`` shapes each returned
   item, and the CLI renders the rows through the configured formatter and
   display sink.
