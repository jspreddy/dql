//! Check a DQL script without running it.
//!
//! Parse errors are reported at the statement. When a table definition is
//! available from an earlier `CREATE TABLE` in the script, or from the cached
//! description, reads and writes are checked the same way execution would
//! reject them. A table with no definition is a warning, not a fetch.

use std::collections::{HashMap, HashSet};

use dql_expr::render_condition;
use dql_models::{plan_read, PlanError, PlanInput, ReadKind, TableMeta};
use dql_parser::{
    parse_statement, Condition, ConditionOperand, EquiJoin, InsertForm, QueryOptions, Selection,
    Statement,
};
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
    /// Local and temp tables created earlier in the script. Their columns come from a file
    /// or a query, so they have no definition yet, but they exist.
    created_virtual: HashSet<String>,
    dropped: HashSet<String>,
    /// `WITH` names for the statement being checked, with their columns when the query
    /// names them. A CTE alias is not a temp table, so a dropped temp table of the same
    /// name does not apply.
    ctes: HashMap<String, Option<Vec<String>>>,
    /// Names of the `WITH` being checked that are not stored yet: the query being checked
    /// and the ones after it. The engine runs them in order.
    pending_ctes: HashSet<String>,
    /// The `WITH` query being checked, innermost last.
    cte_stack: Vec<String>,
    /// `WITH` names from earlier statements. They last for one statement.
    ended_ctes: HashSet<String>,
    /// Columns of temp tables created earlier in the script, when their query names them.
    virtual_columns: HashMap<String, Vec<String>>,
    /// `WITH` names a query has read, so an unused one can be reported.
    cte_reads: HashSet<String>,
}

