//! Check a DQL script without running it.
//!
//! Parse errors are reported at the statement. When a table definition is
//! available from an earlier `CREATE TABLE` in the script, or from the cached
//! description, reads and writes are checked the same way execution would
//! reject them. A table with no definition is a warning, not a fetch.

use std::collections::{HashMap, HashSet};

use dql_expr::render_condition;
use dql_models::{plan_read, PlanError, PlanInput, ReadKind, TableMeta};
use dql_parser::{parse_statement, Condition, InsertForm, QueryOptions, Selection, Statement};
use serde::Serialize;

use crate::session::Session;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LintIssue {
    pub severity: String,
    pub message: String,
    pub offset: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LintReport {
    pub ok: bool,
    pub issues: Vec<LintIssue>,
}

struct Span {
    from: usize,
    to: usize,
    text: String,
    closed: bool,
}

struct Schemas {
    local: HashMap<String, TableMeta>,
    dropped: HashSet<String>,
}

pub fn lint_script(session: &mut Session, text: &str) -> LintReport {
    let allow_select_scan = session.config.allow_select_scan;
    let spans = statement_spans(text);
    let mut schemas = Schemas {
        local: HashMap::new(),
        dropped: HashSet::new(),
    };
    let mut issues = Vec::new();
    for span in &spans {
        match parse_statement(&span.text) {
            Ok(statement) => {
                let mut cx = Cx {
                    source: text,
                    span,
                    schemas: &mut schemas,
                    session,
                    allow_select_scan,
                    issues: &mut issues,
                };
                check_statement(&statement, &mut cx, true);
            }
            Err(err) => {
                let byte = err.location().unwrap_or(0);
                let at = span.from + utf16_index(&span.text, byte);
                push_issue(
                    &mut issues,
                    text,
                    if span.closed { "error" } else { "warning" },
                    err.to_string(),
                    at,
                    span.to,
                );
            }
        }
    }
    LintReport {
        ok: issues.iter().all(|issue| issue.severity != "error"),
        issues,
    }
}

struct Cx<'a> {
    source: &'a str,
    span: &'a Span,
    schemas: &'a mut Schemas,
    session: &'a mut Session,
    allow_select_scan: bool,
    issues: &'a mut Vec<LintIssue>,
}

fn check_statement(statement: &Statement, cx: &mut Cx<'_>, apply_schema: bool) {
    match statement {
        Statement::CreateTable { .. } => match TableMeta::from_create_statement(statement) {
            Ok(meta) => {
                if apply_schema {
                    cx.schemas.dropped.remove(&meta.name);
                    cx.schemas.local.insert(meta.name.clone(), meta);
                }
            }
            Err(err) => cx.error(err.to_string()),
        },
        Statement::DropTable { name, .. } => {
            if apply_schema {
                cx.schemas.local.remove(name);
                cx.schemas.dropped.insert(name.clone());
            }
        }
        Statement::Explain(inner) | Statement::Analyze(inner) => {
            check_statement(inner, cx, false);
        }
        Statement::Insert { table, form, .. } => check_insert(cx, table, form),
        Statement::Select {
            table,
            selection,
            condition,
            options,
            ..
        } => check_read(
            cx,
            table,
            ReadKind::Select,
            selection,
            condition.as_ref(),
            options,
        ),
        Statement::Scan {
            table,
            selection,
            condition,
            options,
            ..
        } => check_read(
            cx,
            table,
            ReadKind::Scan,
            selection,
            condition.as_ref(),
            options,
        ),
        Statement::Update {
            table,
            condition,
            options,
            ..
        }
        | Statement::Delete {
            table,
            condition,
            options,
            ..
        } => check_read(
            cx,
            table,
            ReadKind::Scan,
            &Selection::All,
            condition.as_ref(),
            options,
        ),
        Statement::AlterTable { table, .. } => {
            let _ = schema_for(cx, table);
        }
        _ => {}
    }
}

fn check_insert(cx: &mut Cx<'_>, table: &str, form: &InsertForm) {
    if let InsertForm::Values { columns, rows } = form {
        for row in rows {
            if row.len() != columns.len() {
                cx.error(format!(
                    "values have {} column(s) but the insert lists {}",
                    row.len(),
                    columns.len()
                ));
                return;
            }
        }
    }
    let Some(meta) = schema_for(cx, table) else {
        return;
    };
    match form {
        InsertForm::Values { columns, .. } => require_keys(cx, table, columns, &meta),
        InsertForm::Keyword { rows } => {
            for row in rows {
                let columns = row.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>();
                require_keys(cx, table, &columns, &meta);
            }
        }
    }
}

