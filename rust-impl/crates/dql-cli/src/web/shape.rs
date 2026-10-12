//! Statement splitting, table-shape parsing, and result column order.
//!
//! Column roles come from `ls` text and `EXPLAIN`, the same information the
//! browser uses to tint hash and range keys.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

pub const PAGE_SIZE: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableShape {
    pub hash_key: String,
    pub range_key: Option<String>,
    pub indexes: HashMap<String, (String, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Column {
    pub name: String,
    pub table: Option<String>,
    pub index: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExecText {
    pub ok: bool,
    pub message: String,
    pub error: String,
}

impl ExecText {
    pub fn from_envelope(envelope: &crate::serve::protocol::ServeEnvelope) -> Self {
        Self {
            ok: envelope.ok,
            message: envelope.message.clone().unwrap_or_default(),
            error: envelope
                .error
                .as_ref()
                .map(|err| err.message.clone())
                .unwrap_or_default(),
        }
    }

    pub(crate) fn failure(&self) -> String {
        if !self.error.is_empty() {
            self.error.clone()
        } else if !self.message.is_empty() {
            self.message.clone()
        } else {
            "dqlrs request failed".to_string()
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct KeyRole {
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ListedTable {
    pub name: String,
    pub keys: Vec<KeyRole>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TableList {
    pub tables: Vec<ListedTable>,
    pub note: String,
}

pub fn split_statements(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied().unwrap_or('\0');
        if quote.is_none() && ch == '-' && next == '-' {
            buf.push(ch);
            buf.push(next);
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                buf.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if let Some(open) = quote {
            buf.push(ch);
            if ch == open && chars.get(i.wrapping_sub(1)).copied() != Some('\\') {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            buf.push(ch);
            i += 1;
            continue;
        }
        if ch == ';' {
            let statement = buf.trim().to_string();
            buf.clear();
            if has_code(&statement) {
                parts.push(format!("{statement};"));
            }
            i += 1;
            continue;
        }
        buf.push(ch);
        i += 1;
    }
    let tail = buf.trim();
    if has_code(tail) {
        if tail.ends_with(';') {
            parts.push(tail.to_string());
        } else {
            parts.push(format!("{tail};"));
        }
    }
    parts
}

pub fn list_tables(
    pattern: &str,
    refresh: bool,
    shapes: &mut HashMap<String, TableShape>,
    catalog: &mut Option<Vec<ListedTable>>,
    mut exec: impl FnMut(&str) -> ExecText,
) -> Result<TableList, String> {
    if refresh {
        shapes.clear();
        *catalog = None;
    }
    if catalog.is_none() {
        *catalog = Some(load_table_catalog(refresh, &mut exec)?);
    }
    Ok(filter_table_list(catalog.as_deref().unwrap_or(&[]), pattern))
}

pub fn parse_table_shape(text: &str) -> Option<TableShape> {
    let mut hash_key = String::new();
    let mut range_key = None;
    let mut indexes = HashMap::new();
    let mut section = "";
    for raw in text.lines() {
        let stripped = raw.trim();
        if let Some(rest) = stripped.strip_prefix("Hash Key:") {
            hash_key = key_name(rest);
            section = "";
            continue;
        }
        if let Some(rest) = stripped.strip_prefix("Range Key:") {
            range_key = Some(key_name(rest)).filter(|name| !name.is_empty());
            section = "";
            continue;
        }
        if stripped == "Local Indexes:" {
            section = "lsi";
            continue;
        }
        if stripped == "Global Indexes:" {
            section = "gsi";
            continue;
        }
        if stripped.starts_with("CREATE TABLE") || stripped.starts_with("Name:") {
            section = "";
        }
        if section == "lsi" {
            if let Some(caps) = LSI_LINE.captures(raw) {
                indexes.insert(
                    caps["name"].to_string(),
                    (caps["hash"].to_string(), blank_key(&caps["range"])),
                );
            }
        } else if section == "gsi" && !stripped.starts_with("Name ") {
            if let Some(caps) = GSI_LINE.captures(raw) {
                indexes.insert(
                    caps["name"].to_string(),
                    (caps["hash"].to_string(), blank_key(&caps["range"])),
                );
            }
        }
    }
    if hash_key.is_empty() {
        return None;
    }
    Some(TableShape {
        hash_key,
        range_key,
        indexes,
    })
}

pub fn read_query(statement: &str) -> (Option<String>, Option<Vec<String>>) {
    let stripped = strip_comments(statement);
    let Some(caps) = READ_QUERY.captures(&stripped) else {
        return (None, None);
    };
    let selected = CONSISTENT
        .replace(caps["select"].trim(), "")
        .trim()
        .to_string();
    let table = caps["table"].to_string();
    if selected == "*" || COUNT_STAR.is_match(&selected) {
        return (Some(table), None);
    }
    let names = split_csv(&selected)
        .into_iter()
        .filter_map(|part| {
            let piece = part.trim();
            if piece.is_empty() {
                return None;
            }
            let name = AS_ALIAS
                .captures(piece)
                .map(|caps| caps[1].to_string())
                .unwrap_or_else(|| piece.to_string());
            Some(norm_ws(&name))
        })
        .collect();
    (Some(table), Some(names))
}

pub fn explain_index(message: &str) -> Option<String> {
    for caps in EXPLAIN_INDEX.captures_iter(message) {
        let name = &caps[1];
        if name != "TABLE" && name != "-" {
            return Some(name.to_string());
        }
    }
    None
}

pub fn column_order(
    rows: &[Value],
    shape: Option<&TableShape>,
    index: Option<&str>,
    selection: Option<&[String]>,
) -> Vec<Column> {
    let mut present = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        for key in object.keys() {
            if seen.insert(key.clone()) {
                present.push(key.clone());
            }
        }
    }
    let mut ordered = Vec::new();
    let mut roles: HashMap<String, Column> = HashMap::new();
    let take = |name: &str, ordered: &mut Vec<String>| {
        if seen.contains(name) && !ordered.iter().any(|item| item == name) {
            ordered.push(name.to_string());
        }
    };
    let mark = |name: &str, kind: &str, role: &str, roles: &mut HashMap<String, Column>| {
        if !seen.contains(name) {
            return;
        }
        let entry = roles.entry(name.to_string()).or_insert_with(|| Column {
            name: name.to_string(),
            table: None,
            index: None,
        });
        match kind {
            "table" => entry.table = Some(role.to_string()),
            "index" => entry.index = Some(role.to_string()),
            _ => {}
        }
    };
    if let Some(shape) = shape {
        mark(&shape.hash_key, "table", "hash", &mut roles);
        take(&shape.hash_key, &mut ordered);
        if let Some(range_key) = &shape.range_key {
            mark(range_key, "table", "range", &mut roles);
            take(range_key, &mut ordered);
        }
        if let Some(index) = index.and_then(|name| shape.indexes.get(name)) {
            mark(&index.0, "index", "hash", &mut roles);
            take(&index.0, &mut ordered);
            if let Some(range_key) = &index.1 {
                mark(range_key, "index", "range", &mut roles);
                take(range_key, &mut ordered);
            }
        }
    }
    if let Some(selection) = selection {
        for name in selection {
            take(name, &mut ordered);
        }
    }
    let mut rest: Vec<String> = present
        .into_iter()
        .filter(|name| !ordered.contains(name))
        .collect();
    rest.sort_by(|left, right| {
        left.to_lowercase()
            .cmp(&right.to_lowercase())
            .then_with(|| left.cmp(right))
    });
    ordered.extend(rest);
    ordered
        .into_iter()
        .map(|name| {
            roles.remove(&name).unwrap_or(Column {
                name,
                table: None,
                index: None,
            })
        })
        .collect()
}

pub fn changes_schema(statement: &str) -> bool {
    SCHEMA_CHANGE.is_match(strip_comments(statement).trim())
}

pub fn safe_table_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

pub fn shell_quote(text: &str) -> String {
    const SAFE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@%+=:,./-_";
    if !text.is_empty() && text.chars().all(|ch| SAFE.contains(ch)) {
        return text.to_string();
    }
    format!("'{}'", text.replace('\'', r#"'"'"'"#))
}

fn load_table_catalog(
    refresh: bool,
    exec: &mut impl FnMut(&str) -> ExecText,
) -> Result<Vec<ListedTable>, String> {
    let command = if refresh {
        "ls refresh=True"
    } else {
        "ls"
    };
    let summary = exec(command);
    if !summary.ok {
        let message = summary.failure();
        if message.to_lowercase().contains("not found") {
            return Ok(Vec::new());
        }
        return Err(message);
    }
    let mut tables = Vec::new();
    for name in ls_names(&summary.message) {
        let detail = exec(&format!("ls {}", shell_quote(&name)));
        let keys = if detail.ok {
            ls_keys(&detail.message)
        } else {
            Vec::new()
        };
        tables.push(ListedTable { name, keys });
    }
    Ok(tables)
}

fn filter_table_list(catalog: &[ListedTable], pattern: &str) -> TableList {
    let needle = pattern.trim();
    if needle.is_empty() {
        return TableList {
            tables: catalog.to_vec(),
            note: String::new(),
        };
    }
    let exact: Vec<ListedTable> = catalog
        .iter()
        .filter(|table| glob_matches(needle, &table.name))
        .cloned()
        .collect();
    if !exact.is_empty() {
        return TableList {
            tables: exact,
            note: String::new(),
        };
    }
    let match_tables: Vec<dql_engine::MatchTable> = catalog
        .iter()
        .map(|table| dql_engine::MatchTable {
            name: table.name.clone(),
            fields: table.keys.iter().map(|key| key.name.clone()).collect(),
        })
        .collect();
    match dql_engine::intelligent_matches(&match_tables, needle) {
        Ok(hit) => TableList {
            tables: hit
                .names
                .into_iter()
                .filter_map(|name| catalog.iter().find(|table| table.name == name).cloned())
                .collect(),
            note: hit.note,
        },
        Err(_) => TableList {
            tables: Vec::new(),
            note: String::new(),
        },
    }
}

fn glob_matches(pattern: &str, name: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|compiled| compiled.matches(name))
        .unwrap_or(false)
}

#[cfg(test)]
fn ls_note(text: &str) -> String {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        return if INTELLIGENT_NOTE.is_match(line) {
            line.to_string()
        } else {
            String::new()
        };
    }
    String::new()
}

fn ls_names(text: &str) -> Vec<String> {
    let mut described = Vec::new();
    let mut summary = Vec::new();
    let mut in_summary = false;
    let mut past_header = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("No exact match for ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Name:") {
            let name = rest.split_whitespace().next().unwrap_or("");
            if !name.is_empty() {
                described.push(name.to_string());
            }
            continue;
        }
        if line == "Tables" {
            in_summary = true;
            continue;
        }
        if !in_summary {
            continue;
        }
        if !past_header {
            if line.starts_with("Name ") || line.starts_with("Name\t") {
                past_header = true;
            }
            continue;
        }
        if let Some(name) = line.split_whitespace().next() {
            if name != "Name" && name != "Tables" {
                summary.push(name.to_string());
            }
        }
    }
    if described.is_empty() {
        summary
    } else {
        described
    }
}

fn ls_keys(text: &str) -> Vec<KeyRole> {
    let mut hash_key = String::new();
    let mut range_key = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("Hash Key:") {
            hash_key = key_name(rest);
        } else if let Some(rest) = line.strip_prefix("Range Key:") {
            range_key = key_name(rest);
        }
    }
    let mut keys = Vec::new();
    if !hash_key.is_empty() {
        keys.push(KeyRole {
            name: hash_key,
            role: "hash".to_string(),
        });
    }
    if !range_key.is_empty() {
        keys.push(KeyRole {
            name: range_key,
            role: "range".to_string(),
        });
    }
    keys
}

fn key_name(rest: &str) -> String {
    rest.trim()
        .split(" (")
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn blank_key(name: &str) -> Option<String> {
    if name.is_empty() || name == "-" {
        None
    } else {
        Some(name.to_string())
    }
}

fn has_code(statement: &str) -> bool {
    statement.lines().any(|line| {
        let stripped = line.trim();
        !stripped.is_empty() && !stripped.starts_with("--")
    })
}

fn strip_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied().unwrap_or('\0');
        if quote.is_none() && ch == '-' && next == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if let Some(open) = quote {
            out.push(ch);
            if ch == open && chars.get(i.wrapping_sub(1)).copied() != Some('\\') {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn split_csv(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut depth: usize = 0;
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(open) = quote {
            buf.push(ch);
            if ch == '\\' && i + 1 < chars.len() {
                buf.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if ch == open {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            buf.push(ch);
        } else if ch == '(' {
            depth += 1;
            buf.push(ch);
        } else if ch == ')' {
            depth = depth.saturating_sub(1);
            buf.push(ch);
        } else if ch == ',' && depth == 0 {
            parts.push(std::mem::take(&mut buf));
        } else {
            buf.push(ch);
        }
        i += 1;
    }
    if !buf.is_empty() {
        parts.push(buf);
    }
    parts
}

fn norm_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
static INTELLIGENT_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^No exact match for "[^"]*", so showing (?:similar names and related keys|similar names|related keys)\.$"#,
    )
    .expect("intelligent note")
});
static LSI_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(?P<name>\S+)\s+hash=(?P<hash>\S+)\s+range=(?P<range>\S+)\s+projection=")
        .expect("lsi line")
});
static GSI_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s+(?P<name>\S+)\s+\S+\s+\S+\s+\S+\s+(?P<hash>\S+)(?:\s+\([^)]+\))?\s+(?P<range>\S+)(?:\s+\([^)]+\))?\s+\S+\s*$",
    )
    .expect("gsi line")
});
static READ_QUERY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)^\s*(?:select|scan)\s+(?P<select>.*?)\s+from\s+(?P<table>[A-Za-z_][A-Za-z0-9_.-]*)\b",
    )
    .expect("read query")
});
static SCHEMA_CHANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(create|drop|alter)\s+table\b").expect("schema change"));
static EXPLAIN_INDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"'index':\s+"([^"]+)""#).expect("explain index"));
static CONSISTENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^consistent\s+").expect("consistent"));
static COUNT_STAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^count\s*\(\s*\*\s*\)$").expect("count star"));
static AS_ALIAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+AS\s+(\S+)\s*$").expect("as alias"));

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_on_semicolons_and_keeps_quotes() {
        let text = "SELECT * FROM t WHERE name = 'a;b';\nSCAN * FROM t;";
        assert_eq!(
            split_statements(text),
            ["SELECT * FROM t WHERE name = 'a;b';", "SCAN * FROM t;"]
        );
        assert!(split_statements("  \n-- just a comment\n").is_empty());
        assert_eq!(
            split_statements("DESCRIBE posts;\nls posts"),
            ["DESCRIBE posts;", "ls posts;"]
        );
        assert_eq!(
            split_statements("SHOW TABLES LIKE 'alpha%';\nls alpha* refresh=True"),
            ["SHOW TABLES LIKE 'alpha%';", "ls alpha* refresh=True;"]
        );
    }

    #[test]
    fn names_keys_and_column_order() {
        let summary = "Tables\nName     Items\nnb_posts        2\nnb_users        1\n";
        assert_eq!(ls_names(summary), ["nb_posts", "nb_users"]);
        let noted = format!("No exact match for \"post\", so showing similar names.\n\n{summary}");
        assert_eq!(ls_names(&noted), ["nb_posts", "nb_users"]);
        assert_eq!(
            ls_note(&noted),
            "No exact match for \"post\", so showing similar names."
        );
        assert_eq!(
            ls_note("No exact match for \"order_id\", so showing related keys.\n"),
            "No exact match for \"order_id\", so showing related keys."
        );
        assert_eq!(ls_note(summary), "");
        assert_eq!(
            ls_names("Name: catalog\nHash Key: isbn (STRING)\n"),
            ["catalog"]
        );
        assert_eq!(
            ls_keys("Name: nb_posts\nHash Key: username (STRING)\nRange Key: postid (NUMBER)\n"),
            vec![
                KeyRole {
                    name: "username".to_string(),
                    role: "hash".to_string()
                },
                KeyRole {
                    name: "postid".to_string(),
                    role: "range".to_string()
                }
            ]
        );

        let detail = "\
Name: sample_cosmos
Hash Key: pk (STRING)
Range Key: sk (STRING)

Global Indexes:
  Name             Projection       Read    Write HashKey              RangeKey               Status
  by-kind          ALL               N/A      N/A kind (STRING)        name (STRING)          ACTIVE

CREATE TABLE sample_cosmos (pk STRING HASH KEY, sk STRING RANGE KEY);";
        let shape = parse_table_shape(detail).unwrap();
        assert_eq!(shape.hash_key, "pk");
        assert_eq!(shape.range_key.as_deref(), Some("sk"));
        assert_eq!(
            shape
                .indexes
                .get("by-kind")
                .map(|(hash, range)| (hash.as_str(), range.as_deref())),
            Some(("kind", Some("name")))
        );
        let local = "\
Hash Key: device_id (STRING)
Range Key: ts (NUMBER)

Local Indexes:
  kind-index  hash=device_id  range=kind  projection=ALL";
        let local_shape = parse_table_shape(local).unwrap();
        assert_eq!(
            local_shape
                .indexes
                .get("kind-index")
                .map(|(hash, range)| (hash.as_str(), range.as_deref())),
            Some(("device_id", Some("kind")))
        );

        let shape = TableShape {
            hash_key: "id".to_string(),
            range_key: Some("sk".to_string()),
            indexes: HashMap::from([(
                "by-n".to_string(),
                ("n".to_string(), Some("sk".to_string())),
            )]),
        };
        let rows = vec![
            json!({"zebra": "z", "id": "a", "n": 9, "sk": 2, "apple": "p"}),
            json!({"id": "a", "sk": 1, "n": 3, "apple": "q", "zebra": "y"}),
        ];
        let plain = column_order(&rows, Some(&shape), None, None);
        assert_eq!(
            plain
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "sk", "apple", "n", "zebra"]
        );
        assert_eq!(plain[0].table.as_deref(), Some("hash"));
        assert!(plain[0].index.is_none());
        assert_eq!(plain[1].table.as_deref(), Some("range"));
        assert!(plain[2].table.is_none());
        let indexed = column_order(&rows, Some(&shape), Some("by-n"), None);
        assert_eq!(
            indexed
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "sk", "n", "apple", "zebra"]
        );
        assert_eq!(indexed[1].table.as_deref(), Some("range"));
        assert_eq!(indexed[1].index.as_deref(), Some("range"));
        assert!(indexed[2].table.is_none());
        assert_eq!(indexed[2].index.as_deref(), Some("hash"));

        let swapped = TableShape {
            hash_key: "id".to_string(),
            range_key: Some("sk".to_string()),
            indexes: HashMap::from([(
                "by-apple".to_string(),
                ("apple".to_string(), Some("id".to_string())),
            )]),
        };
        let by_apple = column_order(&rows, Some(&swapped), Some("by-apple"), None);
        assert_eq!(
            by_apple
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "sk", "apple", "n", "zebra"]
        );
        assert_eq!(by_apple[0].table.as_deref(), Some("hash"));
        assert_eq!(by_apple[0].index.as_deref(), Some("range"));
        assert_eq!(by_apple[1].table.as_deref(), Some("range"));
        assert!(by_apple[1].index.is_none());
        assert!(by_apple[2].table.is_none());
        assert_eq!(by_apple[2].index.as_deref(), Some("hash"));

        let projected = vec![json!({"zebra": "z", "apple": "p", "id": "a"})];
        let selection = ["zebra".to_string(), "apple".to_string(), "id".to_string()];
        assert_eq!(
            column_order(&projected, Some(&shape), None, Some(&selection))
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "zebra", "apple"]
        );
        let indexed_rows = vec![json!({"apple": "p", "zebra": "z", "n": 9})];
        let indexed_selection = ["apple".to_string(), "zebra".to_string(), "n".to_string()];
        assert_eq!(
            column_order(
                &indexed_rows,
                Some(&shape),
                Some("by-n"),
                Some(&indexed_selection)
            )
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
            ["n", "apple", "zebra"]
        );
        assert_eq!(
            explain_index("query t {'index': \"by-n\", 'filter': \"n = :v1\"}").as_deref(),
            Some("by-n")
        );
        assert!(explain_index("query t {'index': \"TABLE\"}").is_none());
        assert!(explain_index("scan t {'filter': \"habitable = :v1\"}").is_none());
        assert_eq!(
            read_query("SELECT zebra, apple AS fruit FROM posts WHERE id = 'a';"),
            (
                Some("posts".to_string()),
                Some(vec!["zebra".to_string(), "fruit".to_string()])
            )
        );
        assert_eq!(
            read_query("-- note\nSELECT CONSISTENT * FROM posts WHERE id = 'a';"),
            (Some("posts".to_string()), None)
        );
        assert_eq!(
            read_query("SCAN name, ts(updated) FROM posts;"),
            (
                Some("posts".to_string()),
                Some(vec!["name".to_string(), "ts(updated)".to_string()])
            )
        );
        assert!(safe_table_name("nb_posts"));
        assert!(safe_table_name("nb-posts"));
        assert!(!safe_table_name("nb posts"));
        assert!(!safe_table_name(""));
    }

    #[test]
    fn search_uses_cached_catalog_until_refresh() {
        let mut shapes = HashMap::from([(
            "nb_posts".to_string(),
            TableShape {
                hash_key: "id".to_string(),
                range_key: None,
                indexes: HashMap::new(),
            },
        )]);
        let mut catalog = None;
        let mut commands = Vec::new();
        let listed = list_tables("nb_*", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(listed.note, "");
        assert_eq!(listed.tables.len(), 1);
        assert_eq!(listed.tables[0].name, "nb_posts");
        assert_eq!(listed.tables[0].keys[0].name, "id");
        assert_eq!(commands, ["ls", "ls nb_posts", "ls catalog"]);
        assert!(catalog.as_ref().is_some_and(|tables| tables.len() == 2));
        assert!(shapes.contains_key("nb_posts"));

        commands.clear();
        let cached = list_tables("nb_*", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(cached.tables[0].name, "nb_posts");
        assert!(commands.is_empty());

        commands.clear();
        let refreshed = list_tables("nb_*", true, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(refreshed.note, "");
        assert_eq!(commands, ["ls refresh=True", "ls nb_posts", "ls catalog"]);
        assert!(shapes.is_empty());

        commands.clear();
        let similar = list_tables("post", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(
            similar.note,
            "No exact match for \"post\", so showing similar names."
        );
        assert_eq!(similar.tables[0].name, "nb_posts");
        assert!(commands.is_empty());

        commands.clear();
        let one = list_tables("catalog", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(one.note, "");
        assert_eq!(one.tables.len(), 1);
        assert_eq!(one.tables[0].name, "catalog");
        assert_eq!(one.tables[0].keys[0].name, "isbn");
        assert!(commands.is_empty());

        commands.clear();
        let related = list_tables("isbn", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert_eq!(
            related.note,
            "No exact match for \"isbn\", so showing related keys."
        );
        assert_eq!(related.tables[0].name, "catalog");
        assert!(commands.is_empty());

        commands.clear();
        let missed = list_tables("missing", false, &mut shapes, &mut catalog, |command| {
            commands.push(command.to_string());
            fake_ls(command)
        })
        .unwrap();
        assert!(missed.tables.is_empty());
        assert_eq!(missed.note, "");
        assert!(commands.is_empty());

        let mut empty = None;
        assert!(
            list_tables("nb_*", false, &mut shapes, &mut empty, |_| ExecText {
                ok: false,
                message: String::new(),
                error: "connection refused".to_string(),
            })
            .is_err()
        );
    }

    fn fake_ls(command: &str) -> ExecText {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let target = parts
            .iter()
            .skip(1)
            .find(|part| !part.starts_with("refresh="))
            .map(|raw| raw.trim_matches(['\'', '"']))
            .unwrap_or("");
        let message = match target {
            "nb_posts" => "Name: nb_posts\nHash Key: id (STRING)\n".to_string(),
            "catalog" => "Name: catalog\nHash Key: isbn (STRING)\n".to_string(),
            "" => "Tables\nName Items\nnb_posts 1\ncatalog 1\n".to_string(),
            _ => {
                return ExecText {
                    ok: false,
                    message: String::new(),
                    error: format!("unexpected command {command}"),
                };
            }
        };
        ExecText {
            ok: true,
            message,
            error: String::new(),
        }
    }
}