pub fn lint_script(session: &mut Session, text: &str) -> LintReport {
    let allow_select_scan = session.config.allow_select_scan;
    let spans = statement_spans(text);
    let mut schemas = Schemas {
        local: HashMap::new(),
        created_virtual: HashSet::new(),
        dropped: HashSet::new(),
        ctes: HashMap::new(),
        pending_ctes: HashSet::new(),
        cte_stack: Vec::new(),
        ended_ctes: HashSet::new(),
        virtual_columns: HashMap::new(),
        cte_reads: HashSet::new(),
    };
    let mut issues = Vec::new();
    for span in &spans {
        match parse_statement(&span.text) {
            Ok(statement) => {
                let mut cx = Cx {
                    source: text,
                    span,
                    region: (0, span.text.len()),
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
    /// Byte range of `span.text` that the statement being checked came from. A `WITH` query
    /// narrows it, so an issue points at the word in that query rather than another one.
    region: (usize, usize),
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
                    cx.schemas.ended_ctes.remove(&meta.name);
                    cx.schemas.local.insert(meta.name.clone(), meta);
                }
            }
            Err(err) => cx.error(err.to_string()),
        },
        Statement::CreateLocalTable { name, .. } => {
            if apply_schema {
                cx.schemas.dropped.remove(name);
                cx.schemas.ended_ctes.remove(name);
                cx.schemas.created_virtual.insert(name.clone());
            }
        }
        Statement::CreateTempTable { name, query, .. } => {
            check_statement(query, cx, false);
            if apply_schema {
                let columns = query_columns(cx, query);
                cx.schemas.dropped.remove(name);
                cx.schemas.ended_ctes.remove(name);
                cx.schemas.created_virtual.insert(name.clone());
                match columns {
                    Some(columns) => cx.schemas.virtual_columns.insert(name.clone(), columns),
                    None => cx.schemas.virtual_columns.remove(name),
                };
            }
        }
        Statement::DropTable { name, .. }
        | Statement::DropLocalTable { name, .. }
        | Statement::DropTempTable { name, .. } => {
            if apply_schema {
                cx.schemas.local.remove(name);
                cx.schemas.created_virtual.remove(name);
                cx.schemas.virtual_columns.remove(name);
                cx.schemas.dropped.insert(name.clone());
            }
        }
        Statement::Explain(inner) | Statement::Analyze(inner) => {
            check_statement(inner, cx, false);
        }
        Statement::With { ctes, body } => {
            // The same checks as the engine's `run_with`, in the same order: it stops at the
            // first name that clashes, before that query or the body runs. Each query reads
            // only the names before it, and every name ends with the statement.
            let region = cx.region;
            let new_pending: Vec<String> = ctes
                .iter()
                .map(|cte| cte.name.clone())
                .filter(|name| cx.schemas.pending_ctes.insert(name.clone()))
                .collect();
            let mut added: Vec<String> = Vec::new();
            let mut clash = None;
            let mut cursor = region.0;
            for cte in ctes {
                if added.contains(&cte.name) {
                    clash = Some(format!("WITH name '{}' is used more than once", cte.name));
                    break;
                }
                if cx.schemas.ctes.contains_key(&cte.name) || table_exists(cx, &cte.name) {
                    clash = Some(format!("Table '{}' already exists", cte.name));
                    break;
                }
                let query_region = cx.find_text(&cte.query_text, cursor);
                if let Some((_, end)) = query_region {
                    cursor = end;
                }
                cx.region = query_region.unwrap_or(region);
                cx.schemas.cte_stack.push(cte.name.clone());
                if matches!(
                    cte.query.as_ref(),
                    Statement::Select {
                        selection: Selection::CountAll,
                        ..
                    } | Statement::Scan {
                        selection: Selection::CountAll,
                        ..
                    }
                ) {
                    let at = cx.locate_word("COUNT", &[]);
                    cx.error_at(
                        at,
                        format!(
                            "WITH query '{}' uses COUNT(*), but a WITH query must return rows",
                            cte.name
                        ),
                    );
                } else {
                    check_statement(&cte.query, cx, false);
                }
                cx.schemas.cte_stack.pop();
                let columns = query_columns(cx, &cte.query);
                cx.schemas.pending_ctes.remove(&cte.name);
                cx.schemas.ctes.insert(cte.name.clone(), columns);
                added.push(cte.name.clone());
            }
            cx.region = (cursor.min(region.1), region.1);
            match clash {
                Some(message) => {
                    let at =
                        clash_name(&message).and_then(|name| cx.locate_word(name, &["WITH", ","]));
                    cx.error_at(at, message);
                }
                None => {
                    check_statement(body, cx, false);
                    cx.region = region;
                    for name in &added {
                        if !cx.schemas.cte_reads.contains(name) {
                            let at = cx.locate_definition(name);
                            cx.warning_at(
                                at,
                                format!(
                                    "WITH name '{name}' is never read, but its query still runs"
                                ),
                            );
                        }
                    }
                }
            }
            cx.region = region;
            for name in new_pending {
                cx.schemas.pending_ctes.remove(&name);
            }
            for name in added {
                cx.schemas.ctes.remove(&name);
                cx.schemas.cte_reads.remove(&name);
                if apply_schema {
                    cx.schemas.ended_ctes.insert(name);
                }
            }
        }
        Statement::Insert { table, form, .. } => check_insert(cx, table, form),
        Statement::Select {
            table,
            selection,
            condition,
            options,
            joins,
        }
        | Statement::Scan {
            table,
            selection,
            condition,
            options,
            joins,
        } if !joins.is_empty() => {
            check_join(cx, table, selection, condition.as_ref(), options, joins);
        }
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
    /// Report at a byte range of the statement text, or over the whole statement.
    fn issue_at(&mut self, severity: &str, at: Option<(usize, usize)>, message: String) {
        let (from, to) = match at {
            Some((start, end)) => (
                self.span.from + utf16_index(&self.span.text, start),
                self.span.from + utf16_index(&self.span.text, end),
            ),
            None => (self.span.from, self.span.to),
        };
        push_issue(self.issues, self.source, severity, message, from, to);
    }

    fn error_at(&mut self, at: Option<(usize, usize)>, message: String) {
        self.issue_at("error", at, message);
    }

    fn warning_at(&mut self, at: Option<(usize, usize)>, message: String) {
        self.issue_at("warning", at, message);
    }

    fn region_tokens(&self) -> Vec<(usize, usize, &str)> {
        let (start, end) = self.region;
        tokens(&self.span.text)
            .into_iter()
            .filter(|(from, to, _)| *from >= start && *to <= end)
            .collect()
    }

    /// The first `word` in the region, after one of `after` when given.
    fn locate_word(&self, word: &str, after: &[&str]) -> Option<(usize, usize)> {
        let list = self.region_tokens();
        let found = |exact: bool| {
            list.iter().enumerate().find(|(index, (_, _, text))| {
                let same = if exact {
                    *text == word
                } else {
                    text.eq_ignore_ascii_case(word)
                };
                same && (after.is_empty()
                    || index.checked_sub(1).is_some_and(|prev| {
                        after
                            .iter()
                            .any(|keyword| list[prev].2.eq_ignore_ascii_case(keyword))
                    }))
            })
        };
        found(true)
            .or_else(|| found(false))
            .map(|(_, (from, to, _))| (*from, *to))
    }

    /// A table name after FROM or JOIN.
    fn locate_table(&self, table: &str) -> Option<(usize, usize)> {
        self.locate_word(table, &["FROM", "JOIN"])
    }

    /// The name in `name AS (`.
    fn locate_definition(&self, name: &str) -> Option<(usize, usize)> {
        let list = self.region_tokens();
        list.iter()
            .enumerate()
            .find(|(index, (_, _, text))| {
                *text == name
                    && list
                        .get(index + 1)
                        .is_some_and(|next| next.2.eq_ignore_ascii_case("AS"))
            })
            .map(|(_, (from, to, _))| (*from, *to))
    }

    /// A column in the ON clause of `JOIN join_table`, written `qualifier.column` or `column`.
    fn locate_on_column(
        &self,
        join_table: &str,
        qualifier: &str,
        column: &str,
    ) -> Option<(usize, usize)> {
        let list = self.region_tokens();
        let start = list.iter().enumerate().position(|(index, (_, _, text))| {
            *text == join_table
                && index
                    .checked_sub(1)
                    .is_some_and(|prev| list[prev].2.eq_ignore_ascii_case("JOIN"))
        })?;
        let qualified = format!("{qualifier}.{column}");
        list[start..]
            .iter()
            .take_while(|(_, _, text)| !text.eq_ignore_ascii_case("WHERE"))
            .skip(1)
            .find(|(_, _, text)| {
                (!qualifier.is_empty() && *text == qualified)
                    || (*text == column)
                    || (qualifier.is_empty() && text.ends_with(&format!(".{column}")))
            })
            .map(|(from, to, _)| (*from, *to))
    }

    /// Byte range of `needle` in the statement text, at or after `from`.
    fn find_text(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        let needle = needle.trim();
        if needle.is_empty() {
            return None;
        }
        let start = from + self.span.text.get(from..)?.find(needle)?;
        Some((start, start + needle.len()))
    }

    fn error(&mut self, message: String) {
        let from = self.span.from;
        let to = self.span.to;
        push_issue(self.issues, self.source, "error", message, from, to);
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

/// Whether `name` is a table the engine would find, as far as the script and the cached
/// definitions tell. A DynamoDB table that was never described is not known here.
fn table_exists(cx: &mut Cx<'_>, name: &str) -> bool {
    if cx.schemas.dropped.contains(name) {
        return false;
    }
    cx.schemas.local.contains_key(name)
        || cx.schemas.created_virtual.contains(name)
        || cx.session.engine.cached_definition(name).is_some()
}

fn schema_for(cx: &mut Cx<'_>, table: &str) -> Option<TableMeta> {
    if cx.schemas.ctes.contains_key(table) {
        cx.schemas.cte_reads.insert(table.to_string());
        return None;
    }
    if let Some(message) = unavailable_cte(cx, table) {
        let at = cx.locate_table(table);
        cx.error_at(at, message);
        return None;
    }
    if let Some(meta) = cx.schemas.local.get(table) {
        return Some(meta.clone());
    }
    if cx.schemas.dropped.contains(table) {
        let at = cx.locate_table(table);
        cx.error_at(
            at,
            format!("table {table} was dropped earlier in this script"),
        );
        return None;
    }
    if let Some(meta) = cx.session.engine.cached_definition(table) {
        return Some(meta);
    }
    if cx.schemas.created_virtual.contains(table) {
        return None;
    }
    let at = cx.locate_table(table);
    let hint = suggestion(cx, table);
    cx.warning_at(
        at,
        format!(
            "table {table} has no cached definition, so key and index checks were skipped{hint}"
        ),
    );
    None
}

/// A `WITH` name that a query cannot read yet, or any more.
fn unavailable_cte(cx: &mut Cx<'_>, table: &str) -> Option<String> {
    if cx.schemas.pending_ctes.contains(table) {
        // The query meant this name, so it is not also reported as never read.
        cx.schemas.cte_reads.insert(table.to_string());
        let current = cx.schemas.cte_stack.last().map(String::as_str);
        return Some(match current {
            Some(name) if name == table => format!(
                "WITH query '{table}' reads itself. A WITH query reads only the WITH names before it"
            ),
            Some(name) => format!(
                "WITH query '{name}' reads '{table}', which is defined after it. \
                 A WITH query reads only the WITH names before it"
            ),
            None => format!("'{table}' is not stored yet"),
        });
    }
    if cx.schemas.ended_ctes.contains(table) {
        return Some(format!(
            "'{table}' is a WITH name from an earlier statement. A WITH name lasts for one \
             statement; use CREATE TEMPTABLE {table} AS (…) to keep its rows"
        ));
    }
    None
}

/// Where a joined row's columns can come from, and whether its columns are known.
enum JoinSource {
    Memory(Option<Vec<String>>),
    Missing(String),
}

fn join_source(cx: &mut Cx<'_>, table: &str) -> JoinSource {
    const LOAD: &str = "Load it with CREATE TEMPTABLE or a WITH clause first";
    if let Some(columns) = cx.schemas.ctes.get(table) {
        let columns = columns.clone();
        cx.schemas.cte_reads.insert(table.to_string());
        return JoinSource::Memory(columns);
    }
    if let Some(message) = unavailable_cte(cx, table) {
        return JoinSource::Missing(message);
    }
    if cx.schemas.dropped.contains(table) {
        return JoinSource::Missing(format!("table {table} was dropped earlier in this script"));
    }
    if cx.schemas.created_virtual.contains(table) {
        return JoinSource::Memory(cx.schemas.virtual_columns.get(table).cloned());
    }
    if cx.schemas.local.contains_key(table) {
        return JoinSource::Missing(format!(
            "JOIN reads '{table}' from memory, but it is a DynamoDB table created earlier in \
             this script. {LOAD}"
        ));
    }
    if cx.session.engine.virtual_schema_line(table).is_some() {
        // A stored temp or local table: its rows decided its columns, so the list is whole.
        let columns = cx
            .session
            .engine
            .cached_definition(table)
            .map(|meta| meta.attrs.keys().cloned().collect::<Vec<_>>())
            .filter(|columns| !columns.is_empty());
        return JoinSource::Memory(columns);
    }
    if cx.session.engine.cached_definition(table).is_some() {
        return JoinSource::Missing(format!(
            "JOIN reads '{table}' from memory, but it is a DynamoDB table. {LOAD}"
        ));
    }
    let hint = suggestion(cx, table);
    JoinSource::Missing(format!(
        "JOIN reads '{table}' from memory, but it is not a WITH name, temp table, or local \
         table. {LOAD}{hint}"
    ))
}

/// The same rules as the engine's `join_reads`, plus warnings for columns that cannot match.
fn check_join(
    cx: &mut Cx<'_>,
    table: &str,
    selection: &Selection,
    condition: Option<&Condition>,
    options: &QueryOptions,
    joins: &[EquiJoin],
) {
    if matches!(selection, Selection::CountAll) {
        let at = cx.locate_word("COUNT", &[]);
        cx.error_at(at, "JOIN does not support COUNT(*)".to_string());
        return;
    }
    if options.keys_in.is_some() || options.using_index.is_some() {
        let word = if options.keys_in.is_some() {
            "KEYS"
        } else {
            "USING"
        };
        let at = cx.locate_word(word, &[]);
        cx.error_at(
            at,
            "JOIN reads in-memory rows and does not use KEYS IN or USING".to_string(),
        );
        return;
    }
    if let Some(condition) = condition {
        if let Err(err) = render_condition(condition) {
            cx.error(err.to_string());
            return;
        }
    }
    let names: Vec<&str> = std::iter::once(table)
        .chain(joins.iter().map(|join| join.table.as_str()))
        .collect();
    let mut columns: HashMap<String, Option<Vec<String>>> = HashMap::new();
    let mut missing = false;
    for name in &names {
        match join_source(cx, name) {
            JoinSource::Memory(known) => {
                columns.insert((*name).to_string(), known);
            }
            JoinSource::Missing(message) => {
                missing = true;
                let at = cx.locate_table(name);
                cx.error_at(at, message);
            }
        }
    }
    if missing {
        return;
    }
    let known = |name: &str| columns.get(name).cloned().flatten();
    for (index, join) in joins.iter().enumerate() {
        let earlier = &names[..=index];
        if let Some(right) = known(&join.table) {
            if !right.contains(&join.right_attr) {
                let at = cx.locate_on_column(&join.table, &join.table, &join.right_attr);
                cx.warning_at(
                    at,
                    no_column(
                        &join.table,
                        &join.right_attr,
                        &right,
                        "this JOIN matches no rows",
                    ),
                );
            }
        }
        match &join.left_table {
            Some(left) => {
                if let Some(left_columns) = known(left) {
                    if !left_columns.contains(&join.left_attr) {
                        let at = cx.locate_on_column(&join.table, left, &join.left_attr);
                        cx.warning_at(
                            at,
                            no_column(
                                left,
                                &join.left_attr,
                                &left_columns,
                                "this JOIN matches no rows",
                            ),
                        );
                    }
                }
            }
            None => {
                let lists: Option<Vec<Vec<String>>> =
                    earlier.iter().map(|name| known(name)).collect();
                if let Some(lists) = lists {
                    if !lists.iter().any(|list| list.contains(&join.left_attr)) {
                        let all: Vec<String> = lists.concat();
                        let at = cx.locate_on_column(&join.table, "", &join.left_attr);
                        cx.warning_at(
                            at,
                            format!(
                                "no table before {} has column '{}', so this JOIN matches no rows{}",
                                join.table,
                                join.left_attr,
                                closest_hint(&join.left_attr, &all)
                            ),
                        );
                    }
                }
            }
        }
    }
    if let Some(condition) = condition {
        let mut fields = Vec::new();
        condition_fields(condition, &mut fields);
        let from_columns = known(table);
        for field in fields {
            let joined = joins.iter().map(|join| join.table.as_str()).find(|name| {
                field
                    .split_once('.')
                    .is_some_and(|(qualifier, _)| qualifier == *name)
            });
            let from_has = from_columns.as_ref().map(|list| {
                let root = field.split(['.', '[']).next().unwrap_or(&field);
                list.iter().any(|column| column == root)
            });
            if let Some(other) = joined {
                if from_has != Some(true) {
                    let at = cx.locate_word(&field, &[]);
                    cx.warning_at(
                        at,
                        format!(
                            "WHERE filters {table} before the join, so {field} never matches. \
                             Filter {other} in a WITH query instead"
                        ),
                    );
                }
                continue;
            }
            if from_has == Some(false) {
                let owner = joins
                    .iter()
                    .map(|join| join.table.as_str())
                    .find(|name| known(name).is_some_and(|list| list.contains(&field)));
                let at = cx.locate_word(&field, &[]);
                let message = match owner {
                    Some(owner) => format!(
                        "WHERE filters {table} before the join, and '{field}' is a column of \
                         {owner}, not {table}. Filter {owner} in a WITH query instead"
                    ),
                    None => no_column(
                        table,
                        &field,
                        &from_columns.clone().unwrap_or_default(),
                        "WHERE matches no rows",
                    ),
                };
                cx.warning_at(at, message);
            }
        }
    }
    if let Selection::Items(items) = selection {
        for item in items {
            let Some((qualifier, column)) = item.expression.trim().split_once('.') else {
                continue;
            };
            if column.contains(['.', '[']) {
                continue;
            }
            let Some(position) = names.iter().position(|name| *name == qualifier) else {
                continue;
            };
            let earlier: Option<Vec<Vec<String>>> =
                names[..position].iter().map(|name| known(name)).collect();
            let kept_plain = if position == 0 {
                // The first table's columns are never renamed.
                known(table).is_none_or(|list| !list.contains(&qualifier.to_string()))
            } else {
                earlier
                    .is_some_and(|lists| !lists.iter().any(|list| list.iter().any(|c| c == column)))
                    && known(qualifier).is_some_and(|list| list.iter().any(|c| c == column))
            };
            if kept_plain {
                let at = cx.locate_word(item.expression.trim(), &[]);
                cx.warning_at(
                    at,
                    format!(
                        "joined rows keep '{column}' unqualified, so {qualifier}.{column} is \
                         always null. Select {column}"
                    ),
                );
            }
        }
    }
}

fn no_column(table: &str, column: &str, columns: &[String], effect: &str) -> String {
    let mut sorted = columns.to_vec();
    sorted.sort();
    format!(
        "{table} has no column '{column}', so {effect}. {table} has: {}{}",
        sorted.join(", "),
        closest_hint(column, columns)
    )
}

fn closest_hint(word: &str, candidates: &[String]) -> String {
    closest(word, candidates.iter().map(String::as_str))
        .map(|name| format!(". Did you mean '{name}'?"))
        .unwrap_or_default()
}

/// A near name the script or the session knows, for a table that is not found.
fn suggestion(cx: &mut Cx<'_>, table: &str) -> String {
    let mut candidates: Vec<String> = cx.schemas.ctes.keys().cloned().collect();
    candidates.extend(cx.schemas.created_virtual.iter().cloned());
    candidates.extend(cx.schemas.local.keys().cloned());
    candidates.extend(cx.session.engine.virtual_table_names());
    candidates.retain(|name| !cx.schemas.dropped.contains(name));
    closest(table, candidates.iter().map(String::as_str))
        .map(|name| format!(". Did you mean '{name}'?"))
        .unwrap_or_default()
}

fn closest<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (word.chars().count() / 3).max(1);
    candidates
        .filter(|candidate| *candidate != word)
        .map(|candidate| {
            (
                edit_distance(&word.to_lowercase(), &candidate.to_lowercase()),
                candidate,
            )
        })
        .filter(|(distance, _)| *distance <= limit)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, candidate)| candidate)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(previous + 1);
        }
    }
    row[b.len()]
}

