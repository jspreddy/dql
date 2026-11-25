//! Table browser reads: the structured read the browser sends, turned into the statements to run.
//!
//! The browser holds a read as a mode, a source, key rows, filter rows, an order, and the Keys box
//! text. This module validates it against the table's definition and writes the DQL, quoting
//! names and values the way the `dql-parser` lexer reads them. The preview, Run, and Explain all
//! come from here (rust-docs/plans/todo/table_reads_server.md).

// The endpoints that call `plan` arrive in step 3 of that plan, which removes this allowance.
#![cfg_attr(not(test), allow(dead_code))]

use std::collections::HashSet;

use serde::Deserialize;

use super::shape::ListedTable;

/// The read the table browser sends.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableRead {
    pub table: String,
    /// `scan`, `query`, or `keys`.
    pub mode: String,
    /// `""` lets dqlrs choose, `"-"` is the table (`USING -`), anything else names an index.
    pub source: String,
    /// `forward` or `reverse`.
    pub order: String,
    pub keys: Vec<ReadRow>,
    pub filters: Vec<ReadRow>,
    pub batch_keys: String,
    /// Attribute names seen in rows that the definition does not list. They have no type.
    pub extra: Vec<String>,
}

/// One key or filter row.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReadRow {
    pub attribute: String,
    pub op: String,
    pub value: String,
    pub value2: String,
    pub role: String,
    pub size_op: String,
}

/// The source the browser calls the table.
pub const TABLE_SOURCE: &str = "-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Compare,
    Between,
    Function,
    Exists,
    Type,
    Size,
}

#[derive(Clone, Copy, Debug)]
struct Op {
    id: &'static str,
    dql: &'static str,
    kind: Kind,
}

const fn op(id: &'static str, dql: &'static str, kind: Kind) -> Op {
    Op { id, dql, kind }
}

const EQ: Op = op("eq", "=", Kind::Compare);
const COMPARE_OPS: [Op; 7] = [
    EQ,
    op("ne", "<>", Kind::Compare),
    op("lt", "<", Kind::Compare),
    op("le", "<=", Kind::Compare),
    op("gt", ">", Kind::Compare),
    op("ge", ">=", Kind::Compare),
    op("between", "BETWEEN", Kind::Between),
];
const BEGINS_WITH: Op = op("begins_with", "begins_with", Kind::Function);
const FUNCTION_OPS: [Op; 6] = [
    BEGINS_WITH,
    op("contains", "contains", Kind::Function),
    op("exists", "attribute_exists", Kind::Exists),
    op("not_exists", "attribute_not_exists", Kind::Exists),
    op("attribute_type", "attribute_type", Kind::Type),
    op("size", "size", Kind::Size),
];

/// The ops a row may use, as the browser's menus offer them.
fn ops_for(filter: bool, role: &str, mode: &str) -> Vec<Op> {
    if filter {
        return COMPARE_OPS
            .iter()
            .chain(FUNCTION_OPS.iter())
            .copied()
            .collect();
    }
    if mode == "query" && role == "hash" {
        return vec![EQ];
    }
    if mode == "query" {
        let mut ops: Vec<Op> = COMPARE_OPS
            .iter()
            .filter(|op| op.id != "ne")
            .copied()
            .collect();
        ops.push(BEGINS_WITH);
        return ops;
    }
    let mut ops = COMPARE_OPS.to_vec();
    ops.push(BEGINS_WITH);
    ops
}

/// An op by id, or `=` when the id is not one of `ops`.
fn op_by_id(id: &str, ops: &[Op]) -> Op {
    ops.iter().find(|op| op.id == id).copied().unwrap_or(EQ)
}

fn size_compare(id: &str) -> Op {
    COMPARE_OPS
        .iter()
        .filter(|op| op.kind == Kind::Compare)
        .find(|op| op.id == id)
        .copied()
        .unwrap_or(COMPARE_OPS[4])
}

#[derive(Clone, Debug, Default)]
struct Source {
    name: String,
    hash_key: String,
    range_key: String,
}

impl Source {
    fn uses_index(&self) -> bool {
        !self.name.is_empty() && self.name != TABLE_SOURCE
    }
}

fn table_source(table: &ListedTable) -> Source {
    let key = |role: &str| {
        table
            .keys
            .iter()
            .find(|key| key.role == role)
            .map(|key| key.name.clone())
            .unwrap_or_default()
    };
    Source {
        name: String::new(),
        hash_key: key("hash"),
        range_key: key("range"),
    }
}

