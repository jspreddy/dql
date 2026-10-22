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
    CREATE TABLE foobars (id STRING HASH KEY); ALTER TABLE foobars SET THROUGHPUT (4, 8);
    CREATE TABLE foobars (id STRING HASH KEY); ALTER TABLE foobars SET THROUGHPUT (7, *);
    CREATE TABLE foobars (id STRING HASH KEY, ts NUMBER) GLOBAL INDEX ('ts-index', ts NUMBER); ALTER TABLE foobars SET INDEX ts-index THROUGHPUT (5, *);
    CREATE TABLE foobars (id STRING HASH KEY, ts NUMBER) GLOBAL INDEX ('ts-index', ts NUMBER); ALTER TABLE foobars DROP INDEX ts-index;
    CREATE TABLE foobars (id STRING HASH KEY, ts NUMBER); ALTER TABLE foobars CREATE GLOBAL INDEX ('ts-index', ts NUMBER, THROUGHPUT (5, 5));
";

pub const ANALYZE: &str = r"
    Run a query and print out the consumed capacity

    ANALYZE query

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); ANALYZE SELECT * FROM foobars WHERE id = 'a';
    CREATE TABLE foobars (id STRING HASH KEY); ANALYZE INSERT INTO foobars (id, name) VALUES ('a', 'dsa');
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
    CREATE TABLE foobars (id STRING HASH KEY); DELETE FROM foobars WHERE foo != 'bar' AND baz >= 3;
";

pub const DROP: &str = r"
    Delete a table

    DROP TABLE
        [ IF EXISTS ]
        tablename

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); DROP TABLE foobars;
    DROP TABLE IF EXISTS foobars;
";

pub const DUMP: &str = r"
    Print the schema creation statements for your tables

    DUMP SCHEMA [ tablename [, ...] ]

    Examples
    --------
    DUMP SCHEMA;
    CREATE TABLE foobars (id STRING HASH KEY); CREATE TABLE widgets (id STRING HASH KEY); DUMP SCHEMA foobars, widgets;
";

pub const EXPLAIN: &str = r"
    Print out the DynamoDB queries that will be executed for a command

    Each operation includes the attribute names and expression values bound by the statement.

    EXPLAIN query

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); EXPLAIN SELECT * FROM foobars WHERE id = 'a';
";

pub const INSERT: &str = r"
    Insert data into a table

    INSERT INTO tablename
        attributes VALUES values
    INSERT INTO tablename
        items

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); INSERT INTO foobars (id) VALUES ('a');
    CREATE TABLE foobars (id STRING HASH KEY); INSERT INTO foobars (id='foo', bar=10);
";

pub const LOAD: &str = r"
    Load data from a file (saved with SELECT ... SAVE) into a table

    LOAD filename INTO tablename

    Formats: .json / .csv / .msgpack (optional .gz). Pickle (.p) is not supported.

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); INSERT INTO foobars (id) VALUES ('a'); SCAN * FROM foobars SAVE archive.msgpack; CREATE TABLE mytable (id STRING HASH KEY); LOAD archive.msgpack INTO mytable;
    CREATE TABLE foobars (id STRING HASH KEY); INSERT INTO foobars (id) VALUES ('a'); SCAN * FROM foobars SAVE archive.json.gz; CREATE TABLE mytable (id STRING HASH KEY); LOAD archive.json.gz INTO mytable;
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
        [ SAVE file.json [ CREATE PARENTS ] ]

    Examples
    --------
    CREATE TABLE foobars (id STRING HASH KEY); SELECT * FROM foobars WHERE id = 'a';
    CREATE TABLE foobars (id STRING HASH KEY); SELECT * FROM foobars KEYS IN 'id1', 'id2';
    CREATE TABLE foobars (id STRING HASH KEY); SCAN * FROM foobars SAVE out.msgpack;
    CREATE TABLE foobars (id STRING HASH KEY); SCAN * FROM foobars SAVE out.json.gz;
    CREATE TABLE foobars (id STRING HASH KEY); SCAN * FROM foobars SAVE 'exports/out.json' CREATE PARENTS;
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
    CREATE TABLE foobars (id STRING HASH KEY); UPDATE foobars SET foo = 'a' WHERE id = 'a';
    CREATE TABLE foobars (id STRING HASH KEY); UPDATE foobars ADD foo 1, bar 4 WHERE id = 'a';
";