fn condition_fields(condition: &Condition, out: &mut Vec<String>) {
    match condition {
        Condition::Compare { field, rhs, .. } => {
            out.push(field.clone());
            if let ConditionOperand::Field(other) = rhs {
                out.push(other.clone());
            }
        }
        Condition::Between { field, .. }
        | Condition::In { field, .. }
        | Condition::Size { field, .. }
        | Condition::AttributeType { field, .. } => out.push(field.clone()),
        Condition::InTuples { fields, .. } => out.extend(fields.iter().cloned()),
        Condition::Function { args, .. } => {
            for arg in args {
                if let ConditionOperand::Field(field) = arg {
                    out.push(field.clone());
                }
            }
        }
        Condition::And(parts) | Condition::Or(parts) => {
            for part in parts {
                condition_fields(part, out);
            }
        }
        Condition::Not(inner) => condition_fields(inner, out),
    }
}

/// The columns a query's rows have, when the query or its source names all of them.
fn query_columns(cx: &mut Cx<'_>, query: &Statement) -> Option<Vec<String>> {
    let (table, selection, joins) = match query {
        Statement::Select {
            table,
            selection,
            joins,
            ..
        }
        | Statement::Scan {
            table,
            selection,
            joins,
            ..
        } => (table, selection, joins),
        _ => return None,
    };
    match selection {
        Selection::Items(items) => items
            .iter()
            .map(|item| {
                let name = item
                    .alias
                    .clone()
                    .unwrap_or_else(|| item.expression.trim().to_string());
                name.chars()
                    .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
                    .then_some(name)
            })
            .collect(),
        Selection::All if joins.is_empty() => {
            if let Some(columns) = cx.schemas.ctes.get(table) {
                return columns.clone();
            }
            if cx.schemas.created_virtual.contains(table) {
                return cx.schemas.virtual_columns.get(table).cloned();
            }
            if cx.schemas.local.contains_key(table) || cx.schemas.dropped.contains(table) {
                return None;
            }
            if cx.session.engine.virtual_schema_line(table).is_some() {
                return cx
                    .session
                    .engine
                    .cached_definition(table)
                    .map(|meta| meta.attrs.keys().cloned().collect::<Vec<_>>())
                    .filter(|columns| !columns.is_empty());
            }
            None
        }
        _ => None,
    }
}