fn available_sources(table: &ListedTable, mode: &str) -> Vec<Source> {
    let base = table_source(table);
    let mut sources = vec![
        base.clone(),
        Source {
            name: TABLE_SOURCE.to_string(),
            ..base
        },
    ];
    for index in &table.indexes {
        if mode == "scan" && index.kind == "local" {
            continue;
        }
        sources.push(Source {
            name: index.name.clone(),
            hash_key: index.hash_key.clone(),
            range_key: index.range_key.clone().unwrap_or_default(),
        });
    }
    sources
}

fn source_of(table: &ListedTable, name: &str, mode: &str) -> Source {
    let sources = available_sources(table, mode);
    sources
        .iter()
        .find(|source| source.name == name)
        .cloned()
        .unwrap_or_else(|| sources[0].clone())
}

/// The source's keys as (name, role), hash first.
fn source_keys(table: &ListedTable, name: &str, mode: &str) -> Vec<(String, &'static str)> {
    let source = source_of(table, name, mode);
    let mut keys = Vec::new();
    if !source.hash_key.is_empty() {
        keys.push((source.hash_key.clone(), "hash"));
    }
    if !source.range_key.is_empty() && source.range_key != source.hash_key {
        keys.push((source.range_key.clone(), "range"));
    }
    keys
}

/// The attribute's type from the definition, or `""`. Extra attributes have no type.
fn attribute_type<'a>(table: &'a ListedTable, name: &str) -> &'a str {
    table
        .attributes
        .iter()
        .find(|attr| attr.name == name)
        .map(|attr| attr.data_type.as_str())
        .unwrap_or("")
}

fn is_number_type(kind: &str) -> bool {
    matches!(kind.to_ascii_uppercase().as_str(), "NUMBER" | "N")
}

/// The parser's identifier rule. Keywords are identifiers too.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
}

/// A string literal the lexer reads back as `text`: `\` and `'` are escaped with `\`.
fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        if ch == '\\' || ch == '\'' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

/// `text` as a plain decimal the lexer reads (`-12`, `0.5`, `1000`), or `None` when it is not a
/// number. Accepts what people type: `+3`, `.5`, `6.`, `1e3`. Works on the digits, so a 38-digit
/// DynamoDB number keeps every digit.
fn decimal(text: &str) -> Option<String> {
    let text = text.trim();
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => {
            let exponent: i64 = rest[at + 1..].parse().ok()?;
            (&rest[..at], exponent)
        }
        None => (rest, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits: String = format!("{whole}{fraction}");
    let point = whole.len() as i64 + exponent;
    let (int_part, frac_part) = if point <= 0 {
        (
            "0".to_string(),
            format!("{}{digits}", "0".repeat((-point) as usize)),
        )
    } else if point as usize >= digits.len() {
        (
            format!("{digits}{}", "0".repeat(point as usize - digits.len())),
            String::new(),
        )
    } else {
        (
            digits[..point as usize].to_string(),
            digits[point as usize..].to_string(),
        )
    };
    let int_part = int_part.trim_start_matches('0');
    let int_part = if int_part.is_empty() { "0" } else { int_part };
    let frac_part = frac_part.trim_end_matches('0');
    let body = if frac_part.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{frac_part}")
    };
    Some(if negative && body != "0" {
        format!("-{body}")
    } else {
        body
    })
}

/// Why `raw` cannot be a value of `name`, a NUMBER, or `None` when it can.
fn number_error(name: &str, raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() || decimal(text).is_some() {
        return None;
    }
    Some(format!(
        "{name} is a number. {} is not.",
        serde_json::to_string(text).unwrap_or_default()
    ))
}

