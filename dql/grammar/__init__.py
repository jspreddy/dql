""" DQL language parser """
from pyparsing import (delimitedList, Optional, Group, restOfLine, Keyword,
                       Suppress, ZeroOrMore, oneOf, StringEnd)

from .common import (from_, table, var, value, table_key, into, type_, upkey,
                     set_)
from .query import (where, select_where, limit, if_exists, if_not_exists,
                    using, filter_)


def create_throughput():
    """ Create a throughput specification """
    return (upkey('throughput') + Suppress('(') +
            Group(value + Suppress(',') + value).setResultsName('throughput') +
            Suppress(')'))

# pylint: disable=C0103
throughput = create_throughput()
# pylint: enable=C0103

# pylint: disable=W0104,W0106


def create_select():
    """ Create the grammar for the 'select' statement """
    select = upkey('select').setResultsName('action')
    attrs = Group(Keyword('*') |
                  (Optional(Suppress('(')) + delimitedList(var) +
                   Optional(Suppress(')'))))\
        .setResultsName('attrs')
    ordering = Optional(upkey('desc') | upkey('asc')).setResultsName('order')

    return (select + attrs + from_ + table + select_where +
            Optional(using + value).setResultsName('using') +
            Optional(limit) + ordering)


def create_scan():
    """ Create the grammar for the 'scan' statement """
    scan = upkey('scan').setResultsName('action')
    return (scan + table + Optional(filter_) + Optional(limit))


def create_count():
    """ Create the grammar for the 'count' statement """
    count = upkey('count').setResultsName('action')

    return (count + table + where +
            Optional(using + value).setResultsName('using'))


def create_create():
    """ Create the grammar for the 'create' statement """
    create = upkey('create').setResultsName('action')
    hash_key = Group(upkey('hash') +
                     upkey('key'))
    range_key = Group(upkey('range') +
                      upkey('key'))

    # ATTR DECLARATION
    index = Group(upkey('index') + Suppress('(') +
                  value + Suppress(')'))
    index_type = (hash_key | range_key | index)\
        .setName('index specification').setResultsName('index')
    attr_declaration = Group(var.setResultsName('name') + type_ + index_type)\
        .setName('attr').setResultsName('attr')
    attrs_declaration = (Suppress('(') +
                         Group(delimitedList(attr_declaration))
                         .setName('attrs').setResultsName('attrs')
                         + Suppress(')'))

    return (create + table_key + Optional(if_not_exists) + table +
            attrs_declaration + Optional(throughput))


def create_delete():
    """ Create the grammar for the 'delete' statement """
    delete = upkey('delete').setResultsName('action')
    return (delete + from_ + table + select_where +
            Optional(using + value).setResultsName('using'))


def create_insert():
    """ Create the grammar for the 'insert' statement """
    insert = upkey('insert').setResultsName('action')
    attrs = Group(delimitedList(var)).setResultsName('attrs')

    # VALUES
    values_key = upkey('values')
    value_group = Group(Suppress('(') + delimitedList(value) + Suppress(')'))
    values = Group(delimitedList(value_group)).setResultsName('data')

    return (insert + into + table + Suppress('(') + attrs + Suppress(')') +
            values_key + values)


def create_drop():
    """ Create the grammar for the 'drop' statement """
    drop = upkey('drop').setResultsName('action')
    return (drop + table_key + Optional(if_exists) + table)


def create_update():
    """ Create the grammar for the 'update' statement """
    update = upkey('update').setResultsName('action')
    returns, none, all_, updated, old, new = \
        map(upkey, ['returns', 'none', 'all', 'updated', 'old',
                    'new'])
    set_op = oneOf('= += -=', caseless=True).setName('operator')
    clause = Group(var + set_op + value)
    set_values = Group(delimitedList(clause)).setResultsName('updates')
    return_ = returns + Group(none |
                             (all_ + old) |
                             (all_ + new) |
                             (updated + old) |
                             (updated + new))\
        .setResultsName('returns')
    return (update + table + set_ + set_values + Optional(select_where) +
            Optional(return_))


def create_alter():
    """ Create the grammar for the 'alter' statement """
    alter = upkey('alter').setResultsName('action')
    return alter + table_key + table + set_ + throughput


def create_dump():
    """ Create the grammar for the 'dump' statement """
    dump = upkey('dump').setResultsName('action')
    return (dump + upkey('schema') +
            Optional(Group(delimitedList(var)).setResultsName('tables')))


def create_parser():
    """ Create the language parser """
    dql = (create_select() |
           create_scan() |
           create_count() |
           create_delete() |
           create_update() |
           create_create() |
           create_insert() |
           create_drop() |
           create_alter() |
           create_dump()
           )

    dql.ignore('--' + restOfLine)

    return dql

# pylint: disable=C0103
_statement = create_parser()
statement_parser = _statement + Suppress(';' | StringEnd())
parser = Group(_statement) + ZeroOrMore(Suppress(
    ';') + Group(_statement)) + Suppress(';' | StringEnd())