/// The table name inside the quotes of a clash message.
fn clash_name(message: &str) -> Option<&str> {
    let start = message.find('\'')? + 1;
    let end = start + message[start..].find('\'')?;
    Some(&message[start..end])
}

/// Identifier and punctuation tokens of a statement, past quoted strings and `--` comments.
/// Each is (start byte, end byte, text).
fn tokens(text: &str) -> Vec<(usize, usize, &str)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let ch = bytes[at];
        if ch == b'-' && bytes.get(at + 1) == Some(&b'-') {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            continue;
        }
        if ch == b'\'' || ch == b'"' {
            at += 1;
            while at < bytes.len() && bytes[at] != ch {
                if bytes[at] == b'\\' {
                    at += 1;
                }
                at += 1;
            }
            at += 1;
            continue;
        }
        let ident = |byte: u8| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'.' | b'-' | b'[' | b']')
                || byte >= 0x80
        };
        if ident(ch) {
            let start = at;
            while at < bytes.len()
                && ident(bytes[at])
                && !(bytes[at] == b'-' && bytes.get(at + 1) == Some(&b'-'))
            {
                at += 1;
            }
            out.push((start, at, &text[start..at]));
            continue;
        }
        if !ch.is_ascii_whitespace() {
            out.push((at, at + 1, &text[at..at + 1]));
        }
        at += 1;
    }
    out
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
    let limit = utf16_len(full_text);
    let mut offset = offset.min(limit);
    let mut end = end.min(limit);
    if end <= offset {
        if offset == 0 {
            end = if limit == 0 { 0 } else { 1 };
        } else {
            offset -= 1;
            end = offset + 1;
        }
    }
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
    // A span runs from the previous `;` to this one, so it can start with comment lines that
    // belong above the statement, and an unclosed tail can end with comment lines. Issues
    // underline the whole span, so keep both out of it.
    let lead = code_start(raw);
    let parsed_end = raw.trim_end().len().max(lead);
    let shown_end = code_end(raw).max(lead);
    let statement = raw[lead..parsed_end].to_string();
    if statement.is_empty() {
        return;
    }
    let from = from_utf16 + utf16_len(&raw[..lead]);
    let to = from_utf16 + utf16_len(&raw[..shown_end]);
    debug_assert!(to <= to_utf16);
    spans.push(Span {
        from,
        to: to.max(from),
        text: statement,
        closed,
    });
}