/// `raw` written for an attribute of type `kind`. NUMBER values must already have passed
/// `number_error`.
fn value_literal(raw: &str, kind: &str) -> String {
    let trimmed = raw.trim();
    if is_number_type(kind) {
        return decimal(trimmed).unwrap_or_else(|| trimmed.to_string());
    }
    let upper = kind.to_ascii_uppercase();
    if (upper == "BOOL" || upper == "BOOLEAN")
        && (trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false"))
    {
        return trimmed.to_ascii_uppercase();
    }
    if kind.is_empty() {
        if is_plain_number(trimmed) {
            return trimmed.to_string();
        }
        if ["true", "false", "null"]
            .iter()
            .any(|word| trimmed.eq_ignore_ascii_case(word))
        {
            return trimmed.to_ascii_uppercase();
        }
    }
    string_literal(raw)
}

/// `-?\d+(\.\d+)?`: a value of an untyped attribute written as a number.
fn is_plain_number(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// The Keys box: a list of keys, each one value or a (hash, range) pair.
struct BatchKeys {
    keys: Vec<Vec<String>>,
    errors: Vec<String>,
}

enum Elem {
    Value(String),
    Keys(Vec<Vec<String>>),
}

struct KeysParser<'a> {
    src: &'a [char],
    at: usize,
    errors: Vec<String>,
}

impl KeysParser<'_> {
    fn line_at(&self, index: usize) -> usize {
        1 + self.src[..index.min(self.src.len())]
            .iter()
            .filter(|ch| **ch == '\n')
            .count()
    }

    fn is_sep(ch: char) -> bool {
        matches!(ch, ',' | '\n' | '\r')
    }

    fn is_space(ch: char) -> bool {
        matches!(ch, ' ' | '\t')
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.at).copied()
    }

    fn skip_idle(&mut self) {
        while self
            .peek()
            .is_some_and(|ch| Self::is_space(ch) || Self::is_sep(ch))
        {
            self.at += 1;
        }
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(Self::is_space) {
            self.at += 1;
        }
    }

    fn quoted(&mut self, quote: char) -> Option<String> {
        self.at += 1;
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch == quote && self.src.get(self.at + 1) == Some(&quote) {
                out.push(quote);
                self.at += 2;
                continue;
            }
            self.at += 1;
            if ch == quote {
                return Some(out);
            }
            out.push(ch);
        }
        None
    }

    fn bare(&mut self) -> String {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|ch| !Self::is_sep(ch) && ch != '(' && ch != ')')
        {
            self.at += 1;
        }
        self.src[start..self.at]
            .iter()
            .collect::<String>()
            .trim()
            .to_string()
    }

    fn value(&mut self) -> Result<String, String> {
        self.skip_space();
        let Some(ch) = self.peek() else {
            return Ok(String::new());
        };
        if ch == '\'' || ch == '"' {
            let start = self.at;
            return self
                .quoted(ch)
                .ok_or_else(|| format!("Line {} has an unclosed quote.", self.line_at(start)));
        }
        if Self::is_sep(ch) || ch == '(' || ch == ')' {
            return Ok(String::new());
        }
        Ok(self.bare())
    }

    fn collapse(&mut self, elems: Vec<Elem>, open_at: usize) -> Vec<Vec<String>> {
        if elems.iter().all(|elem| matches!(elem, Elem::Value(_))) {
            let values: Vec<String> = elems
                .into_iter()
                .filter_map(|elem| match elem {
                    Elem::Value(value) => Some(value),
                    Elem::Keys(_) => None,
                })
                .collect();
            if values.len() > 2 {
                let line = self.line_at(open_at);
                self.errors.push(format!(
                    "Line {line}: a key has at most two parts: (hash, range)."
                ));
                return Vec::new();
            }
            if values.len() == 2 {
                return vec![values];
            }
            return values.into_iter().map(|value| vec![value]).collect();
        }
        let mut nested = Vec::new();
        for elem in elems {
            match elem {
                Elem::Value(value) => nested.push(vec![value]),
                Elem::Keys(keys) => nested.extend(keys),
            }
        }
        nested
    }

    fn group(&mut self, open_at: usize) -> Vec<Vec<String>> {
        let mut elems = Vec::new();
        while self.at < self.src.len() {
            self.skip_idle();
            match self.peek() {
                None => break,
                Some(')') => {
                    self.at += 1;
                    return self.collapse(elems, open_at);
                }
                Some('(') => {
                    let nested_at = self.at;
                    self.at += 1;
                    let keys = self.group(nested_at);
                    elems.push(Elem::Keys(keys));
                }
                Some(_) => match self.value() {
                    Ok(value) => {
                        if !value.is_empty() {
                            elems.push(Elem::Value(value));
                        }
                    }
                    Err(error) => {
                        self.errors.push(error);
                        return self.collapse(elems, open_at);
                    }
                },
            }
        }
        let line = self.line_at(open_at);
        self.errors.push(format!("Line {line} has an unclosed (."));
        self.collapse(elems, open_at)
    }
}