fn require_keys(cx: &mut Cx<'_>, table: &str, columns: &[String], meta: &TableMeta) {
    if !columns.iter().any(|column| column == &meta.hash_key) {
        cx.error(format!(
            "insert into {table} is missing hash key '{}'",
            meta.hash_key
        ));
    }
    if let Some(range_key) = &meta.range_key {
        if !columns.iter().any(|column| column == range_key) {
            cx.error(format!(
                "insert into {table} is missing range key '{range_key}'"
            ));
        }
    }
}

fn check_read(
    cx: &mut Cx<'_>,
    table: &str,
    kind: ReadKind,
    selection: &Selection,
    condition: Option<&Condition>,
    options: &QueryOptions,
) {
    if let Some(message) = keys_in_message(&kind, options, condition) {
        cx.error(message);
        return;
    }
    if matches!(selection, Selection::CountAll) && options.save_file.is_some() {
        cx.error("Cannot use count(*) with SAVE".to_string());
        return;
    }
    if let Some(condition) = condition {
        if let Err(err) = render_condition(condition) {
            cx.error(err.to_string());
            return;
        }
    }
    let Some(meta) = schema_for(cx, table) else {
        return;
    };
    if let Some(keys) = &options.keys_in {
        let width = meta.primary_key_attributes().len();
        for key in keys {
            if key.len() != width {
                cx.error(format!(
                    "primary key has {} value(s) but {table} expects {width}",
                    key.len()
                ));
                return;
            }
        }
        return;
    }
    let allow = match kind {
        ReadKind::Select => cx.allow_select_scan,
        ReadKind::Scan => true,
    };
    if let Err(err) = plan_read(PlanInput {
        table: &meta,
        kind,
        condition,
        selection: Some(selection),
        using_index: options.using_index.as_deref(),
        allow_select_scan: allow,
    }) {
        cx.error(plan_message(err));
    }
}

impl Cx<'_> {
    fn error(&mut self, message: String) {
        let from = self.span.from;
        let to = self.span.to;
        push_issue(self.issues, self.source, "error", message, from, to);
    }

    fn warning(&mut self, message: String) {
        let from = self.span.from;
        let to = self.span.to;
        push_issue(self.issues, self.source, "warning", message, from, to);
    }
}

fn keys_in_message(
    kind: &ReadKind,
    options: &QueryOptions,
    condition: Option<&Condition>,
) -> Option<String> {
    options.keys_in.as_ref()?;
    if options.using_index.is_some() {
        return Some("Cannot use USING with KEYS IN".to_string());
    }
    if matches!(kind, ReadKind::Select | ReadKind::Scan) {
        if options.limit.is_some() {
            return Some("Cannot use LIMIT with KEYS IN".to_string());
        }
        if options.order_by.is_some() {
            return Some("Cannot use ORDER BY with KEYS IN".to_string());
        }
        if condition.is_some() {
            return Some("Cannot use WHERE with KEYS IN".to_string());
        }
    }
    None
}

fn schema_for(cx: &mut Cx<'_>, table: &str) -> Option<TableMeta> {
    if let Some(meta) = cx.schemas.local.get(table) {
        return Some(meta.clone());
    }
    if cx.schemas.dropped.contains(table) {
        cx.error(format!("table {table} was dropped earlier in this script"));
        return None;
    }
    if let Some(meta) = cx.session.engine.cached_definition(table) {
        return Some(meta);
    }
    cx.warning(format!(
        "table {table} has no cached definition, so key and index checks were skipped"
    ));
    None
}

fn plan_message(err: PlanError) -> String {
    match err {
        PlanError::SelectScanRejected => {
            "Cannot perform SELECT without an indexed WHERE clause. Use SCAN or specify USING -"
                .to_string()
        }
        PlanError::AmbiguousIndex(names) => {
            format!("Ambiguous index selection: {}", names.join(", "))
        }
        PlanError::CannotScanLocalIndex(name) => format!("Cannot scan local index '{name}'"),
        PlanError::UnknownIndex(name) => format!("Unknown index '{name}'"),
    }
}

fn push_issue(
    issues: &mut Vec<LintIssue>,
    full_text: &str,
    severity: &str,
    message: String,
    offset: usize,
    end: usize,
) {
    let end = end.max(offset.saturating_add(1));
    let (line, column) = if full_text.is_empty() {
        (1, 1)
    } else {
        line_column(full_text, offset)
    };
    issues.push(LintIssue {
        severity: severity.to_string(),
        message,
        offset,
        end,
        line,
        column,
    });
}