pub const OPTIONS: &str = r"
    Get or set options

    use 'opt <option>' to get the value of option, and 'opt <option> <value>'
    to set the option.

                width : int, The number of characters wide to format the display
             pagesize : int, The number of results to get per page for queries
              display : (less|stdout), The reader used to view query results
               format : (smart|column|expanded|json|rich), Display format for query results
    allow_select_scan : bool, If True, SELECT statements can perform table scans
";

pub const HELP: &str = r"
    Show the command list, or the details for one topic

    HELP;
    HELP <topic>;

    The topic name is case-insensitive. HELP SELECT and HELP SCAN print the same page.
";

pub const CLEAR: &str = r"
    Scroll the visible screen into the scrollback so the input line is at the top.
    Previous output stays available with the scroll wheel.

    clear
    Aliases: cls, c
";

pub const EXIT: &str = r"
    Exit the DQL prompt

    exit
    Aliases: quit
";

pub const SHELL: &str = r"
    Run a shell command

    shell <command> [args...]
";

pub const VERSION: &str = r"
    Print the DQL version

    version
";

pub const WHOAMI: &str = r"
    Print the current AWS identity

    whoami
    Aliases: iam
";

pub const USE: &str = r"
    Connect to an AWS region

    use <region>
";

pub const LOCAL: &str = r"
    Connect to a local DynamoDB endpoint

    local [host] [port]
    local host=<host> port=<port>
";

pub const FILE: &str = r"
    Execute DQL statements from a file

    file <path>
";

pub const LS: &str = r"
    List tables or describe one table

    ls
    ls <glob>
    ls <glob> refresh=True
    ls refresh=True
    ls refresh=True async=True
    ls <glob> metrics=True

    <glob> uses * and ? the same way as a filename glob.
    One match prints that table's description, the same text as DESCRIBE.
    Several matches print a list.
    When nothing matches exactly, ls lists tables with similar names and tables
    whose keys, indexes, or attributes are similar to the text.
    A glob that is still not similar prints an empty list.
    A two-letter pattern matches only names that start with it.
    The result says whether it is showing similar names, related keys, or both.
    The table list is saved to disk the first time it is fetched. A table
    definition is saved when that table is fetched. Later ls commands reuse
    those snapshots. A line starting with cache: says where the list came from.
    refresh=True reloads the cached list and the definitions already saved.
    It does not describe tables that have never been fetched.
    async=True with refresh=True returns the snapshot immediately and reloads
    those cached entries in the background. That reload runs until the process
    exits, so it finishes in the REPL, the web UI, and `dqlrs --serve`.
    True, true, and yes are accepted.

    Examples
    --------
    ls
    ls foo-*
    ls *foo* refresh=True
";

pub const DESCRIBE: &str = r"
    Print one table's description

    DESCRIBE <table>;

    The text matches `ls <table>` when that name matches one table.
    A glob that matches one table prints this description too.
    Several matches print the list instead.

    Examples
    --------
    CREATE TABLE posts (id STRING HASH KEY); DESCRIBE posts;
    ls post*
";

pub const SHOW: &str = r"
    List table names

    SHOW TABLES;
    SHOW TABLES LIKE 'pattern';

    SHOW TABLES prints the same list as `ls`: name, items, read, write, status,
    and size. LIKE is case-sensitive. % matches any sequence. _ matches one
    character.
    When that matches nothing, the result lists tables with similar names and
    tables whose keys, indexes, or attributes are similar to the text.
    A two-letter pattern matches only names that start with it.
    The result says whether it is showing similar names, related keys, or both.
    A pattern that is still not similar stays an empty list.

    Examples
    --------
    SHOW TABLES;
    SHOW TABLES LIKE 'alpha%';
    SHOW TABLES LIKE 'foo_bar';
    SHOW TABLES LIKE 'post';
";

pub const HISTORY: &str = r#"
    Inspect and manage command history

    history
        List history entries
    history search <string>
        Search history (case-insensitive)
    history dedupe
        Keep the most recent of each unique command; drop skippable entries
    history remove --exact "string"
        Remove entries that exactly match the string
    history edit <editor-name>
        Open the history file in an editor, then exit dql
"#;

pub const THROTTLE: &str = r"
    Show or set capacity throttle limits

    throttle
    throttle <read> <write>
    throttle default <read> <write>
    throttle <table> <read> <write>
    throttle <table> <index> <read> <write>
";

pub const UNTHROTTLE: &str = r"
    Clear capacity throttle limits

    unthrottle
    unthrottle default
    unthrottle <table>
    unthrottle <table> <index>
";

pub const WATCH: &str = r"
    Monitor CloudWatch metrics for tables (requires --features watch)

    watch <table> [table...]
";