fn parse_batch_keys(text: &str) -> BatchKeys {
    let chars: Vec<char> = text.chars().collect();
    let mut parser = KeysParser {
        src: &chars,
        at: 0,
        errors: Vec::new(),
    };
    let mut keys = Vec::new();
    parser.skip_idle();
    while parser.at < chars.len() {
        parser.skip_idle();
        let Some(ch) = parser.peek() else {
            break;
        };
        let start = parser.at;
        if ch == ')' {
            let line = parser.line_at(start);
            parser.errors.push(format!("Line {line} has an extra )."));
            parser.at += 1;
            continue;
        }
        if ch == '(' {
            parser.at += 1;
            keys.extend(parser.group(start));
            continue;
        }
        match parser.value() {
            Ok(value) => {
                if !value.is_empty() {
                    keys.push(vec![value]);
                }
            }
            Err(error) => {
                parser.errors.push(error);
                break;
            }
        }
    }
    BatchKeys {
        keys,
        errors: parser.errors,
    }
}

/// `keys` without repeats. A pair whose hash is also listed alone is dropped: the lone value
/// already reads every item under that hash key. DynamoDB rejects a batch get that repeats a key.
fn dedupe_keys(keys: Vec<Vec<String>>, table: &ListedTable, source: &Source) -> Vec<Vec<String>> {
    let canonical = |value: &str, name: &str| {
        if is_number_type(attribute_type(table, name)) {
            if let Some(number) = decimal(value) {
                return format!("N:{number}");
            }
        }
        format!("S:{value}")
    };
    let lone: HashSet<String> = if source.range_key.is_empty() {
        HashSet::new()
    } else {
        keys.iter()
            .filter(|fields| fields.len() == 1)
            .map(|fields| canonical(&fields[0], &source.hash_key))
            .collect()
    };
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for fields in keys {
        if fields.len() > 1 && lone.contains(&canonical(&fields[0], &source.hash_key)) {
            continue;
        }
        let id = fields
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let name = if index == 0 {
                    &source.hash_key
                } else {
                    &source.range_key
                };
                canonical(value, name)
            })
            .collect::<Vec<_>>()
            .join("\u{0}");
        if seen.insert(id) {
            unique.push(fields);
        }
    }
    unique
}

fn keys_mode_error(table: &ListedTable, parsed: &BatchKeys, source: &Source) -> Option<String> {
    if let Some(error) = parsed.errors.first() {
        return Some(error.clone());
    }
    if parsed.keys.is_empty() {
        return Some("Keys needs at least one key.".to_string());
    }
    if source.range_key.is_empty() && parsed.keys.iter().any(|key| key.len() > 1) {
        return Some(if source.uses_index() {
            "This index only has a hash key.".to_string()
        } else {
            "This table only has a hash key.".to_string()
        });
    }
    for fields in &parsed.keys {
        for (index, value) in fields.iter().enumerate() {
            let name = if index == 0 {
                &source.hash_key
            } else {
                &source.range_key
            };
            if name.is_empty() || !is_number_type(attribute_type(table, name)) {
                continue;
            }
            if let Some(error) = number_error(name, value) {
                return Some(error);
            }
        }
    }
    None
}

/// In Query mode the source's hash key gets a locked `=` row. Other modes keep the rows as sent.
fn ensure_query_keys(table: &ListedTable, read: &TableRead) -> Vec<ReadRow> {
    let mut keys = read.keys.clone();
    if read.mode != "query" {
        return keys;
    }
    let source_keys = source_keys(table, &read.source, &read.mode);
    let Some((hash, _)) = source_keys.iter().find(|(_, role)| *role == "hash") else {
        if keys.is_empty() {
            keys.push(ReadRow {
                op: "eq".to_string(),
                role: "hash".to_string(),
                ..ReadRow::default()
            });
        }
        return keys;
    };
    if let Some(row) = keys.iter_mut().find(|row| &row.attribute == hash) {
        row.role = "hash".to_string();
        row.op = "eq".to_string();
        return keys;
    }
    keys.insert(
        0,
        ReadRow {
            attribute: hash.clone(),
            op: "eq".to_string(),
            role: "hash".to_string(),
            ..ReadRow::default()
        },
    );
    keys
}

/// The read with Query's hash row filled in, and the source it reads.
struct Prepared<'a> {
    table: &'a ListedTable,
    read: &'a TableRead,
    keys: Vec<ReadRow>,
    source: Source,
}