fn line_column(text: &str, utf16_at: usize) -> (usize, usize) {
    let mut seen = 0usize;
    let mut line = 1usize;
    let mut column = 1usize;
    for ch in text.chars() {
        if seen >= utf16_at {
            break;
        }
        let width = ch.len_utf16();
        if seen + width > utf16_at {
            break;
        }
        seen += width;
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn utf16_index(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[..byte].chars().map(char::len_utf16).sum()
}

fn statement_spans(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut buf_start_utf16 = 0usize;
    let mut buf_start_byte = 0usize;
    let mut utf16 = 0usize;
    let mut quote: Option<char> = None;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let (char_byte, ch) = chars[index];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        if quote.is_none() && ch == '-' && next == Some('-') {
            utf16 += ch.len_utf16();
            index += 1;
            if let Some((_, next_ch)) = chars.get(index) {
                utf16 += next_ch.len_utf16();
            }
            index += 1;
            while index < chars.len() && chars[index].1 != '\n' {
                utf16 += chars[index].1.len_utf16();
                index += 1;
            }
            continue;
        }
        if let Some(open) = quote {
            utf16 += ch.len_utf16();
            if ch == open {
                let escaped = index > 0 && chars[index - 1].1 == '\\';
                if !escaped {
                    quote = None;
                }
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            utf16 += ch.len_utf16();
            index += 1;
            continue;
        }
        if ch == ';' {
            let end_utf16 = utf16 + ch.len_utf16();
            let end_byte = char_byte + ch.len_utf8();
            push_span(
                text,
                &mut spans,
                buf_start_byte,
                end_byte,
                buf_start_utf16,
                end_utf16,
                true,
            );
            buf_start_utf16 = end_utf16;
            buf_start_byte = end_byte;
            utf16 = end_utf16;
            index += 1;
            continue;
        }
        utf16 += ch.len_utf16();
        index += 1;
    }
    let end_byte = text.len();
    let end_utf16 = utf16_len(text);
    push_span(
        text,
        &mut spans,
        buf_start_byte,
        end_byte,
        buf_start_utf16,
        end_utf16,
        false,
    );
    spans
}

fn push_span(
    text: &str,
    spans: &mut Vec<Span>,
    from_byte: usize,
    to_byte: usize,
    from_utf16: usize,
    to_utf16: usize,
    closed: bool,
) {
    let raw = text.get(from_byte..to_byte).unwrap_or("");
    if !has_code(raw) {
        return;
    }
    let lead_chars = raw.chars().take_while(|ch| ch.is_whitespace()).count();
    let lead_utf16: usize = raw
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf16)
        .sum();
    let lead_bytes: usize = raw.chars().take(lead_chars).map(char::len_utf8).sum();
    let trimmed_end = raw.trim_end();
    let end_bytes = from_byte + trimmed_end.len();
    let trail_utf16 = utf16_len(&raw[trimmed_end.len()..]);
    let from = from_utf16 + lead_utf16;
    let to = to_utf16.saturating_sub(trail_utf16);
    let statement = text
        .get(from_byte + lead_bytes..end_bytes)
        .unwrap_or("")
        .to_string();
    if statement.is_empty() {
        return;
    }
    spans.push(Span {
        from,
        to: to.max(from),
        text: statement,
        closed,
    });
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

fn has_code(statement: &str) -> bool {
    statement.lines().any(|line| {
        let stripped = line.trim();
        !stripped.is_empty() && !stripped.starts_with("--")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CliConfig;
    use crate::history::HistoryManager;
    use crate::session::RuntimeEngine;
    use crate::throttle::TableLimits;

    fn session(allow_select_scan: bool) -> Session {
        let mut config = CliConfig::load();
        config.allow_select_scan = allow_select_scan;
        Session {
            config,
            engine: RuntimeEngine::in_memory(allow_select_scan),
            local_endpoint: None,
            region: "us-west-1".to_string(),
            history: HistoryManager::new(),
            throttle: TableLimits::default(),
            table_disk: None,
        }
    }

    fn messages(report: &LintReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect()
    }

    #[test]
    fn parse_error_is_an_error_and_an_open_tail_is_a_warning() {
        let mut session = session(false);
        let closed = lint_script(&mut session, "SELEKT * FROM shelf;\n");
        assert!(!closed.ok);
        assert_eq!(closed.issues[0].severity, "error");
        assert_eq!(closed.issues[0].line, 1);

        let open = lint_script(&mut session, "SELEKT * FROM shelf");
        assert!(open.ok);
        assert_eq!(open.issues[0].severity, "warning");
    }

    #[test]
    fn create_table_checks_the_following_select_and_insert() {
        let mut session = session(false);
        let script = "\
CREATE TABLE shelf (id STRING HASH KEY, sk STRING RANGE KEY);
SELECT * FROM shelf;
INSERT INTO shelf (sk) VALUES ('a');
SELECT * FROM shelf WHERE id = 'a';
";
        let report = lint_script(&mut session, script);
        assert!(!report.ok);
        let text = messages(&report).join("\n");
        assert!(text.contains("indexed WHERE"), "{text}");
        assert!(text.contains("missing hash key 'id'"), "{text}");
        assert_eq!(
            report
                .issues
                .iter()
                .filter(|issue| issue.severity == "error")
                .count(),
            2,
            "{text}"
        );
    }

    #[test]
    fn select_scan_is_allowed_when_the_session_allows_it() {
        let mut session = session(true);
        let report = lint_script(
            &mut session,
            "CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf;\n",
        );
        assert!(report.ok, "{:?}", report.issues);
        assert!(report.issues.is_empty(), "{:?}", report.issues);
    }

    #[test]
    fn unknown_index_and_keys_in_conflict_are_errors() {
        let mut session = session(false);
        let script = "\
CREATE TABLE shelf (id STRING HASH KEY);
SELECT * FROM shelf WHERE id = 'a' USING missing;
SELECT * FROM shelf KEYS IN 'b' WHERE id = 'a';
";
        let report = lint_script(&mut session, script);
        assert!(!report.ok);
        let text = messages(&report).join("\n");
        assert!(text.contains("Unknown index 'missing'"), "{text}");
        assert!(text.contains("Cannot use WHERE with KEYS IN"), "{text}");
    }

    #[test]
    fn a_table_without_a_definition_is_a_warning() {
        let mut session = session(false);
        let report = lint_script(&mut session, "SELECT * FROM shelf WHERE id = 'a';\n");
        assert!(report.ok, "{:?}", report.issues);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, "warning");
        assert!(report.issues[0].message.contains("no cached definition"));
    }

    #[test]
    fn cached_definition_is_used_when_the_script_does_not_create_the_table() {
        let mut session = session(false);
        session
            .engine
            .execute_script("CREATE TABLE shelf (id STRING HASH KEY);")
            .unwrap();
        let report = lint_script(&mut session, "SELECT * FROM shelf;\n");
        assert!(!report.ok);
        assert!(messages(&report).join("\n").contains("indexed WHERE"));
    }

    #[test]
    fn lint_does_not_run_the_script() {
        let mut session = session(false);
        session
            .engine
            .execute_script(
                "CREATE TABLE shelf (id STRING HASH KEY);\nINSERT INTO shelf (id, title) VALUES ('a', 'kept');\n",
            )
            .unwrap();
        let before = session
            .engine
            .execute_script("SELECT * FROM shelf WHERE id = 'a';")
            .unwrap();
        let tables = session.engine.list_tables().unwrap();
        let count = session.engine.table_item_count("shelf");
        let capacity = session.engine.statement_capacity();
        let context = session
            .engine
            .last_query_context()
            .map(|ctx| (ctx.table.name.clone(), ctx.explicit_projection));

        let script = "\
CREATE TABLE other (id STRING HASH KEY);
INSERT INTO shelf (id, title) VALUES ('b', 'new');
UPDATE shelf SET title = 'changed' WHERE id = 'a';
DELETE FROM shelf WHERE id = 'a';
SELECT title FROM shelf WHERE id = 'a';
SELECT * FROM shelf;
SCAN * FROM shelf;
DROP TABLE shelf;
DROP TABLE other;
";
        let report = lint_script(&mut session, script);
        assert!(!report.ok, "{:?}", report.issues);
        assert!(
            messages(&report).join("\n").contains("indexed WHERE"),
            "{:?}",
            report.issues
        );

        assert_eq!(session.engine.list_tables().unwrap(), tables);
        assert_eq!(session.engine.table_item_count("shelf"), count);
        assert_eq!(session.engine.table_item_count("other"), 0);
        assert_eq!(session.engine.statement_capacity(), capacity);
        assert_eq!(
            session
                .engine
                .last_query_context()
                .map(|ctx| (ctx.table.name.clone(), ctx.explicit_projection)),
            context
        );
        let after = session
            .engine
            .execute_script("SELECT * FROM shelf WHERE id = 'a';")
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn comment_before_a_statement_does_not_move_the_error_onto_the_comment() {
        let mut session = session(false);
        let report = lint_script(&mut session, "-- header\nSELEKT * FROM shelf;\n");
        assert_eq!(report.issues[0].line, 2);
        assert_eq!(report.issues[0].column, 1);
    }
}
