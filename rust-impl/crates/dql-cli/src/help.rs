use crossterm::style::{Color as CtColor, Stylize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::io::Write;

struct HelpCommand {
    name: &'static str,
    summary: &'static str,
}

struct HelpGroup {
    title: &'static str,
    blurb: &'static str,
    commands: &'static [HelpCommand],
}

const GROUPS: &[HelpGroup] = &[
    HelpGroup {
        title: "Queries",
        blurb: "DQL statements for reading and writing DynamoDB data",
        commands: &[
            HelpCommand {
                name: "alter",
                summary: "Change table throughput or global indexes",
            },
            HelpCommand {
                name: "analyze",
                summary: "Run a query and show consumed capacity",
            },
            HelpCommand {
                name: "create",
                summary: "Create a table",
            },
            HelpCommand {
                name: "delete",
                summary: "Delete items from a table",
            },
            HelpCommand {
                name: "drop",
                summary: "Delete a table",
            },
            HelpCommand {
                name: "dump",
                summary: "Print CREATE statements for table schemas",
            },
            HelpCommand {
                name: "explain",
                summary: "Show the DynamoDB API calls a query would make",
            },
            HelpCommand {
                name: "insert",
                summary: "Insert items into a table",
            },
            HelpCommand {
                name: "load",
                summary: "Load items from a saved file into a table",
            },
            HelpCommand {
                name: "scan",
                summary: "Scan a table (same syntax family as select)",
            },
            HelpCommand {
                name: "select",
                summary: "Query items from a table or index",
            },
            HelpCommand {
                name: "update",
                summary: "Update items in a table",
            },
        ],
    },
    HelpGroup {
        title: "Connection",
        blurb: "Choose where DQL connects and inspect identity",
        commands: &[
            HelpCommand {
                name: "use",
                summary: "Connect to an AWS region",
            },
            HelpCommand {
                name: "local",
                summary: "Connect to a local DynamoDB endpoint",
            },
            HelpCommand {
                name: "whoami",
                summary: "Show the current AWS identity",
            },
        ],
    },
    HelpGroup {
        title: "Tables & files",
        blurb: "Inspect tables and run scripts from disk",
        commands: &[
            HelpCommand {
                name: "ls",
                summary: "List tables or describe one table",
            },
            HelpCommand {
                name: "file",
                summary: "Execute DQL statements from a file",
            },
            HelpCommand {
                name: "watch",
                summary: "Monitor CloudWatch metrics for tables",
            },
        ],
    },
    HelpGroup {
        title: "Configuration",
        blurb: "Tune display options and capacity throttles",
        commands: &[
            HelpCommand {
                name: "opt",
                summary: "Get or set session options",
            },
            HelpCommand {
                name: "throttle",
                summary: "Show or set capacity throttle limits",
            },
            HelpCommand {
                name: "unthrottle",
                summary: "Clear capacity throttle limits",
            },
        ],
    },
    HelpGroup {
        title: "History",
        blurb: "Inspect and clean the command history file",
        commands: &[HelpCommand {
            name: "history",
            summary: "List, search, dedupe, remove, or edit history",
        }],
    },
    HelpGroup {
        title: "Session",
        blurb: "REPL utilities and process control",
        commands: &[
            HelpCommand {
                name: "help",
                summary: "List commands or show details for one",
            },
            HelpCommand {
                name: "clear",
                summary: "Clear the screen",
            },
            HelpCommand {
                name: "shell",
                summary: "Run a shell command",
            },
            HelpCommand {
                name: "version",
                summary: "Print the DQL version",
            },
            HelpCommand {
                name: "exit",
                summary: "Leave the DQL prompt",
            },
        ],
    },
];

/// Render top-level help or a specific topic as styled ratatui lines.
pub fn render_lines(args: &[String], width: usize) -> Result<Vec<Line<'static>>, String> {
    let _ = width;
    if let Some(topic) = args.first() {
        topic_lines(topic).ok_or_else(|| format!("No help available for {topic}"))
    } else {
        Ok(overview_lines())
    }
}

/// Write top-level help or a specific topic to a plain writer (ANSI when possible).
pub fn write_help(args: &[String], out: &mut dyn Write, width: usize) -> Result<(), String> {
    let _ = width;
    if let Some(topic) = args.first() {
        let text = topic_help(topic).ok_or_else(|| format!("No help available for {topic}"))?;
        write!(out, "{text}").map_err(|err| err.to_string())
    } else {
        write_overview(out).map_err(|err| err.to_string())
    }
}

pub fn overview_lines() -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            "Available commands",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for group in GROUPS {
        lines.extend(group_lines(group));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Type 'help <command>' for details on a query or meta-command.",
        Style::default().fg(Color::DarkGray),
    )));
    lines
}

