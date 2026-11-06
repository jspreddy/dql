//! In-process workspace for the native window.
//!
//! The same functions the web handlers call. The window does not open a socket.

use std::collections::HashMap;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::environment::{self, EnvFacts};
use super::files::{self, DiskFile, Node, SaveFile};
use super::lint::LintReport;
use super::query_cache::{self, StoredQuery};
use super::search::SearchResults;
use super::shape::{
    self, changes_schema, column_order, explain_index, read_query, safe_table_name,
    split_statements, Column, TableList, TableShape, PAGE_SIZE,
};
use crate::session::Session;

/// One statement the window can draw as a card.
#[derive(Clone, Debug, PartialEq)]
pub struct StatementCard {
    pub statement: String,
    pub ok: bool,
    pub kind: String,
    pub message: String,
    pub error: String,
    pub items: Vec<Value>,
    pub columns: Vec<Column>,
    pub elapsed_ms: u64,
    pub read_units: f64,
    pub write_units: f64,
    pub help: Option<Value>,
    pub affected: Option<u64>,
}

impl StatementCard {
    pub fn failure_text(&self) -> String {
        if !self.error.is_empty() {
            self.error.clone()
        } else {
            self.message.clone()
        }
    }

    fn from_stored(stored: &StoredQuery) -> Self {
        let result = &stored.result;
        Self {
            statement: stored.query.clone(),
            ok: stored.success,
            kind: result
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or(if stored.success { "message" } else { "error" })
                .to_string(),
            message: result
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            error: result
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            items: result
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            columns: columns_from_value(result.get("columns")),
            elapsed_ms: result
                .get("elapsed_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            read_units: result
                .get("read_units")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            write_units: result
                .get("write_units")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            help: result.get("help").cloned(),
            affected: result.get("affected").and_then(Value::as_u64),
        }
    }

    fn to_value(&self) -> Value {
        let mut value = serde_json::json!({
            "statement": self.statement,
            "ok": self.ok,
            "kind": self.kind,
            "message": self.message,
            "affected": self.affected,
            "items": self.items,
            "error": self.error,
            "elapsed_ms": self.elapsed_ms,
            "read_units": self.read_units,
            "write_units": self.write_units,
        });
        if !self.columns.is_empty() {
            value["columns"] = serde_json::to_value(&self.columns).unwrap_or(Value::Null);
        }
        if let Some(help) = &self.help {
            value["help"] = help.clone();
        }
        value
    }
}

/// Mid-statement progress the window polls while a run holds the studio lock.
#[derive(Clone, Debug, Default)]
pub struct LiveProgress {
    pub path: String,
    pub index: usize,
    pub done: u64,
    pub total: Option<u64>,
    pub phase: String,
}

/// Workspace plus the session. Callers on other threads share it behind a mutex.
pub struct Studio {
    root: PathBuf,
    session: Session,
    shapes: HashMap<String, TableShape>,
}

impl Studio {
    pub fn open(session: Session, root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        let root = root.canonicalize().map_err(|err| err.to_string())?;
        Ok(Self {
            root,
            session,
            shapes: HashMap::new(),
        })
    }

