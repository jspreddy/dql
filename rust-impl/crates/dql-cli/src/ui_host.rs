//! In-process engine for `dqlrs --ui`.
//!
//! The window calls these methods directly. This module does not open a socket
//! and does not speak HTTP.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dql_engine::{help_page, HelpBlock, HelpPage};
use dql_ui::host::{CacheEntry, Host, LintIssue, RunResult, SearchHit, TableRow, TreeNode};
use dql_ui::tokens::cache_age_label;
use serde_json::{json, Value};

use crate::session::Session;
use crate::web::{files, lint, search};

pub struct LiveHost {
    session: Arc<Mutex<Session>>,
    root: PathBuf,
    endpoint: String,
}

impl LiveHost {
    pub fn new(session: Session, root: PathBuf, endpoint: String) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            root,
            endpoint,
        }
    }

    /// Describe every table on a background thread, the same work `--web --load-tables` does.
    pub fn load_tables_in_background(&self) {
        let session = Arc::clone(&self.session);
        std::thread::spawn(move || {
            let names = {
                let mut session = lock(&session);
                match session.engine.cached_table_names(true) {
                    Ok(names) => names,
                    Err(err) => {
                        eprintln!("dqlrs: load tables: {err}");
                        return;
                    }
                }
            };
            for name in names {
                let mut session = lock(&session);
                if let Err(err) = session.engine.describe(&name, false) {
                    eprintln!("dqlrs: load tables: {name}: {err}");
                }
            }
        });
    }
}

impl Host for LiveHost {
    fn endpoint_label(&self) -> String {
        self.endpoint.clone()
    }

    fn workspace(&self) -> &Path {
        &self.root
    }

    fn tree(&self) -> Result<Vec<TreeNode>, String> {
        let node = files::tree(&self.root).map_err(|err| err.0)?;
        Ok(node
            .children
            .unwrap_or_default()
            .into_iter()
            .map(tree_node)
            .collect())
    }

    fn read_file(&self, path: &str) -> Result<String, String> {
        files::read_text(&self.root, path).map_err(|err| err.0)
    }

    fn write_file(&mut self, path: &str, text: &str) -> Result<(), String> {
        files::write_text(&self.root, path, text).map_err(|err| err.0)
    }

    fn create_file(&mut self, path: &str) -> Result<String, String> {
        files::create_file(&self.root, path).map_err(|err| err.0)
    }

    fn create_dir(&mut self, path: &str) -> Result<String, String> {
        files::make_dir(&self.root, path).map_err(|err| err.0)
    }

    fn rename(&mut self, path: &str, name: &str) -> Result<String, String> {
        let parent = Path::new(path).parent().unwrap_or(Path::new(""));
        let dest = parent.join(name);
        let dest = dest.to_string_lossy().replace('\\', "/");
        files::move_path(&self.root, path, &dest)
            .map(|(path, _)| path)
            .map_err(|err| err.0)
    }

    fn duplicate(&mut self, path: &str) -> Result<String, String> {
        files::duplicate_path(&self.root, path)
            .map(|(path, _)| path)
            .map_err(|err| err.0)
    }

    fn move_to(&mut self, path: &str, dest_dir: &str) -> Result<String, String> {
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest = if dest_dir.is_empty() || dest_dir == "." {
            name
        } else {
            format!("{dest_dir}/{name}")
        };
        files::move_path(&self.root, path, &dest)
            .map(|(path, _)| path)
            .map_err(|err| err.0)
    }

    fn delete(&mut self, path: &str) -> Result<(), String> {
        files::delete_path(&self.root, path).map_err(|err| err.0)
    }

    fn lint(&mut self, text: &str) -> Vec<LintIssue> {
        let mut session = lock(&self.session);
        lint::lint_script(&mut session, text)
            .issues
            .into_iter()
            .map(|issue| LintIssue {
                severity: issue.severity,
                message: issue.message,
                line: issue.line,
                column: issue.column,
            })
            .collect()
    }

    fn search(&mut self, query: &str) -> Vec<SearchHit> {
        let mut session = lock(&self.session);
        search::search(&mut session, &self.root, query)
            .items
            .into_iter()
            .map(|hit| SearchHit {
                kind: hit.kind,
                title: hit.title,
                detail: hit.detail,
                table: hit.table.unwrap_or_default(),
                path: hit.path.unwrap_or_default(),
                offset: hit.offset,
            })
            .collect()
    }

    fn execute(&mut self, dql: &str) -> RunResult {
        let mut session = lock(&self.session);
        session.cancel().clear();
        let envelope = session.execute_for_serve(dql);
        let message = envelope
            .error
            .as_ref()
            .map(|err| err.message.clone())
            .or(envelope.message)
            .unwrap_or_default();
        present_statement(
            dql,
            RunResult {
                ok: envelope.ok,
                kind: envelope.kind,
                message,
                items: envelope.items.unwrap_or_default(),
                affected: envelope.affected,
                help: envelope.help,
            },
        )
    }

    fn cancel(&self) {
        lock(&self.session).cancel().request();
    }