impl<'a> Prepared<'a> {
    fn new(table: &'a ListedTable, read: &'a TableRead) -> Self {
        let mode = if read.mode == "keys" {
            "query"
        } else {
            read.mode.as_str()
        };
        Self {
            table,
            read,
            keys: ensure_query_keys(table, read),
            source: source_of(table, &read.source, mode),
        }
    }

    /// Every name the statement will contain, with what it is. A name the statement does not use
    /// (a key attribute in a plain Scan) is not checked.
    fn names(&self) -> Vec<(String, &'static str)> {
        let mut names = vec![(self.read.table.clone(), "table")];
        for row in &self.keys {
            names.push((row.attribute.clone(), "key"));
        }
        for row in &self.read.filters {
            names.push((row.attribute.clone(), "filter"));
        }
        if self.source.uses_index() {
            names.push((self.source.name.clone(), "index"));
            if self.read.mode == "keys" {
                names.push((self.source.hash_key.clone(), "key"));
                names.push((self.source.range_key.clone(), "key"));
            }
        }
        if self.read.mode == "scan" && self.read.order == "reverse" {
            let key = if self.source.range_key.is_empty() {
                &self.source.hash_key
            } else {
                &self.source.range_key
            };
            names.push((key.clone(), "key"));
        }
        names
            .into_iter()
            .filter(|(name, _)| !name.trim().is_empty())
            .collect()
    }

    fn name_error(&self) -> Option<String> {
        self.names().into_iter().find_map(|(name, kind)| {
            let name = name.trim();
            (!is_identifier(name)).then(|| format!("{name} cannot be used as a {kind} name"))
        })
    }

    /// The message for a read that cannot run, in the order the browser checked them.
    fn error(&self) -> Option<String> {
        if self.read.table.trim().is_empty() {
            return Some("Choose a table.".to_string());
        }
        if let Some(error) = self.name_error() {
            return Some(error);
        }
        if self.read.mode == "keys" {
            return keys_mode_error(
                self.table,
                &parse_batch_keys(&self.read.batch_keys),
                &self.source,
            );
        }
        if self.read.mode == "query" {
            let source_keys = source_keys(self.table, &self.read.source, &self.read.mode);
            let hash = source_keys.iter().find(|(_, role)| *role == "hash");
            let row = match hash {
                Some((name, _)) => self.keys.iter().find(|row| &row.attribute == name),
                None => self
                    .keys
                    .iter()
                    .find(|row| !row.attribute.trim().is_empty()),
            };
            match row {
                Some(row) if !row.attribute.trim().is_empty() => {
                    if row.value.is_empty() {
                        return Some("Query needs a hash key value.".to_string());
                    }
                }
                _ => return Some("Query needs a hash key.".to_string()),
            }
        }
        self.number_errors()
    }

    /// A NUMBER attribute compared with text, and a `size` compared with text, cannot be written.
    fn number_errors(&self) -> Option<String> {
        let rows = self
            .keys
            .iter()
            .map(|row| (row, false))
            .chain(self.read.filters.iter().map(|row| (row, true)));
        for (row, filter) in rows {
            let name = row.attribute.trim();
            if name.is_empty() {
                continue;
            }
            let op = op_by_id(&row.op, &ops_for(filter, &self.role(row), &self.read.mode));
            let values: Vec<&str> = match op.kind {
                Kind::Compare | Kind::Function
                    if is_number_type(attribute_type(self.table, name)) =>
                {
                    vec![&row.value]
                }
                Kind::Between if is_number_type(attribute_type(self.table, name)) => {
                    vec![&row.value, &row.value2]
                }
                Kind::Size => {
                    if let Some(error) = number_error(&format!("size({name})"), &row.value) {
                        return Some(error);
                    }
                    continue;
                }
                _ => continue,
            };
            // begins_with and contains on a NUMBER are DynamoDB's to refuse, not ours.
            if op.kind == Kind::Function {
                continue;
            }
            for value in values {
                if let Some(error) = number_error(name, value) {
                    return Some(error);
                }
            }
        }
        None
    }

    fn role(&self, row: &ReadRow) -> String {
        if !row.role.is_empty() {
            return row.role.clone();
        }
        source_keys(self.table, &self.read.source, &self.read.mode)
            .into_iter()
            .find(|(name, _)| name == &row.attribute)
            .map(|(_, role)| role.to_string())
            .unwrap_or_default()
    }