    /// Offline session for tests. Does not read `~/.dql_history`.
    #[cfg(test)]
    pub fn memory(root: &Path) -> Result<Self, String> {
        Self::open(
            Session::new_memory_headless("us-west-1"),
            root.to_path_buf(),
        )
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn endpoint_label(&self) -> String {
        self.environment().label
    }

    pub fn environment_lines(&self) -> Vec<(String, String)> {
        let view = self.environment();
        let mut lines = vec![
            ("kind".to_string(), view.kind.to_string()),
            ("endpoint".to_string(), view.label.clone()),
        ];
        if let Some(profile) = view.profile {
            lines.push(("profile".to_string(), profile));
        }
        if let Some(region) = view.region {
            lines.push(("region".to_string(), region));
        }
        if let Some(key) = view.access_key {
            lines.push(("access key".to_string(), key));
        }
        if let Some(expiration) = view.expiration {
            lines.push(("expiration".to_string(), expiration));
        }
        lines
    }

    fn environment(&self) -> environment::EnvironmentView {
        environment::environment_view(
            self.session
                .local_endpoint
                .as_ref()
                .map(|(host, port)| (host.as_str(), *port)),
            &self.session.region,
            self.session.engine.is_memory(),
            &EnvFacts::from_env(),
        )
    }

    pub fn tree(&self) -> Result<Node, String> {
        files::tree(&self.root).map_err(into_string)
    }

    pub fn read_file(&self, relative: &str) -> Result<DiskFile, String> {
        files::read_file(&self.root, relative).map_err(into_string)
    }

    pub fn save_file(
        &self,
        relative: &str,
        text: &str,
        base: Option<&str>,
    ) -> Result<SaveFile, String> {
        files::save_file(&self.root, relative, text, base).map_err(into_string)
    }

    pub fn create_file(&self, relative: &str) -> Result<String, String> {
        files::create_file(&self.root, relative).map_err(into_string)
    }

    pub fn make_dir(&self, relative: &str) -> Result<String, String> {
        files::make_dir(&self.root, relative).map_err(into_string)
    }

    pub fn move_path(&self, source: &str, dest: &str) -> Result<(String, &'static str), String> {
        files::move_path(&self.root, source, dest).map_err(into_string)
    }

    pub fn duplicate_path(&self, source: &str) -> Result<(String, &'static str), String> {
        files::duplicate_path(&self.root, source).map_err(into_string)
    }

    pub fn delete_path(&self, relative: &str) -> Result<(), String> {
        files::delete_path(&self.root, relative).map_err(into_string)
    }

    pub fn lint(&mut self, text: &str) -> LintReport {
        super::lint::lint_script(&mut self.session, text)
    }

    pub fn search(&mut self, query: &str) -> SearchResults {
        super::search::search(&mut self.session, &self.root, query)
    }

    pub fn dql_files_under(&self, relative: &str) -> Result<Vec<String>, String> {
        let tree = files::tree(&self.root).map_err(into_string)?;
        let target = relative.trim().trim_matches('/');
        let node = if target.is_empty() || target == "." {
            &tree
        } else {
            find_node(&tree, target)
                .ok_or_else(|| "choose a file or folder in the workspace".to_string())?
        };
        let mut paths = Vec::new();
        collect_files(node, &mut paths);
        paths.sort();
        Ok(paths)
    }

    pub fn load_results(&self, relative: &str) -> Result<Vec<StatementCard>, String> {
        let stored = query_cache::list(&self.root, relative).map_err(into_string)?;
        Ok(stored
            .iter()
            .rev()
            .map(StatementCard::from_stored)
            .collect())
    }

    pub fn clear_results(&self, relative: &str) -> Result<Vec<String>, String> {
        query_cache::clear_at(&self.root, relative).map_err(into_string)
    }

    pub fn tables(&mut self, pattern: &str, refresh: bool) -> Result<TableList, String> {
        crate::meta::ls::web_table_list(&mut self.session, pattern, refresh, false)
    }

    pub fn clear_table_cache(&mut self) {
        self.shapes.clear();
        self.session.clear_table_cache();
    }

    pub fn cache_refreshing(&self) -> bool {
        self.session.cache_refreshing()
    }

    /// Saved table-list age for the settings menu. Does not list tables.
    pub fn table_cache_status(&self) -> crate::web::shape::TableCacheHint {
        crate::meta::ls::web_table_cache_status(&self.session)
    }

    pub fn start_table_load(&self) -> bool {
        self.session.start_background_load()
    }

    pub fn table_page(&mut self, name: &str, page: usize) -> Result<TablePage, String> {
        if !safe_table_name(name) {
            return Err(format!("unsupported table name {name:?}"));
        }
        let limit = PAGE_SIZE.saturating_mul(page.saturating_add(1));
        let envelope = self
            .session
            .execute_for_serve(&format!("SCAN * FROM {name} LIMIT {limit}"));
        if !envelope.ok {
            let error = envelope
                .error
                .as_ref()
                .map(|err| err.message.clone())
                .or(envelope.message)
                .unwrap_or_else(|| "scan failed".to_string());
            return Err(error);
        }
        let items = envelope.items.unwrap_or_default();
        let start = PAGE_SIZE.saturating_mul(page);
        let end = (start + PAGE_SIZE).min(items.len());
        let window = items.get(start..end).unwrap_or(&[]).to_vec();
        let shape = self.load_shape(name);
        let columns = column_order(&window, shape.as_ref(), None, None);
        Ok(TablePage {
            name: name.to_string(),
            page,
            page_size: PAGE_SIZE,
            items: window,
            columns,
            has_more: items.len() >= limit,
        })
    }

    pub fn cancel_flag(&self) -> dql_engine::CancelFlag {
        self.session.cancel().clone()
    }

    /// Run every statement in `text`. Stops on the first failure or on cancel.
    ///
    /// `progress` is updated from the engine callback and does not need this studio.
    pub fn run_text(
        &mut self,
        relative: &str,
        text: &str,
        progress: &Arc<Mutex<LiveProgress>>,
    ) -> Vec<StatementCard> {
        self.session.cancel().clear();
        let statements = split_statements(text);
        let mut cards = Vec::new();
        for (index, statement) in statements.iter().enumerate() {
            if self.session.cancel().is_requested() {
                cards.push(cancelled(statement));
                break;
            }
            {
                let mut live = progress.lock().unwrap_or_else(|err| err.into_inner());
                live.index = index;
                live.done = 0;
                live.total = None;
                live.phase = "running".to_string();
            }
            let live = Arc::clone(progress);
            self.session.progress().set(Some(Box::new(move |event| {
                let mut slot = live.lock().unwrap_or_else(|err| err.into_inner());
                slot.done = event.done;
                slot.total = event.total;
                slot.phase = event.phase;
            })));
            let started = std::time::Instant::now();
            let envelope = self.session.execute_for_serve(statement);
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let (read_units, write_units) = self.session.engine.statement_capacity();
            self.session.progress().clear();
            let columns = if envelope.ok && envelope.kind == "items" {
                self.columns_for(statement, envelope.items.as_deref().unwrap_or(&[]))
            } else {
                Vec::new()
            };
            if envelope.ok && changes_schema(statement) {
                self.shapes.clear();
            }
            let card = card_from(
                statement,
                &envelope,
                columns,
                elapsed_ms,
                read_units,
                write_units,
            );
            let ok = card.ok;
            if !relative.is_empty() {
                let _ =
                    query_cache::save(&self.root, relative, statement, card.to_value(), card.ok);
            }
            cards.push(card);
            if !ok || self.session.cancel().is_requested() {
                break;
            }
        }
        cards
    }

    fn columns_for(&mut self, statement: &str, items: &[Value]) -> Vec<Column> {
        let (table, selection) = read_query(statement);
        let Some(table) = table else {
            return column_order(items, None, None, None);
        };
        let explained = self
            .session
            .execute_for_serve(&format!("EXPLAIN {statement}"));
        let index = if explained.ok {
            explain_index(explained.message.as_deref().unwrap_or(""))
        } else {
            None
        };
        let mut shape = self.load_shape(&table);
        if let Some(name) = index.as_deref() {
            let missing = shape
                .as_ref()
                .is_some_and(|shape| !shape.indexes.contains_key(name));
            if missing {
                self.shapes.remove(&table);
                shape = self.load_shape(&table);
            }
        }
        column_order(
            items,
            shape.as_ref(),
            index.as_deref(),
            selection.as_deref(),
        )
    }

    fn load_shape(&mut self, name: &str) -> Option<TableShape> {
        if let Some(shape) = self.shapes.get(name) {
            return Some(shape.clone());
        }
        if !safe_table_name(name) {
            return None;
        }
        let detail =
            shape::ExecText::from_envelope(&self.session.execute_for_serve(&format!("ls {name}")));
        if !detail.ok {
            return None;
        }
        let shape = shape::parse_table_shape(&detail.message)?;
        self.shapes.insert(name.to_string(), shape.clone());
        Some(shape)
    }
}

#[derive(Clone, Debug)]
pub struct TablePage {
    pub name: String,
    pub page: usize,
    pub page_size: usize,
    pub items: Vec<Value>,
    pub columns: Vec<Column>,
    pub has_more: bool,
}

fn card_from(
    statement: &str,
    envelope: &crate::serve::protocol::ServeEnvelope,
    columns: Vec<Column>,
    elapsed_ms: u64,
    read_units: f64,
    write_units: f64,
) -> StatementCard {
    let error = envelope
        .error
        .as_ref()
        .map(|err| err.message.clone())
        .unwrap_or_default();
    let message = match envelope.message.as_deref() {
        Some(message) if !message.is_empty() => message.to_string(),
        _ => error.clone(),
    };
    StatementCard {
        statement: statement.to_string(),
        ok: envelope.ok,
        kind: envelope.kind.clone(),
        message,
        error,
        items: envelope.items.clone().unwrap_or_default(),
        columns,
        elapsed_ms,
        read_units,
        write_units,
        help: envelope.help.clone(),
        affected: envelope.affected,
    }
}

fn cancelled(statement: &str) -> StatementCard {
    StatementCard {
        statement: statement.to_string(),
        ok: false,
        kind: "error".to_string(),
        message: "cancelled".to_string(),
        error: "cancelled".to_string(),
        items: Vec::new(),
        columns: Vec::new(),
        elapsed_ms: 0,
        read_units: 0.0,
        write_units: 0.0,
        help: None,
        affected: None,
    }
}

fn columns_from_value(value: Option<&Value>) -> Vec<Column> {
    let Some(list) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|column| {
            let name = column.get("name")?.as_str()?.to_string();
            let text = |key| column.get(key).and_then(Value::as_str).map(str::to_string);
            Some(Column {
                name,
                table: text("table"),
                index: text("index"),
            })
        })
        .collect()
}

fn into_string(err: files::PathError) -> String {
    err.0
}

fn find_node<'a>(node: &'a Node, path: &str) -> Option<&'a Node> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .as_ref()
        .and_then(|children| children.iter().find_map(|child| find_node(child, path)))
}