    fn list_tables(&mut self, pattern: &str, refresh: bool) -> Result<Vec<TableRow>, String> {
        let mut session = lock(&self.session);
        let listed = crate::meta::ls::web_table_list(&mut session, pattern, refresh, false)?;
        Ok(listed
            .tables
            .into_iter()
            .map(|table| {
                let hash = table
                    .keys
                    .iter()
                    .find(|key| key.role == "hash")
                    .map(|key| key.name.clone())
                    .unwrap_or_default();
                let range = table
                    .keys
                    .iter()
                    .find(|key| key.role == "range")
                    .map(|key| key.name.clone())
                    .unwrap_or_default();
                TableRow {
                    name: table.name,
                    status: String::new(),
                    hash_key: hash,
                    range_key: range,
                    item_count: String::new(),
                    read_units: String::new(),
                    write_units: String::new(),
                    indexes: String::new(),
                }
            })
            .collect())
    }

    fn cache_status(&self) -> String {
        let session = lock(&self.session);
        let hint = crate::meta::ls::web_table_cache_status(&session);
        if hint.text.is_empty() {
            return cache_age_label(None);
        }
        hint.text
    }

    fn cache_entries(&self) -> Vec<CacheEntry> {
        Vec::new()
    }

    fn clear_cache(&mut self) -> Result<(), String> {
        lock(&self.session).clear_table_cache();
        Ok(())
    }
}

fn present_statement(dql: &str, mut result: RunResult) -> RunResult {
    let statement = dql.trim().trim_end_matches(';').trim();
    let upper = statement.to_ascii_uppercase();
    if upper.starts_with("HELP") {
        let topic = statement[4..].trim();
        let topic = if topic.is_empty() { None } else { Some(topic) };
        if let Ok(page) = help_page(topic) {
            result.ok = true;
            result.kind = "help".into();
            result.message = page.title.clone();
            result.help = Some(help_json(&page));
        }
    } else if upper.starts_with("EXPLAIN") && result.kind.is_empty() {
        result.kind = "explain".into();
    } else if (upper.starts_with("DESCRIBE") || upper.starts_with("SHOW")) && result.kind.is_empty()
    {
        result.kind = "describe".into();
    }
    result
}

fn help_json(page: &HelpPage) -> Value {
    let mut entries = Vec::new();
    let mut code = Vec::new();
    let mut prose = Vec::new();
    for block in &page.blocks {
        match block {
            HelpBlock::Entry { name, summary } => {
                entries.push(json!({ "name": name, "summary": summary }));
            }
            HelpBlock::Code(text) => code.push(json!(text)),
            HelpBlock::Prose(text) | HelpBlock::Heading(text) => prose.push(json!(text)),
        }
    }
    json!({
        "title": page.title,
        "entries": entries,
        "code": code,
        "prose": prose,
    })
}

fn lock(session: &Mutex<Session>) -> std::sync::MutexGuard<'_, Session> {
    session.lock().unwrap_or_else(|err| err.into_inner())
}

fn tree_node(node: files::Node) -> TreeNode {
    TreeNode {
        kind: node.kind.to_string(),
        name: node.name,
        path: node.path,
        children: node
            .children
            .unwrap_or_default()
            .into_iter()
            .map(tree_node)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_host_calls_the_session_directly() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.dql"), "SELECT 1;\n").unwrap();
        let session = Session::new_memory("us-west-1");
        let mut host = LiveHost::new(session, dir.path().to_path_buf(), "memory".into());
        let tree = host.tree().unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "a.dql");
        assert_eq!(host.read_file("a.dql").unwrap(), "SELECT 1;\n");
        let result = host.execute("SELECT 1;");
        assert!(!result.message.is_empty() || !result.kind.is_empty());
        let help = host.execute("HELP;");
        assert_eq!(help.message, "Available commands");
        let page = help.help.unwrap().to_string();
        assert!(page.contains("Query items from a table or index"), "{page}");
        let topic = host.execute("HELP SELECT;");
        assert_eq!(topic.message, "HELP select");
        let body = topic.help.unwrap().to_string();
        assert!(
            body.contains("WHERE can compare a.b.c and a[0].b on SELECT and SCAN."),
            "{body}"
        );
        let setup = host
            .execute("CREATE TABLE demo (id STRING HASH KEY); INSERT INTO demo (id) VALUES ('a');");
        assert!(setup.ok, "{}", setup.message);
        let saved = dir.path().join("out.json");
        let save = host.execute(&format!("SCAN * FROM demo SAVE '{}';", saved.display()));
        assert!(save.ok, "{}", save.message);
        assert!(saved.is_file(), "{}", save.message);
        let loaded = host.execute(&format!("LOAD '{}' INTO demo;", saved.display()));
        assert!(loaded.ok, "{}", loaded.message);
        let missing = dir.path().join("missing-parent").join("out.json");
        let rejected = host.execute(&format!("SCAN * FROM demo SAVE '{}';", missing.display()));
        assert!(!rejected.ok, "{}", rejected.message);
        assert!(!missing.exists());
        let nested = dir.path().join("made").join("out.csv");
        let parents = host.execute(&format!(
            "SCAN * FROM demo SAVE '{}' CREATE PARENTS;",
            nested.display()
        ));
        assert!(parents.ok, "{}", parents.message);
        assert!(nested.is_file(), "{}", parents.message);
        host.cancel();
    }
}