    /// One condition, or `None` for a row that is not filled in.
    fn condition(&self, row: &ReadRow, filter: bool) -> Option<String> {
        let field = row.attribute.trim();
        if field.is_empty() {
            return None;
        }
        let op = op_by_id(&row.op, &ops_for(filter, &self.role(row), &self.read.mode));
        let kind = attribute_type(self.table, field);
        match op.kind {
            Kind::Exists => Some(format!("{}({field})", op.dql)),
            Kind::Type => {
                let ty = row.value.trim();
                let ty = if ty.is_empty() { "S" } else { ty };
                Some(format!("{}({field}, {})", op.dql, string_literal(ty)))
            }
            Kind::Size => {
                if row.value.is_empty() {
                    return None;
                }
                let compare = size_compare(if row.size_op.is_empty() {
                    "gt"
                } else {
                    &row.size_op
                });
                Some(format!(
                    "size({field}) {} {}",
                    compare.dql,
                    value_literal(&row.value, "NUMBER")
                ))
            }
            Kind::Function => {
                if row.value.is_empty() {
                    return None;
                }
                Some(format!(
                    "{}({field}, {})",
                    op.dql,
                    value_literal(&row.value, kind)
                ))
            }
            Kind::Between => {
                if row.value.is_empty() || row.value2.is_empty() {
                    return None;
                }
                Some(format!(
                    "{field} BETWEEN {} AND {}",
                    value_literal(&row.value, kind),
                    value_literal(&row.value2, kind)
                ))
            }
            Kind::Compare => {
                if row.value.is_empty() {
                    return None;
                }
                Some(format!(
                    "{field} {} {}",
                    op.dql,
                    value_literal(&row.value, kind)
                ))
            }
        }
    }

    fn using(&self) -> String {
        if self.source.name.is_empty() {
            String::new()
        } else {
            format!(" USING {}", self.source.name)
        }
    }

