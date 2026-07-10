pub const GENERAL: &str = "Type 'help <statement>' for help on DQL statements.\n\
Meta-commands: opt, version, exit, clear, shell, whoami, use, local, file, ls, throttle, unthrottle, help\n";

pub fn statement_help(topic: &str) -> Option<&'static str> {
    match topic.to_ascii_lowercase().as_str() {
        "alter" => Some(ALTER),
        "analyze" => Some(ANALYZE),
        "create" => Some(CREATE),
        "delete" => Some(DELETE),
        "drop" => Some(DROP),
        "dump" => Some(DUMP),
        "explain" => Some(EXPLAIN),
        "insert" => Some(INSERT),
        "load" => Some(LOAD),
        "scan" => Some(SCAN),
        "select" => Some(SELECT),
        "update" => Some(UPDATE),
        "options" => Some(OPTIONS),
        _ => None,
    }
}

pub const ALTER: &str = r"
    Alter a table's throughput or create/drop global indexes

    ALTER TABLE tablename
        SET [INDEX index] THROUGHPUT throughput
    ALTER TABLE tablename
        DROP INDEX index [IF EXISTS]
    ALTER TABLE tablename
        CREATE GLOBAL [ALL|KEYS|INCLUDE] INDEX global_index [IF NOT EXISTS]

    Examples
    --------
    ALTER TABLE foobars SET THROUGHPUT (4, 8);
    ALTER TABLE foobars SET THROUGHPUT (7, *);
    ALTER TABLE foobars SET INDEX ts-index THROUGHPUT (5, *);
    ALTER TABLE foobars DROP INDEX ts-index;
    ALTER TABLE foobars CREATE GLOBAL INDEX ('ts-index', ts NUMBER, THROUGHPUT (5, 5));
";

pub const ANALYZE: &str = r"
    Run a query and print out the consumed capacity

    ANALYZE query

    Examples
    --------
    ANALYZE SELECT * FROM foobars WHERE id = 'a';
    ANALYZE INSERT INTO foobars (id, name) VALUES (1, 'dsa');
";

pub const CREATE: &str = r"
    Create a new table

    CREATE TABLE
        [IF NOT EXISTS]
        tablename
        attributes
        [GLOBAL [ALL|KEYS|INCLUDE] INDEX global_index]

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY);
    CREATE TABLE IF NOT EXISTS foobars (id STRING HASH KEY);
";

pub const DELETE: &str = r"
    Delete items from a table

    DELETE FROM
        tablename
        [ KEYS IN primary_keys ]
        [ WHERE expression ]
        [ USING index ]

    Examples
    --------
    DELETE FROM foobars WHERE foo != 'bar' AND baz >= 3;
";

pub const DROP: &str = r"
    Delete a table

    DROP TABLE
        [ IF EXISTS ]
        tablename

    Examples
    --------
    DROP TABLE foobars;
    DROP TABLE IF EXISTS foobars;
";

pub const DUMP: &str = r"
    Print the schema creation statements for your tables

    DUMP SCHEMA [ tablename [, ...] ]

    Examples
    --------
    DUMP SCHEMA;
    DUMP SCHEMA foobars, widgets;
";

pub const EXPLAIN: &str = r"
    Print out the DynamoDB queries that will be executed for a command

    EXPLAIN query

    Examples
    --------
    EXPLAIN SELECT * FROM foobars WHERE id = 'a';
";

pub const INSERT: &str = r"
    Insert data into a table

    INSERT INTO tablename
        attributes VALUES values
    INSERT INTO tablename
        items

    Examples
    --------
    INSERT INTO foobars (id) VALUES (1);
    INSERT INTO foobars (id='foo', bar=10);
";

pub const LOAD: &str = r"
    Load data from a file (saved with SELECT ... SAVE) into a table

    LOAD filename INTO tablename

    Formats: .json / .csv / .msgpack (optional .gz). Pickle (.p) is not supported.

    Examples
    --------
    LOAD archive.msgpack INTO mytable;
    LOAD archive.json.gz INTO mytable;
";

pub const SCAN: &str = SELECT;
pub const SELECT: &str = r"
    Select items from a table by querying an index

    SELECT
        [ CONSISTENT ]
        attributes
        FROM tablename
        [ KEYS IN primary_keys | WHERE expression ]
        [ USING index ]
        [ LIMIT limit ]
        [ ORDER BY field ]
        [ ASC | DESC ]
        [ SAVE file.json ]

    Examples
    --------
    SELECT * FROM foobars WHERE foo = 'bar';
    SELECT * FROM foobars KEYS IN 'id1', 'id2';
    SELECT * FROM foobars SAVE out.msgpack;
    SELECT * FROM foobars SAVE out.json.gz;
";

pub const UPDATE: &str = r"
    Update items in a table

    UPDATE tablename
        update_expression
        [ KEYS IN primary_keys ]
        [ WHERE expression ]
        [ USING index ]

    Examples
    --------
    UPDATE foobars SET foo = 'a';
    UPDATE foobars ADD foo 1, bar 4;
";

pub const OPTIONS: &str = r"
    Get or set options

    use 'opt <option>' to get the value of option, and 'opt <option> <value>'
    to set the option.

                width : int, The number of characters wide to format the display
             pagesize : int, The number of results to get per page for queries
              display : (less|stdout), The reader used to view query results
               format : (smart|column|expanded|json), Display format for query results
    allow_select_scan : bool, If True, SELECT statements can perform table scans
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_docs() {
        for topic in [
            "alter", "analyze", "create", "delete", "drop", "dump", "explain", "insert", "load",
            "scan", "select", "update", "options",
        ] {
            assert!(statement_help(topic).is_some(), "missing help for {topic}");
        }
    }
}
