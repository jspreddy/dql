//! `HELP` and `HELP <topic>` pages. Each interface paints these blocks itself.

#[derive(Debug, Clone, PartialEq)]
pub struct HelpPage {
    pub topic: String,
    pub title: String,
    pub blocks: Vec<HelpBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HelpBlock {
    Prose(String),
    Heading(String),
    Code(String),
    Entry { name: String, summary: String },
}

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
                name: "describe",
                summary: "Print one table's description",
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
                name: "ls",
                summary: "List tables or describe one table",
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
                name: "show",
                summary: "List table names, optionally with a LIKE pattern",
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
        blurb: "Run a script from disk and watch table metrics",
        commands: &[
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
                summary: "Show this list, or the details for one topic",
            },
            HelpCommand {
                name: "clear",
                summary: "Scroll the screen so the input is at the top",
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

const CODE_START: &[&str] = &[
    "select",
    "scan",
    "insert",
    "update",
    "create",
    "alter",
    "delete",
    "drop",
    "dump",
    "load",
    "explain",
    "analyze",
    "show",
    "describe",
    "help",
    "from",
    "where",
    "into",
    "values",
    "set",
    "add",
    "using",
    "limit",
    "order",
    "keys",
    "global",
    "table",
    "index",
    "consistent",
    "save",
    "returns",
    "ls",
    "opt",
    "file",
    "watch",
    "use",
    "local",
    "history",
    "throttle",
    "unthrottle",
    "clear",
    "exit",
    "shell",
    "whoami",
    "version",
    "quit",
    "cls",
];

const SENTENCE_WORDS: &[&str] = &[
    " the ", " a ", " an ", " to ", " of ", " for ", " and ", " with ", " when ", " that ",
    " this ", " are ", " is ", " can ", " does ", " has ",
];

pub fn topic_help(topic: &str) -> Option<&'static str> {
    match topic.to_ascii_lowercase().as_str() {
        "alter" => Some(ALTER),
        "analyze" => Some(ANALYZE),
        "create" => Some(CREATE),
        "delete" => Some(DELETE),
        "describe" => Some(DESCRIBE),
        "drop" => Some(DROP),
        "dump" => Some(DUMP),
        "explain" => Some(EXPLAIN),
        "insert" => Some(INSERT),
        "load" => Some(LOAD),
        "scan" => Some(SCAN),
        "select" => Some(SELECT),
        "show" => Some(SHOW),
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

pub fn canonical_topic(topic: &str) -> String {
    match topic.to_ascii_lowercase().as_str() {
        "options" => "opt".to_string(),
        "cls" | "c" => "clear".to_string(),
        "quit" => "exit".to_string(),
        "iam" => "whoami".to_string(),
        other => other.to_string(),
    }
}

/// `None` is the command index. An unknown topic is an error.
pub fn help_page(topic: Option<&str>) -> Result<HelpPage, String> {
    let Some(topic) = topic.map(str::trim).filter(|topic| !topic.is_empty()) else {
        return Ok(index_page());
    };
    let canonical = canonical_topic(topic);
    let text = topic_help(&canonical).ok_or_else(|| format!("No help available for {topic}"))?;
    Ok(HelpPage {
        topic: canonical.clone(),
        title: format!("HELP {canonical}"),
        blocks: blocks_from_body(text),
    })
}

fn index_page() -> HelpPage {
    let mut blocks = Vec::new();
    for group in GROUPS {
        blocks.push(HelpBlock::Heading(group.title.to_string()));
        blocks.push(HelpBlock::Prose(group.blurb.to_string()));
        for command in group.commands {
            blocks.push(HelpBlock::Entry {
                name: command.name.to_string(),
                summary: command.summary.to_string(),
            });
        }
    }
    blocks.push(HelpBlock::Prose(
        "Run HELP <topic>; for details on a query or meta-command.".to_string(),
    ));
    HelpPage {
        topic: String::new(),
        title: "Available commands".to_string(),
        blocks,
    }
}

fn blocks_from_body(text: &str) -> Vec<HelpBlock> {
    let mut blocks = Vec::new();
    let mut bucket: Vec<String> = Vec::new();
    let mut bucket_code = false;
    let mut examples = false;

    let flush = |bucket: &mut Vec<String>, code: bool, blocks: &mut Vec<HelpBlock>| {
        if bucket.is_empty() {
            return;
        }
        let text = if code {
            dedent(bucket)
        } else {
            bucket
                .iter()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        };
        bucket.clear();
        if text.is_empty() {
            return;
        }
        blocks.push(if code {
            HelpBlock::Code(text)
        } else {
            HelpBlock::Prose(text)
        });
    };

    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            flush(&mut bucket, bucket_code, &mut blocks);
            continue;
        }
        if trimmed.chars().all(|ch| ch == '-') {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("examples") {
            flush(&mut bucket, bucket_code, &mut blocks);
            blocks.push(HelpBlock::Heading("Examples".to_string()));
            examples = true;
            continue;
        }
        let code = examples || line_is_code(trimmed);
        if !bucket.is_empty() && code != bucket_code {
            flush(&mut bucket, bucket_code, &mut blocks);
        }
        bucket_code = code;
        bucket.push(raw.trim_end().to_string());
    }
    flush(&mut bucket, bucket_code, &mut blocks);
    blocks
}

fn line_is_code(line: &str) -> bool {
    if line.ends_with('.') {
        return false;
    }
    if line.contains(';') {
        return true;
    }
    let lower = format!(" {} ", line.to_ascii_lowercase());
    if SENTENCE_WORDS.iter().any(|word| lower.contains(word)) {
        return false;
    }
    if line.starts_with('[') || line.starts_with('<') || line.starts_with('(') {
        return true;
    }
    let head = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
    if CODE_START
        .iter()
        .any(|word| head.eq_ignore_ascii_case(word))
    {
        return true;
    }
    line.split_whitespace().count() == 1
        && line
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn dedent(lines: &[String]) -> String {
    let min = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if line.len() >= min {
                line[min..].trim_end().to_string()
            } else {
                line.trim().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl HelpPage {
    pub fn plain(&self) -> String {
        let mut lines = vec![self.title.clone(), String::new()];
        for block in &self.blocks {
            match block {
                HelpBlock::Prose(text) | HelpBlock::Code(text) => {
                    lines.push(text.clone());
                    lines.push(String::new());
                }
                HelpBlock::Heading(text) => {
                    lines.push(text.clone());
                    lines.push(String::new());
                }
                HelpBlock::Entry { name, summary } => {
                    lines.push(format!("  {name}  {summary}"));
                }
            }
        }
        lines.join("\n").trim_end().to_string()
    }

    pub fn to_value(&self) -> serde_json::Value {
        let blocks = self
            .blocks
            .iter()
            .map(|block| match block {
                HelpBlock::Prose(text) => serde_json::json!({"kind": "prose", "text": text}),
                HelpBlock::Heading(text) => serde_json::json!({"kind": "heading", "text": text}),
                HelpBlock::Code(text) => serde_json::json!({"kind": "code", "text": text}),
                HelpBlock::Entry { name, summary } => {
                    serde_json::json!({"kind": "entry", "name": name, "summary": summary})
                }
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "topic": self.topic,
            "title": self.title,
            "blocks": blocks,
        })
    }

    /// Terminal coloring for `dqlrs -c` and the REPL's plain-output path.
    pub fn ansi(&self) -> String {
        let mut out = String::new();
        out.push_str(&paint(&self.title, "1;36"));
        out.push_str("\n\n");
        for block in &self.blocks {
            match block {
                HelpBlock::Heading(text) => {
                    out.push_str(&paint(text, "1"));
                    out.push('\n');
                }
                HelpBlock::Prose(text) => {
                    out.push_str(text);
                    out.push_str("\n\n");
                }
                HelpBlock::Code(text) => {
                    for line in text.lines() {
                        out.push_str(&highlight_ansi(line));
                        out.push('\n');
                    }
                    out.push('\n');
                }
                HelpBlock::Entry { name, summary } => {
                    out.push_str("  ");
                    out.push_str(&paint(name, "1;35"));
                    out.push_str("  ");
                    out.push_str(summary);
                    out.push('\n');
                }
            }
        }
        out.trim_end().to_string()
    }
}

fn paint(text: &str, code: &str) -> String {
    format!("\u{1b}[{code}m{text}\u{1b}[0m")
}

fn highlight_ansi(line: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '-' && chars.get(i + 1) == Some(&'-') {
            let rest: String = chars[i..].iter().collect();
            out.push_str(&paint(&rest, "90"));
            break;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            out.push_str(&paint(&lit, "32"));
            continue;
        }
        if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num: String = chars[start..i].iter().collect();
            out.push_str(&paint(&num, "33"));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let style = if word.eq_ignore_ascii_case("drop") || word.eq_ignore_ascii_case("delete")
            {
                "1;31"
            } else if is_highlight_keyword(&word) {
                "1;35"
            } else if is_type_name(&word) {
                "34"
            } else {
                ""
            };
            if style.is_empty() {
                out.push_str(&word);
            } else {
                out.push_str(&paint(&word, style));
            }
            continue;
        }
        if matches!(
            ch,
            '=' | '<'
                | '>'
                | '!'
                | ','
                | '('
                | ')'
                | '*'
                | '+'
                | '-'
                | '/'
                | '%'
                | ';'
                | '.'
                | '['
                | ']'
                | '|'
        ) {
            out.push_str(&paint(&ch.to_string(), "36"));
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn is_type_name(word: &str) -> bool {
    matches!(
        word.to_ascii_uppercase().as_str(),
        "STRING" | "NUMBER" | "BINARY" | "BOOL" | "BOOLEAN"
    )
}

fn is_highlight_keyword(word: &str) -> bool {
    const WORDS: &[&str] = &[
        "SELECT",
        "FROM",
        "WHERE",
        "AND",
        "OR",
        "NOT",
        "INSERT",
        "INTO",
        "VALUES",
        "UPDATE",
        "SET",
        "CREATE",
        "TABLE",
        "ALTER",
        "SCAN",
        "INDEX",
        "HASH",
        "RANGE",
        "KEY",
        "KEYS",
        "THROUGHPUT",
        "GLOBAL",
        "LOCAL",
        "INCLUDE",
        "ALL",
        "IF",
        "EXISTS",
        "LIMIT",
        "ORDER",
        "BY",
        "ASC",
        "DESC",
        "BETWEEN",
        "IN",
        "USING",
        "CONSISTENT",
        "RETURNS",
        "DUMP",
        "LOAD",
        "EXPLAIN",
        "ANALYZE",
        "SAVE",
        "SHOW",
        "LIKE",
        "DESCRIBE",
        "HELP",
        "SCHEMA",
        "PARENTS",
        "TRUE",
        "FALSE",
        "NULL",
    ];
    WORDS.iter().any(|kw| kw.eq_ignore_ascii_case(word))
}

include!("help_body.rs");

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(page: &HelpPage, code: bool) -> String {
        page.blocks
            .iter()
            .filter_map(|block| match block {
                HelpBlock::Code(text) if code => Some(text.as_str()),
                HelpBlock::Prose(text) if !code => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn select_page_splits_syntax_and_examples_from_prose() {
        let page = help_page(Some("SELECT")).unwrap();
        assert_eq!(page.topic, "select");
        let code = joined(&page, true);
        let prose = joined(&page, false);
        assert!(code.contains("FROM tablename"), "{code}");
        assert!(code.contains("CREATE TABLE foobars"), "{code}");
        assert!(prose.contains("Select items"), "{prose}");
        assert!(!code.contains("Select items"), "{code}");
        assert!(page.ansi().contains("\u{1b}[1;35mSELECT"));
    }

    #[test]
    fn scan_uses_the_select_page() {
        let scan = help_page(Some("scan")).unwrap();
        let select = help_page(Some("select")).unwrap();
        assert_eq!(scan.blocks, select.blocks);
        assert_eq!(scan.topic, "scan");
    }

    #[test]
    fn describe_and_show_refresh_notes_stay_prose() {
        let describe = help_page(Some("describe")).unwrap();
        let describe_prose = joined(&describe, false);
        let describe_code = joined(&describe, true);
        assert!(
            describe_prose.contains("DESCRIBE always loads that table"),
            "{describe_prose}"
        );
        assert!(!describe_code.contains("always loads"), "{describe_code}");

        let show = help_page(Some("show")).unwrap();
        let show_prose = joined(&show, false);
        let show_code = joined(&show, true);
        assert!(
            show_prose.contains("always reloads the table list"),
            "{show_prose}"
        );
        assert!(show_prose.contains("never been fetched"), "{show_prose}");
        assert!(!show_code.contains("always reloads"), "{show_code}");
    }

    #[test]
    fn ls_examples_stay_code_and_notes_stay_prose() {
        let page = help_page(Some("ls")).unwrap();
        let code = joined(&page, true);
        let prose = joined(&page, false);
        assert!(code.contains("ls foo-*"), "{code}");
        assert!(prose.contains("filename glob"), "{prose}");
        assert!(!code.contains("filename glob"), "{code}");
    }

    #[test]
    fn history_descriptions_stay_prose() {
        let page = help_page(Some("history")).unwrap();
        let code = joined(&page, true);
        let prose = joined(&page, false);
        assert!(code.contains("history search <string>"), "{code}");
        assert!(prose.contains("List history entries"), "{prose}");
    }

    #[test]
    fn index_lists_topics() {
        let page = help_page(None).unwrap();
        assert_eq!(page.title, "Available commands");
        assert!(page.blocks.iter().any(|block| matches!(
            block,
            HelpBlock::Entry { name, .. } if name == "select"
        )));
        assert!(help_page(Some("nope")).is_err());
    }

    #[test]
    fn option_definitions_stay_prose() {
        let page = help_page(Some("options")).unwrap();
        let prose = joined(&page, false);
        let code = joined(&page, true);
        assert!(prose.contains("allow_select_scan"), "{prose}");
        assert!(!code.contains("allow_select_scan"), "{code}");
        assert_eq!(page.topic, "opt");
    }
}