    fn statements(&self) -> Vec<String> {
        let table = self.read.table.trim();
        if self.read.mode == "keys" {
            let parsed = parse_batch_keys(&self.read.batch_keys);
            let keys = dedupe_keys(parsed.keys, self.table, &self.source);
            let literal =
                |value: &str, name: &str| value_literal(value, attribute_type(self.table, name));
            if self.source.uses_index() {
                let hash = if self.source.hash_key.is_empty() {
                    "id"
                } else {
                    self.source.hash_key.as_str()
                };
                return keys
                    .iter()
                    .map(|fields| {
                        let mut sql = format!(
                            "SELECT * FROM {table} WHERE {hash} = {}",
                            literal(&fields[0], hash)
                        );
                        if fields.len() > 1 && !self.source.range_key.is_empty() {
                            let range = &self.source.range_key;
                            sql.push_str(&format!(" AND {range} = {}", literal(&fields[1], range)));
                        }
                        sql.push_str(&self.using());
                        sql
                    })
                    .collect();
            }
            // One KEYS IN for the table: a batch get of whole keys, and every item under a lone
            // hash value on a table with a range key.
            let list = keys
                .iter()
                .map(|fields| {
                    let parts: Vec<String> = fields
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            let name = if index == 0 {
                                &self.source.hash_key
                            } else {
                                &self.source.range_key
                            };
                            literal(value, name)
                        })
                        .collect();
                    if parts.len() == 1 {
                        parts[0].clone()
                    } else {
                        format!("({})", parts.join(", "))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            return vec![format!("SELECT * FROM {table} KEYS IN {list}")];
        }
        let verb = if self.read.mode == "query" {
            "SELECT"
        } else {
            "SCAN"
        };
        let conditions: Vec<String> = self
            .keys
            .iter()
            .filter_map(|row| self.condition(row, false))
            .chain(
                self.read
                    .filters
                    .iter()
                    .filter_map(|row| self.condition(row, true)),
            )
            .collect();
        let mut sql = format!("{verb} * FROM {table}");
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(&self.using());
        if self.read.order == "reverse" {
            if self.read.mode == "query" {
                sql.push_str(" DESC");
            } else {
                let key = if self.source.range_key.is_empty() {
                    &self.source.hash_key
                } else {
                    &self.source.range_key
                };
                if !key.is_empty() {
                    sql.push_str(&format!(" ORDER BY {key} DESC"));
                }
            }
        }
        vec![sql]
    }
}

/// The statements a read runs, in order, or the message that says why it cannot run. Never an
/// empty list without a message.
pub fn plan(table: &ListedTable, read: &TableRead) -> Result<Vec<String>, String> {
    let prepared = Prepared::new(table, read);
    if let Some(error) = prepared.error() {
        return Err(error);
    }
    let statements = prepared.statements();
    if statements.is_empty() {
        return Err("This read has nothing to run.".to_string());
    }
    Ok(statements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::{parse_statement, Statement};
    use serde_json::Value;

    #[derive(Deserialize)]
    struct Case {
        name: String,
        table: ListedTable,
        read: TableRead,
        dql: String,
        error: String,
    }

    fn cases() -> Vec<Case> {
        let text = include_str!("../../tests/fixtures/table_reads.json");
        serde_json::from_str(text).expect("golden cases")
    }

    #[test]
    fn golden_cases() {
        let cases = cases();
        assert!(cases.len() >= 50);
        let mut failures = Vec::new();
        for case in &cases {
            let read = TableRead {
                table: case.table.name.clone(),
                ..case.read.clone()
            };
            let (dql, error) = match plan(&case.table, &read) {
                Ok(statements) => (statements.join("; "), String::new()),
                Err(error) => (String::new(), error),
            };
            if dql != case.dql || error != case.error {
                failures.push(format!(
                    "{}\n  want dql {:?} error {:?}\n  got  dql {dql:?} error {error:?}",
                    case.name, case.dql, case.error
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn every_golden_statement_parses_and_reads_its_table() {
        for case in cases() {
            let read = TableRead {
                table: case.table.name.clone(),
                ..case.read.clone()
            };
            let Ok(statements) = plan(&case.table, &read) else {
                continue;
            };
            for statement in statements {
                let parsed = parse_statement(&statement)
                    .unwrap_or_else(|err| panic!("{}: {statement}: {err}", case.name));
                let table = match parsed {
                    Statement::Select { table, .. } | Statement::Scan { table, .. } => table,
                    other => panic!("{}: {other:?}", case.name),
                };
                assert_eq!(table, case.table.name, "{}", case.name);
            }
        }
    }

    #[test]
    fn a_value_with_a_quote_or_backslash_reads_back_as_typed() {
        for text in ["it's", "a\\b", "\\'", "''", "end\\"] {
            let statement = format!("SCAN * FROM t WHERE a = {}", string_literal(text));
            let parsed = parse_statement(&statement).unwrap();
            let Statement::Scan {
                condition: Some(dql_parser::Condition::Compare { rhs, .. }),
                ..
            } = parsed
            else {
                panic!("{statement}");
            };
            assert_eq!(
                rhs,
                dql_parser::ConditionOperand::Value(dql_parser::Value::String(text.to_string())),
                "{statement}"
            );
        }
    }

    #[test]
    fn numbers_are_written_as_plain_decimals() {
        for (typed, written) in [
            ("1", "1"),
            ("-2", "-2"),
            ("+3", "3"),
            ("4.5", "4.5"),
            (".5", "0.5"),
            ("6.", "6"),
            ("1e3", "1000"),
            ("2.5E-4", "0.00025"),
            ("1.0", "1"),
            ("-0", "0"),
            ("007", "7"),
            (
                "12345678901234567890123456789012345678",
                "12345678901234567890123456789012345678",
            ),
        ] {
            assert_eq!(decimal(typed).as_deref(), Some(written), "{typed}");
            parse_statement(&format!("SCAN * FROM t WHERE n = {written}")).unwrap();
        }
        for bad in ["", "abc", "1e", "e3", "1.2.3", "--1", "+", "."] {
            assert_eq!(decimal(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_request_reads_the_browser_json() {
        let read: TableRead = serde_json::from_value(serde_json::json!({
            "table": "posts",
            "mode": "query",
            "source": "by-status",
            "order": "reverse",
            "keys": [{ "attribute": "status", "op": "eq", "value": "open", "role": "hash", "locked": true, "id": "hash" }],
            "filters": [{ "attribute": "kind", "op": "size", "value": "2", "sizeOp": "le" }],
            "batchKeys": "",
            "extra": ["note"],
        }))
        .unwrap();
        assert_eq!(read.filters[0].size_op, "le");
        assert_eq!(read.extra, ["note"]);
        let _: Value = serde_json::to_value(read.keys.len()).unwrap();
    }
}