fn collect_files(node: &Node, paths: &mut Vec<String>) {
    if node.kind == "file" {
        paths.push(node.path.clone());
    }
    if let Some(children) = &node.children {
        for child in children {
            collect_files(child, paths);
        }
    }
}

/// The statement whose byte range contains `cursor`, including the trailing semicolon.
pub fn statement_at(text: &str, cursor: usize) -> Option<String> {
    let cursor = cursor.min(text.len());
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let (byte, ch) = chars[index];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        if quote.is_none() && ch == '-' && next == Some('-') {
            index += 2;
            while index < chars.len() && chars[index].1 != '\n' {
                index += 1;
            }
            continue;
        }
        if let Some(open) = quote {
            if ch == open && chars.get(index.wrapping_sub(1)).map(|(_, c)| *c) != Some('\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == ';' {
            let end = byte + ch.len_utf8();
            if cursor <= end && cursor >= start {
                let statement = text[start..end].trim();
                if has_code(statement) {
                    return Some(ensure_semi(statement));
                }
            }
            start = end;
            index += 1;
            continue;
        }
        index += 1;
    }
    if cursor >= start {
        let statement = text[start..].trim();
        if has_code(statement) {
            return Some(ensure_semi(statement));
        }
    }
    None
}

fn ensure_semi(statement: &str) -> String {
    if statement.ends_with(';') {
        statement.to_string()
    } else {
        format!("{statement};")
    }
}