fn group_lines(group: &HelpGroup) -> Vec<Line<'static>> {
    let title = format!("{}:", group.title);
    let rule = "-".repeat(title.len());
    let mut lines = vec![
        Line::from(Span::styled(title, group_style())),
        Line::from(Span::styled(rule, Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(group.blurb.to_string(), Style::default().fg(Color::Gray)),
        ]),
    ];
    for command in group.commands {
        lines.push(Line::from(vec![
            Span::raw("  - "),
            Span::styled(
                command.name.to_string(),
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(":  ", Style::default().fg(Color::DarkGray)),
            Span::raw(command.summary.to_string()),
        ]));
    }
    lines
}

pub fn topic_lines(topic: &str) -> Option<Vec<Line<'static>>> {
    let text = topic_help(topic)?;
    let mut lines = Vec::new();
    let canonical = canonical_topic(topic);
    lines.push(Line::from(Span::styled(
        format!("help {canonical}"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    for raw in text.lines() {
        if raw.trim().is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        lines.push(Line::from(raw.to_string()));
    }
    Some(lines)
}

pub fn topic_help(topic: &str) -> Option<&'static str> {
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
        "opt" | "options" => Some(OPTIONS),
        "help" => Some(HELP),
        "clear" | "cls" | "c" => Some(CLEAR),
        "exit" | "quit" => Some(EXIT),
        "shell" => Some(SHELL),
        "version" => Some(VERSION),
        "whoami" | "iam" => Some(WHOAMI),
        "use" => Some(USE),
        "local" => Some(LOCAL),
        "file" => Some(FILE),
        "ls" => Some(LS),
        "history" => Some(HISTORY),
        "throttle" => Some(THROTTLE),
        "unthrottle" => Some(UNTHROTTLE),
        "watch" => Some(WATCH),
        _ => None,
    }
}

/// Back-compat alias used by older call sites / tests.
pub fn statement_help(topic: &str) -> Option<&'static str> {
    topic_help(topic)
}

fn canonical_topic(topic: &str) -> String {
    match topic.to_ascii_lowercase().as_str() {
        "options" => "opt".to_string(),
        "cls" | "c" => "clear".to_string(),
        "quit" => "exit".to_string(),
        "iam" => "whoami".to_string(),
        other => other.to_string(),
    }
}

fn write_overview(out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(out, "{}", "Available commands".bold())?;
    writeln!(out)?;
    for group in GROUPS {
        write_group(out, group)?;
        writeln!(out)?;
    }
    writeln!(
        out,
        "{}",
        "Type 'help <command>' for details on a query or meta-command.".with(CtColor::DarkGrey)
    )?;
    Ok(())
}

fn write_group(out: &mut dyn Write, group: &HelpGroup) -> std::io::Result<()> {
    let title = format!("{}:", group.title);
    let rule = "-".repeat(title.len());
    writeln!(out, "{}", title.with(CtColor::Cyan).bold())?;
    writeln!(out, "{}", rule.with(CtColor::DarkGrey))?;
    writeln!(out, "  {}", group.blurb.with(CtColor::Grey))?;
    for command in group.commands {
        write!(out, "  - ")?;
        write!(out, "{}", command.name.with(CtColor::Magenta))?;
        write!(out, "{}", ":  ".with(CtColor::DarkGrey))?;
        writeln!(out, "{}", command.summary)?;
    }
    Ok(())
}

fn group_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
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
               format : (smart|column|expanded|json|rich), Display format for query results
    allow_select_scan : bool, If True, SELECT statements can perform table scans
";

pub const HELP: &str = r"
    List commands or print details about a command

    help
    help <command>
";

pub const CLEAR: &str = r"
    Clear the screen

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
    ls <tablename>
    ls metrics=true
    ls <tablename> refresh=true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_docs() {
        for topic in [
            "alter", "analyze", "create", "delete", "drop", "dump", "explain", "insert", "load",
            "scan", "select", "update", "options", "opt", "history", "clear", "exit", "ls", "help",
        ] {
            assert!(topic_help(topic).is_some(), "missing help for {topic}");
        }
        assert!(OPTIONS.contains("rich"));
    }

    #[test]
    fn overview_lists_queries_and_meta() {
        let text: String = overview_lines()
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Queries:"));
        assert!(text.contains("-------"));
        assert!(text.contains("DQL statements for reading and writing DynamoDB data"));
        assert!(text.contains("- select:  Query items from a table or index"));
        assert!(text.contains("- history:  List, search, dedupe, remove, or edit history"));
        assert!(text.contains("Session:"));
        assert!(text.contains("help <command>"));
    }

    #[test]
    fn render_lines_topic_and_overview() {
        let overview = render_lines(&[], 80).unwrap();
        assert!(!overview.is_empty());
        let select = render_lines(&[String::from("select")], 80).unwrap();
        assert!(select
            .iter()
            .any(|line| line.spans.iter().any(|s| s.content.contains("help select"))));
        assert!(render_lines(&[String::from("nope")], 80).is_err());
    }
}