/// Byte index of the first code character, past whitespace and `--` comment lines.
fn code_start(raw: &str) -> usize {
    let mut at = 0;
    loop {
        let rest = &raw[at..];
        let trimmed = rest.trim_start();
        at += rest.len() - trimmed.len();
        if !trimmed.starts_with("--") {
            return at;
        }
        match trimmed.find('\n') {
            Some(newline) => at += newline + 1,
            None => return raw.len(),
        }
    }
}

/// Byte index just past the last code character, before trailing whitespace and lines that
/// are only a `--` comment.
fn code_end(raw: &str) -> usize {
    let mut end = raw.trim_end().len();
    loop {
        let line_start = raw[..end].rfind('\n').map_or(0, |newline| newline + 1);
        if !raw[line_start..end].trim_start().starts_with("--") || line_start == 0 {
            return end;
        }
        end = raw[..line_start].trim_end().len();
    }
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
            workspace: std::env::temp_dir(),
            connection_key: "memory|us-west-1".to_string(),
            serves_mcp: false,
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
    fn issue_ranges_stay_inside_the_script() {
        let mut session = session(true);
        let samples = [
            "SCAN * FROM nb_posts;\n",
            "SCAN * FROM nb_posts",
            "SELEKT * FROM shelf",
            "SCAN * FROM",
            "SELECT",
            "SCAN * FROM nb_posts WHERE\n",
        ];
        for text in samples {
            let report = lint_script(&mut session, text);
            let limit = utf16_len(text);
            for issue in &report.issues {
                assert!(
                    issue.offset <= limit && issue.end <= limit,
                    "{text:?} offset {} end {} limit {limit}: {}",
                    issue.offset,
                    issue.end,
                    issue.message
                );
                if !text.is_empty() {
                    assert!(
                        issue.end > issue.offset,
                        "{text:?} empty range {}..{}: {}",
                        issue.offset,
                        issue.end,
                        issue.message
                    );
                }
            }
        }
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
    fn ls_is_a_statement() {
        let mut session = session(false);
        let report = lint_script(&mut session, "LS posts;\n");
        assert!(report.ok, "{:?}", report.issues);
        let bad = lint_script(&mut session, "LS bogus=True;\n");
        assert!(!bad.ok, "{:?}", bad.issues);
        assert!(
            messages(&bad).join("\n").contains("unknown ls option"),
            "{:?}",
            bad.issues
        );
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

    /// The underlined range of a statement-level issue, as text.
    fn underlined<'a>(script: &'a str, issue: &LintIssue) -> &'a str {
        let units: Vec<u16> = script.encode_utf16().collect();
        let start = String::from_utf16(&units[..issue.offset]).unwrap().len();
        let end = String::from_utf16(&units[..issue.end]).unwrap().len();
        &script[start..end]
    }

    #[test]
    fn issues_underline_the_statement_not_the_comments_above_it() {
        let mut session = session(false);
        let script = "\
CREATE TABLE posts (id STRING HASH KEY);
-- add one row
-- (two values for one column)
INSERT INTO posts (id) VALUES ('a', 'b');
";
        let report = lint_script(&mut session, script);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
        let issue = &report.issues[0];
        assert_eq!(
            underlined(script, issue),
            "INSERT INTO posts (id) VALUES ('a', 'b');"
        );
        assert_eq!((issue.line, issue.column), (4, 1));
    }

    #[test]
    fn a_comment_after_a_semicolon_is_not_underlined_with_the_next_statement() {
        let mut session = session(false);
        let script = "CREATE TABLE posts (id STRING HASH KEY); -- made it\nINSERT INTO posts (id) VALUES ('a', 'b');\n";
        let report = lint_script(&mut session, script);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
        assert_eq!(
            underlined(script, &report.issues[0]),
            "INSERT INTO posts (id) VALUES ('a', 'b');"
        );
    }

    #[test]
    fn trailing_comment_lines_of_an_unclosed_statement_are_not_underlined() {
        let mut session = session(false);
        let script = "CREATE TABLE posts (id STRING HASH KEY);\n-- lead\nINSERT INTO posts (id) VALUES ('a', 'b')\n-- tail note\n";
        let report = lint_script(&mut session, script);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
        assert_eq!(
            underlined(script, &report.issues[0]),
            "INSERT INTO posts (id) VALUES ('a', 'b')"
        );
    }

    #[test]
    fn a_parse_error_after_a_leading_comment_points_into_the_statement() {
        let mut session = session(false);
        let script = "-- broken one\nSELEC * FROM shelf;\n";
        let report = lint_script(&mut session, script);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
        let issue = &report.issues[0];
        assert_eq!(issue.severity, "error");
        assert_eq!(issue.line, 2, "{:?}", issue);
        assert!(
            !underlined(script, issue).contains("--"),
            "{:?}",
            underlined(script, issue)
        );
    }

    #[test]
    fn an_existing_local_table_uses_its_definition() {
        let mut session = session(false);
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("people.csv");
        std::fs::write(&csv, "id,name\na,ada\n").unwrap();
        session
            .engine
            .execute_script(&format!("CREATE LOCALTABLE people AS '{}';", csv.display()))
            .unwrap();
        let clean = lint_script(&mut session, "SCAN * FROM people;\n");
        assert!(clean.issues.is_empty(), "{:?}", clean.issues);
        let rejected = lint_script(&mut session, "SELECT * FROM people;\n");
        assert!(
            messages(&rejected).join("\n").contains("indexed WHERE"),
            "{:?}",
            rejected.issues
        );
    }

    #[test]
    fn local_and_temp_tables_created_in_the_script_are_known() {
        let mut session = session(false);
        let script = "\
CREATE TABLE posts (id STRING HASH KEY);
CREATE LOCALTABLE people AS './people.csv';
SCAN * FROM people;
CREATE TEMPTABLE hot AS (SCAN * FROM posts);
SCAN * FROM hot;
";
        let report = lint_script(&mut session, script);
        assert!(report.issues.is_empty(), "{:?}", report.issues);
    }

    #[test]
    fn a_temp_table_query_is_checked_and_a_dropped_one_is_an_error() {
        let mut session = session(false);
        let script = "\
CREATE TABLE posts (id STRING HASH KEY);
CREATE TEMPTABLE hot AS (SELECT * FROM posts);
DROP TEMPTABLE hot;
SCAN * FROM hot;
";
        let report = lint_script(&mut session, script);
        let text = messages(&report).join("\n");
        assert!(text.contains("indexed WHERE"), "{text}");
        assert!(
            text.contains("table hot was dropped earlier in this script"),
            "{text}"
        );
    }

    #[test]
    fn a_with_name_that_clashes_is_the_engine_error() {
        let mut session = session(false);
        let cases = [
            (
                "CREATE TABLE posts (id STRING HASH KEY);\n\
                 WITH posts AS (SCAN * FROM posts) SCAN * FROM posts;\n",
                "Table 'posts' already exists",
            ),
            (
                "CREATE TABLE posts (id STRING HASH KEY);\n\
                 CREATE TEMPTABLE hot AS (SCAN * FROM posts);\n\
                 WITH hot AS (SCAN * FROM posts) SCAN * FROM hot;\n",
                "Table 'hot' already exists",
            ),
            (
                "CREATE TABLE posts (id STRING HASH KEY);\n\
                 WITH a AS (SCAN * FROM posts), a AS (SCAN * FROM posts) SCAN * FROM a;\n",
                "WITH name 'a' is used more than once",
            ),
            (
                "CREATE TABLE posts (id STRING HASH KEY);\n\
                 WITH a AS (SCAN * FROM posts), \
                 b AS (WITH a AS (SCAN * FROM posts) SCAN * FROM a) SCAN * FROM b;\n",
                "Table 'a' already exists",
            ),
        ];
        for (script, expected) in cases {
            // The engine gives the same error when the script runs.
            let ran = self::session(false).engine.execute_script(script);
            let ran = format!("{ran:?}");
            assert!(ran.contains(expected), "{script}\n{ran}");
            let report = lint_script(&mut session, script);
            assert!(!report.ok, "{script}");
            let text = messages(&report).join("\n");
            assert!(text.contains(expected), "{script}\n{text}");
        }

        session
            .engine
            .execute_script("CREATE TABLE stored (id STRING HASH KEY)")
            .unwrap();
        let cached = lint_script(
            &mut session,
            "WITH stored AS (SCAN * FROM stored) SCAN * FROM stored;\n",
        );
        assert!(
            messages(&cached)
                .join("\n")
                .contains("Table 'stored' already exists"),
            "{:?}",
            cached.issues
        );

        let dropped = lint_script(
            &mut session,
            "DROP TABLE stored;\nCREATE TABLE posts (id STRING HASH KEY);\n\
             WITH stored AS (SCAN * FROM posts) SCAN * FROM stored;\n",
        );
        assert!(dropped.ok, "{:?}", dropped.issues);
    }

    #[test]
    fn a_with_name_is_not_a_dropped_temp_table() {
        let mut session = session(false);
        let script = "\
DROP TEMPTABLE IF EXISTS orders;
DROP TEMPTABLE IF EXISTS people;
CREATE TABLE posts (id STRING HASH KEY, customer STRING);
CREATE TABLE customers (id STRING HASH KEY, name STRING);
WITH orders AS (SCAN * FROM posts),
     people AS (SCAN * FROM customers)
SELECT name FROM orders
JOIN people ON orders.customer = people.id;
WITH orders AS (SCAN * FROM posts),
     people AS (SCAN * FROM customers)
SELECT name FROM orders
JOIN people ON people.id = orders.customer
WHERE id = 'b';
CREATE TEMPTABLE orders AS (SCAN * FROM posts);
SCAN name FROM orders;
";
        let report = lint_script(&mut session, script);
        assert!(report.ok, "{:?}", report.issues);
        assert!(report.issues.is_empty(), "{:?}", report.issues);

        let still_dropped = lint_script(
            &mut session,
            "DROP TEMPTABLE IF EXISTS orders;\nSCAN * FROM orders;\n",
        );
        assert!(
            messages(&still_dropped)
                .join("\n")
                .contains("table orders was dropped earlier in this script"),
            "{:?}",
            still_dropped.issues
        );
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

    // Joins and WITH queries. The fixture stores two temp tables in the engine, so their
    // columns are known: orders (id, customer) and people (id, name). posts and customers are
    // DynamoDB tables with cached definitions.
    const JOIN_FIXTURE: &str = "\
CREATE TABLE posts (id STRING HASH KEY, customer STRING);
INSERT INTO posts (id, customer) VALUES ('p1', 'c1');
CREATE TABLE customers (id STRING HASH KEY, name STRING);
INSERT INTO customers (id, name) VALUES ('c1', 'Ada');
CREATE TEMPTABLE orders AS (SCAN * FROM posts);
CREATE TEMPTABLE people AS (SCAN * FROM customers);";

    fn join_session() -> Session {
        let mut session = session(false);
        session.engine.execute_script(JOIN_FIXTURE).unwrap();
        session
    }

    /// Each issue as (severity, message, underlined text).
    fn found(session: &mut Session, script: &str) -> Vec<(String, String, String)> {
        lint_script(session, script)
            .issues
            .iter()
            .map(|issue| {
                (
                    issue.severity.clone(),
                    issue.message.clone(),
                    underlined(script, issue).to_string(),
                )
            })
            .collect()
    }

    /// The one issue a script has.
    fn only(session: &mut Session, script: &str) -> (String, String, String) {
        let mut issues = found(session, script);
        assert_eq!(issues.len(), 1, "{script}\n{issues:#?}");
        issues.remove(0)
    }

    /// What the engine says when the script runs in the fixture.
    fn engine_says(script: &str) -> String {
        let mut session = join_session();
        match session.engine.execute_script(script) {
            Ok(result) => format!("ok {result:?}"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn a_join_of_two_temp_tables_is_clean_even_as_select() {
        let mut session = join_session();
        let script = "SELECT name FROM orders JOIN people ON orders.customer = people.id;";
        assert!(found(&mut session, script).is_empty());
        assert!(engine_says(script).contains("Ada"));
    }

    #[test]
    fn joining_a_dynamodb_table_is_an_error_at_its_name() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN customers ON orders.customer = customers.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!(severity, "error");
        assert_eq!(at, "customers");
        assert_eq!(
            message,
            "JOIN reads 'customers' from memory, but it is a DynamoDB table. \
             Load it with CREATE TEMPTABLE or a WITH clause first"
        );
        assert!(engine_says(script).contains("JOIN reads 'customers' from memory"));
    }

    #[test]
    fn a_dynamodb_table_in_from_is_an_error_too() {
        let mut session = join_session();
        let script = "SCAN * FROM posts JOIN people ON posts.customer = people.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "posts"));
        assert!(
            message.contains("'posts'") && message.contains("DynamoDB table"),
            "{message}"
        );
        assert!(engine_says(script).contains("JOIN reads 'posts' from memory"));
    }

    #[test]
    fn joining_a_table_created_earlier_in_the_script_names_that() {
        let mut session = join_session();
        let script = "\
CREATE TABLE shelf (id STRING HASH KEY);
SCAN * FROM orders JOIN shelf ON orders.id = shelf.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "shelf"));
        assert!(
            message.contains("DynamoDB table created earlier in this script"),
            "{message}"
        );
    }

    #[test]
    fn joining_an_unknown_table_suggests_the_near_name() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN peple ON orders.customer = peple.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "peple"));
        assert_eq!(
            message,
            "JOIN reads 'peple' from memory, but it is not a WITH name, temp table, or local \
             table. Load it with CREATE TEMPTABLE or a WITH clause first. Did you mean 'people'?"
        );
        assert!(engine_says(script).contains("JOIN reads 'peple' from memory"));
    }

    #[test]
    fn every_table_that_is_not_in_memory_is_reported() {
        let mut session = join_session();
        let script =
            "SCAN * FROM posts JOIN people ON posts.customer = people.id JOIN nobody ON people.id = nobody.id;";
        let issues = found(&mut session, script);
        let at: Vec<&str> = issues.iter().map(|issue| issue.2.as_str()).collect();
        assert_eq!(at, ["posts", "nobody"], "{issues:#?}");
        assert!(issues.iter().all(|issue| issue.0 == "error"));
    }

    #[test]
    fn joining_a_temp_table_dropped_earlier_is_an_error() {
        let mut session = join_session();
        let script = "\
DROP TEMPTABLE people;
SCAN * FROM orders JOIN people ON orders.customer = people.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "people"));
        assert_eq!(message, "table people was dropped earlier in this script");
    }

    #[test]
    fn count_star_with_a_join_is_an_error_at_count() {
        let mut session = join_session();
        let script = "SCAN COUNT(*) FROM orders JOIN people ON orders.customer = people.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "COUNT"));
        assert_eq!(message, "JOIN does not support COUNT(*)");
        assert_eq!(engine_says(script), message);
    }

    #[test]
    fn using_or_keys_in_with_a_join_is_an_error() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN people ON orders.customer = people.id USING by_name;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "USING"));
        assert_eq!(
            message,
            "JOIN reads in-memory rows and does not use KEYS IN or USING"
        );
        assert_eq!(engine_says(script), message);
    }

    #[test]
    fn a_missing_column_on_the_joined_side_is_a_warning_with_the_columns() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN people ON orders.customer = people.idd;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "people.idd"));
        assert_eq!(
            message,
            "people has no column 'idd', so this JOIN matches no rows. people has: id, name. \
             Did you mean 'id'?"
        );
        assert_eq!(engine_says(script), "ok Items([])");
    }

    #[test]
    fn a_missing_column_on_the_earlier_side_is_a_warning() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN people ON orders.custmer = people.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!(
            (severity.as_str(), at.as_str()),
            ("warning", "orders.custmer")
        );
        assert!(
            message.starts_with("orders has no column 'custmer'"),
            "{message}"
        );
        assert!(message.ends_with("Did you mean 'customer'?"), "{message}");
    }

    #[test]
    fn an_unqualified_on_column_no_earlier_table_has_is_a_warning() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN people ON custmer = id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "custmer"));
        assert_eq!(
            message,
            "no table before people has column 'custmer', so this JOIN matches no rows. \
             Did you mean 'customer'?"
        );
        assert_eq!(engine_says(script), "ok Items([])");
    }

    #[test]
    fn an_on_qualifier_that_is_not_in_the_query_is_a_parse_error() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN people ON o.customer = people.id;";
        let (severity, message, _) = only(&mut session, script);
        assert_eq!(severity, "error");
        assert!(
            message.contains("ON must compare one new table column with orders.column"),
            "{message}"
        );
    }

    #[test]
    fn joining_the_same_table_twice_is_a_parse_error() {
        let mut session = join_session();
        let script = "SCAN * FROM orders JOIN orders ON orders.customer = orders.id;";
        let (severity, message, _) = only(&mut session, script);
        assert_eq!(severity, "error");
        assert!(
            message.contains("JOIN needs a different table"),
            "{message}"
        );
    }

    #[test]
    fn where_on_a_joined_table_is_a_warning() {
        let mut session = join_session();
        let script =
            "SCAN * FROM orders JOIN people ON orders.customer = people.id WHERE people.name = 'Ada';";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "people.name"));
        assert_eq!(
            message,
            "WHERE filters orders before the join, so people.name never matches. \
             Filter people in a WITH query instead"
        );
        assert_eq!(engine_says(script), "ok Items([])");
    }

    #[test]
    fn where_on_a_column_only_the_joined_table_has_is_a_warning() {
        let mut session = join_session();
        let script =
            "SCAN * FROM orders JOIN people ON orders.customer = people.id WHERE name = 'Ada';";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "name"));
        assert_eq!(
            message,
            "WHERE filters orders before the join, and 'name' is a column of people, not \
             orders. Filter people in a WITH query instead"
        );
        assert_eq!(engine_says(script), "ok Items([])");
    }

    #[test]
    fn where_on_the_first_table_is_clean() {
        let mut session = join_session();
        let script =
            "SCAN name FROM orders JOIN people ON orders.customer = people.id WHERE customer = 'c1';";
        assert!(found(&mut session, script).is_empty());
        assert!(engine_says(script).contains("Ada"));
    }

    #[test]
    fn selecting_a_qualified_column_that_is_kept_plain_is_a_warning() {
        let mut session = join_session();
        let joined = "SELECT people.name FROM orders JOIN people ON orders.customer = people.id;";
        let (severity, message, at) = only(&mut session, joined);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "people.name"));
        assert_eq!(
            message,
            "joined rows keep 'name' unqualified, so people.name is always null. Select name"
        );
        assert!(engine_says(joined).contains("\"people.name\": Null"));

        let first =
            "SELECT orders.customer FROM orders JOIN people ON orders.customer = people.id;";
        let (severity, _, at) = only(&mut session, first);
        assert_eq!(
            (severity.as_str(), at.as_str()),
            ("warning", "orders.customer")
        );

        // people.id clashes with orders.id, so the joined row keeps it as people.id.
        let clash = "SELECT people.id FROM orders JOIN people ON orders.customer = people.id;";
        assert!(found(&mut session, clash).is_empty());
    }

    #[test]
    fn a_join_on_with_names_checks_their_selected_columns() {
        let mut session = join_session();
        let script = "\
WITH a AS (SCAN id FROM orders), p AS (SCAN * FROM people)
SCAN * FROM a JOIN p ON a.customer = p.id;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "a.customer"));
        assert_eq!(
            message,
            "a has no column 'customer', so this JOIN matches no rows. a has: id"
        );
        assert_eq!(engine_says(script), "ok Items([])");
    }

    #[test]
    fn a_misspelled_with_name_in_a_join_suggests_it() {
        let mut session = join_session();
        let script = "\
WITH recent AS (SCAN * FROM orders)
SCAN * FROM recnt JOIN people ON recnt.customer = people.id;";
        let issues = found(&mut session, script);
        let error = issues
            .iter()
            .find(|issue| issue.0 == "error")
            .expect("error");
        assert_eq!(error.2, "recnt");
        assert!(error.1.ends_with("Did you mean 'recent'?"), "{}", error.1);
        let unused = issues
            .iter()
            .find(|issue| issue.0 == "warning")
            .expect("warning");
        assert_eq!(unused.2, "recent");
    }

    #[test]
    fn a_with_query_that_reads_a_later_name_is_an_error() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN * FROM b), b AS (SCAN * FROM orders) SCAN * FROM a;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "b"));
        assert_eq!(
            message,
            "WITH query 'a' reads 'b', which is defined after it. \
             A WITH query reads only the WITH names before it"
        );
        assert!(engine_says(script).contains("Table 'b' not found"));
    }

    #[test]
    fn a_with_query_that_reads_itself_is_an_error() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN * FROM a) SCAN * FROM a;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!(severity, "error");
        assert_eq!(at, "a");
        assert_eq!(
            message,
            "WITH query 'a' reads itself. A WITH query reads only the WITH names before it"
        );
        assert!(engine_says(script).contains("Table 'a' not found"));
    }

    #[test]
    fn a_with_query_with_count_star_is_an_error() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN COUNT(*) FROM orders) SCAN * FROM a;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "COUNT"));
        assert_eq!(
            message,
            "WITH query 'a' uses COUNT(*), but a WITH query must return rows"
        );
        assert!(engine_says(script).contains("COUNT(*)"));
    }

    #[test]
    fn a_with_name_used_in_a_later_statement_is_an_error() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN * FROM orders) SCAN * FROM a;\nSCAN * FROM a;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("error", "a"));
        assert_eq!(
            message,
            "'a' is a WITH name from an earlier statement. A WITH name lasts for one \
             statement; use CREATE TEMPTABLE a AS (…) to keep its rows"
        );
        assert!(engine_says(script).contains("Table 'a' not found"));
    }

    #[test]
    fn a_with_name_kept_as_a_temp_table_is_readable_later() {
        let mut session = join_session();
        let script = "\
WITH a AS (SCAN * FROM orders) SCAN * FROM a;
CREATE TEMPTABLE a AS (SCAN * FROM orders);
SCAN * FROM a;";
        assert!(found(&mut session, script).is_empty());
    }

    #[test]
    fn a_misspelled_with_name_in_the_body_suggests_it() {
        let mut session = join_session();
        let script = "WITH recent AS (SCAN * FROM orders) SCAN * FROM recnt;";
        let issues = found(&mut session, script);
        let missing = issues
            .iter()
            .find(|issue| issue.2 == "recnt")
            .expect("recnt");
        assert_eq!(missing.0, "warning");
        assert_eq!(
            missing.1,
            "table recnt has no cached definition, so key and index checks were skipped. \
             Did you mean 'recent'?"
        );
    }

    #[test]
    fn an_unused_with_name_is_a_warning_at_its_definition() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN * FROM orders), b AS (SCAN * FROM people) SCAN * FROM a;";
        let (severity, message, at) = only(&mut session, script);
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "b"));
        assert_eq!(
            message,
            "WITH name 'b' is never read, but its query still runs"
        );
    }

    #[test]
    fn a_with_name_read_only_by_a_later_with_query_counts_as_read() {
        let mut session = join_session();
        let script = "WITH a AS (SCAN * FROM orders), b AS (SCAN * FROM a) SCAN * FROM b;";
        assert!(found(&mut session, script).is_empty());
    }

    #[test]
    fn an_issue_in_a_with_query_points_into_that_query() {
        let mut session = join_session();
        // shelf appears in both the WITH query and the body. The missing one is in the query.
        let script = "\
WITH shelf_rows AS (SCAN * FROM orders JOIN shelf ON orders.id = shelf.id)
SCAN * FROM shelf_rows;";
        let issues = found(&mut session, script);
        assert_eq!(issues.len(), 1, "{issues:#?}");
        let issue = lint_script(&mut session, script).issues.remove(0);
        assert_eq!(issue.severity, "error");
        assert_eq!((issue.line, issue.column), (1, 45));
    }

    #[test]
    fn a_missing_table_warning_underlines_the_table_name() {
        let mut session = session(false);
        let (severity, _, at) = only(&mut session, "SCAN * FROM shelf;");
        assert_eq!((severity.as_str(), at.as_str()), ("warning", "shelf"));
    }
}