fn has_code(text: &str) -> bool {
    text.chars().any(|ch| !ch.is_whitespace() && ch != ';')
        && !text
            .lines()
            .all(|line| line.trim().is_empty() || line.trim().starts_with("--"))
}

/// Labels for the file-tree menu. `kind` is `file`, `dir`, or `root`.
pub fn menu_labels(kind: &str) -> Vec<&'static str> {
    match kind {
        "file" => vec![
            "Run file",
            "New file",
            "Duplicate",
            "Rename",
            "Move",
            "Delete",
        ],
        "dir" => vec![
            "Run folder",
            "New file",
            "New folder",
            "Duplicate",
            "Rename",
            "Move",
            "Clear results",
            "Delete",
        ],
        _ => vec!["New file", "New folder"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::search::SearchHit;

    fn studio() -> (tempfile::TempDir, Studio) {
        let dir = tempfile::tempdir().unwrap();
        let studio = Studio::memory(dir.path()).unwrap();
        (dir, studio)
    }

    #[test]
    fn file_create_delete_and_foreign_folder() {
        let (_dir, studio) = studio();
        let path = studio.create_file("queries/note.dql").unwrap();
        assert_eq!(path, "queries/note.dql");
        studio.delete_path(&path).unwrap();
        std::fs::create_dir_all(studio.root().join("keep")).unwrap();
        std::fs::write(studio.root().join("keep/readme.txt"), "x").unwrap();
        let err = studio.delete_path("keep").unwrap_err();
        assert!(err.contains("not a .dql file"), "{err}");
        studio.make_dir("empty").unwrap();
        studio.delete_path("empty").unwrap();
    }

    #[test]
    fn save_conflicts_when_disk_changes() {
        let (_dir, studio) = studio();
        studio.create_file("edit.dql").unwrap();
        let first = studio
            .save_file("edit.dql", "SELECT * FROM t;\n", None)
            .unwrap();
        let SaveFile::Stored(stored) = first else {
            panic!("expected store");
        };
        std::fs::write(studio.root().join("edit.dql"), "SELECT * FROM other;\n").unwrap();
        let again = studio
            .save_file("edit.dql", "SELECT * FROM mine;\n", Some(&stored.revision))
            .unwrap();
        assert!(matches!(again, SaveFile::Conflict(_)));
        let disk = std::fs::read_to_string(studio.root().join("edit.dql")).unwrap();
        assert!(disk.contains("other"));
    }

    #[test]
    fn lint_flags_a_bad_query() {
        let (_dir, mut studio) = studio();
        let report = studio.lint("SELECT 1;\n");
        assert!(!report.ok);
        assert!(report.issues.iter().any(|issue| issue.severity == "error"));
    }

    #[test]
    fn runs_create_insert_select_and_keeps_history() {
        let (_dir, mut studio) = studio();
        studio.create_file("q.dql").unwrap();
        let progress = Arc::new(Mutex::new(LiveProgress::default()));
        let script = "\
CREATE TABLE demo (id STRING HASH KEY);
INSERT INTO demo (id) VALUES ('a');
SELECT * FROM demo WHERE id = 'a';
";
        let cards = studio.run_text("q.dql", script, &progress);
        assert_eq!(cards.len(), 3, "{cards:?}");
        assert!(cards.iter().all(|card| card.ok), "{cards:?}");
        let body = serde_json::to_string(&cards[2].items).unwrap();
        assert!(body.contains('a'), "{body}");
        let loaded = studio.load_results("q.dql").unwrap();
        assert_eq!(loaded.len(), 3);
        studio.clear_results("q.dql").unwrap();
        assert!(studio.load_results("q.dql").unwrap().is_empty());
    }

    #[test]
    fn statement_at_cursor_and_menu_labels() {
        let text = "SELECT * FROM a;\nINSERT INTO a (id) VALUES ('b');\n";
        let select = statement_at(text, 0).unwrap();
        assert!(select.starts_with("SELECT"), "{select}");
        let insert_at = text.find("INSERT").unwrap();
        let insert = statement_at(text, insert_at).unwrap();
        assert!(insert.starts_with("INSERT"), "{insert}");
        assert!(menu_labels("file").contains(&"Run file"));
        assert!(!menu_labels("file").contains(&"Run folder"));
        assert!(menu_labels("dir").contains(&"Run folder"));
        assert!(!menu_labels("dir").contains(&"Run file"));
        assert_eq!(menu_labels("root"), ["New file", "New folder"]);
    }

    #[test]
    fn search_finds_a_query_file() {
        let (_dir, mut studio) = studio();
        studio.create_file("queries/widgets.dql").unwrap();
        studio
            .save_file("queries/widgets.dql", "SELECT * FROM widgets;\n", None)
            .unwrap();
        let hits = studio.search("widgets");
        assert!(
            hits.items
                .iter()
                .any(|hit: &SearchHit| hit.path.as_deref() == Some("queries/widgets.dql")),
            "{:?}",
            hits.items
        );
    }
}
