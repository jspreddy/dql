//! In-process calls the window makes into the engine.
//!
//! `dql-cli` implements [`Host`] with the same Rust functions the web server
//! uses. Nothing here opens a socket.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub kind: String,
    pub name: String,
    pub path: String,
    pub children: Vec<TreeNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintIssue {
    pub severity: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub table: String,
    pub path: String,
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunResult {
    pub ok: bool,
    pub kind: String,
    pub message: String,
    pub items: Vec<Value>,
    pub affected: Option<u64>,
    pub help: Option<Value>,
}

impl RunResult {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            kind: "error".into(),
            message: message.into(),
            items: Vec::new(),
            affected: None,
            help: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub name: String,
    pub status: String,
    pub hash_key: String,
    pub range_key: String,
    pub item_count: String,
    pub read_units: String,
    pub write_units: String,
    pub indexes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheEntry {
    pub key: String,
    pub age: String,
    pub query: String,
}

/// What the window is allowed to ask the running `dqlrs` process.
pub trait Host {
    fn endpoint_label(&self) -> String;
    fn workspace(&self) -> &Path;
    fn tree(&self) -> Result<Vec<TreeNode>, String>;
    fn read_file(&self, path: &str) -> Result<String, String>;
    fn write_file(&mut self, path: &str, text: &str) -> Result<(), String>;
    fn create_file(&mut self, path: &str) -> Result<String, String>;
    fn create_dir(&mut self, path: &str) -> Result<String, String>;
    fn rename(&mut self, path: &str, name: &str) -> Result<String, String>;
    fn duplicate(&mut self, path: &str) -> Result<String, String>;
    fn move_to(&mut self, path: &str, dest_dir: &str) -> Result<String, String>;
    fn delete(&mut self, path: &str) -> Result<(), String>;
    fn lint(&mut self, text: &str) -> Vec<LintIssue>;
    fn search(&mut self, query: &str) -> Vec<SearchHit>;
    fn execute(&mut self, dql: &str) -> RunResult;
    fn cancel(&self);
    fn list_tables(&mut self, pattern: &str, refresh: bool) -> Result<Vec<TableRow>, String>;
    fn cache_status(&self) -> String;
    fn cache_entries(&self) -> Vec<CacheEntry>;
    fn clear_cache(&mut self) -> Result<(), String>;
}

/// A workspace of files and canned query replies, for headless tests.
pub struct MemoryHost {
    pub root: PathBuf,
    pub endpoint: String,
    pub replies: Vec<(String, RunResult)>,
    pub tables: Vec<TableRow>,
    pub cache_label: String,
    pub refreshes: u32,
    cancel_requested: AtomicBool,
}

impl MemoryHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            endpoint: "memory".into(),
            replies: Vec::new(),
            tables: Vec::new(),
            cache_label: "not cached".into(),
            refreshes: 0,
            cancel_requested: AtomicBool::new(false),
        }
    }
}

impl Host for MemoryHost {
    fn endpoint_label(&self) -> String {
        self.endpoint.clone()
    }

    fn workspace(&self) -> &Path {
        &self.root
    }

    fn tree(&self) -> Result<Vec<TreeNode>, String> {
        read_tree(&self.root, &self.root)
    }

    fn read_file(&self, path: &str) -> Result<String, String> {
        let full = inside(&self.root, path)?;
        std::fs::read_to_string(full).map_err(|err| err.to_string())
    }

    fn write_file(&mut self, path: &str, text: &str) -> Result<(), String> {
        let full = inside(&self.root, path)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(full, text).map_err(|err| err.to_string())
    }

    fn create_file(&mut self, path: &str) -> Result<String, String> {
        let path = normalize_dql(path);
        self.write_file(&path, "")?;
        Ok(path)
    }

    fn create_dir(&mut self, path: &str) -> Result<String, String> {
        let full = inside(&self.root, path)?;
        std::fs::create_dir_all(&full).map_err(|err| err.to_string())?;
        rel(&self.root, &full)
    }

    fn rename(&mut self, path: &str, name: &str) -> Result<String, String> {
        let src = inside(&self.root, path)?;
        let parent = src.parent().unwrap_or(self.root.as_path());
        let dest = parent.join(name);
        std::fs::rename(&src, &dest).map_err(|err| err.to_string())?;
        rel(&self.root, &dest)
    }

    fn duplicate(&mut self, path: &str) -> Result<String, String> {
        let src = inside(&self.root, path)?;
        let dest = unique_copy(&src);
        if src.is_dir() {
            copy_dir(&src, &dest)?;
        } else {
            std::fs::copy(&src, &dest).map_err(|err| err.to_string())?;
        }
        rel(&self.root, &dest)
    }

    fn move_to(&mut self, path: &str, dest_dir: &str) -> Result<String, String> {
        let src = inside(&self.root, path)?;
        let dir = inside(&self.root, dest_dir)?;
        let dest = dir.join(src.file_name().unwrap_or_default());
        std::fs::rename(&src, &dest).map_err(|err| err.to_string())?;
        rel(&self.root, &dest)
    }

    fn delete(&mut self, path: &str) -> Result<(), String> {
        let full = inside(&self.root, path)?;
        if full.is_file() {
            let name = full
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with('.') || !name.ends_with(".dql") {
                return Err("only .dql files can be deleted".into());
            }
            return std::fs::remove_file(full).map_err(|err| err.to_string());
        }
        if let Some(blocked) = foreign_entry(&full)? {
            return Err(format!(
                "folder contains {blocked}, which is not a .dql file"
            ));
        }
        std::fs::remove_dir_all(full).map_err(|err| err.to_string())
    }

    fn lint(&mut self, text: &str) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            let word = trimmed.split_whitespace().next().unwrap_or("");
            if crate::tokens::style_for_word(word).is_none()
                && !trimmed.ends_with(';')
                && word
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphabetic())
            {
                issues.push(LintIssue {
                    severity: "error".into(),
                    message: format!("unknown statement starting with {word}"),
                    line: index + 1,
                    column: 1,
                });
            }
        }
        issues
    }

    fn search(&mut self, query: &str) -> Vec<SearchHit> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() || needle.chars().count() > 200 {
            return Vec::new();
        }
        let mut hits = Vec::new();
        if let Ok(nodes) = self.tree() {
            walk_search(&self.root, &nodes, &needle, &mut hits);
        }
        for table in &self.tables {
            if table.name.to_ascii_lowercase().contains(&needle) {
                hits.push(SearchHit {
                    kind: "table".into(),
                    title: table.name.clone(),
                    detail: String::new(),
                    table: table.name.clone(),
                    path: String::new(),
                    offset: None,
                });
            }
            for (role, key) in [("hash", &table.hash_key), ("range", &table.range_key)] {
                if !key.is_empty() && key.to_ascii_lowercase().contains(&needle) {
                    hits.push(SearchHit {
                        kind: "column".into(),
                        title: key.clone(),
                        detail: format!("{role} {}", table.name),
                        table: table.name.clone(),
                        path: String::new(),
                        offset: None,
                    });
                }
            }
        }
        hits
    }

    fn execute(&mut self, dql: &str) -> RunResult {
        if self.cancel_requested.swap(false, Ordering::SeqCst) {
            return RunResult::error("Query stopped");
        }
        for (needle, result) in &self.replies {
            if dql.contains(needle.as_str()) {
                return result.clone();
            }
        }
        let trimmed = dql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("HELP") {
            return help_result(trimmed);
        }
        if upper.starts_with("EXPLAIN") {
            return explain_result();
        }
        if upper.starts_with("DESCRIBE") || upper.starts_with("SHOW TABLES") {
            return describe_result(trimmed);
        }
        if trimmed.is_empty() {
            return RunResult::error("Nothing to run.");
        }
        RunResult {
            ok: true,
            kind: "message".into(),
            message: format!("ran {dql}"),
            items: Vec::new(),
            affected: None,
            help: None,
        }
    }

    fn cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
    }

    fn list_tables(&mut self, pattern: &str, refresh: bool) -> Result<Vec<TableRow>, String> {
        if refresh {
            self.refreshes = self.refreshes.saturating_add(1);
        }
        let pattern = pattern.trim().to_ascii_lowercase();
        Ok(self
            .tables
            .iter()
            .filter(|row| pattern.is_empty() || row.name.to_ascii_lowercase().contains(&pattern))
            .cloned()
            .collect())
    }

    fn cache_status(&self) -> String {
        self.cache_label.clone()
    }

    fn cache_entries(&self) -> Vec<CacheEntry> {
        Vec::new()
    }

    fn clear_cache(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn read_tree(root: &Path, dir: &Path) -> Result<Vec<TreeNode>, String> {
    let mut nodes = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|err| err.to_string())?;
    let mut paths: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    paths.sort_by_key(|entry| entry.file_name());
    for entry in paths {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let relative = rel(root, &path)?;
        if path.is_dir() {
            nodes.push(TreeNode {
                kind: "dir".into(),
                name,
                path: relative,
                children: read_tree(root, &path)?,
            });
        } else if name.ends_with(".dql") {
            nodes.push(TreeNode {
                kind: "file".into(),
                name,
                path: relative,
                children: Vec::new(),
            });
        }
    }
    Ok(nodes)
}

fn walk_search(root: &Path, nodes: &[TreeNode], needle: &str, hits: &mut Vec<SearchHit>) {
    for node in nodes {
        if node.kind == "file" {
            let text = std::fs::read_to_string(root.join(&node.path)).unwrap_or_default();
            let lowered = text.to_ascii_lowercase();
            let name_hit = node.name.to_ascii_lowercase().contains(needle);
            if name_hit || lowered.contains(needle) {
                let offset = lowered.find(needle).unwrap_or(0);
                hits.push(SearchHit {
                    kind: "query".into(),
                    title: node.name.clone(),
                    detail: node.path.clone(),
                    table: String::new(),
                    path: node.path.clone(),
                    offset: Some(offset),
                });
            }
        }
        walk_search(root, &node.children, needle, hits);
    }
}

fn help_result(statement: &str) -> RunResult {
    let topic = statement[4..].trim();
    if topic.is_empty() {
        return RunResult {
            ok: true,
            kind: "help".into(),
            message: "Available commands".into(),
            items: Vec::new(),
            affected: None,
            help: Some(serde_json::json!({
                "entries": [{
                    "name": "select",
                    "summary": "Query items from a table or index"
                }]
            })),
        };
    }
    RunResult {
        ok: true,
        kind: "help".into(),
        message: format!("HELP {}", topic.to_ascii_lowercase()),
        items: Vec::new(),
        affected: None,
        help: Some(serde_json::json!({
            "code": [
                "CREATE TABLE widgets (id HASH KEY);",
                "WHERE can compare a.b.c and a[0].b on SELECT and SCAN."
            ]
        })),
    }
}

fn explain_result() -> RunResult {
    RunResult {
        ok: true,
        kind: "explain".into(),
        message: "query widgets TABLE".into(),
        items: Vec::new(),
        affected: None,
        help: Some(serde_json::json!({
            "projection expression": "#f",
            "key condition expression": "id = ",
            "filter expression": "n > ",
            "attribute names": "\"a\"",
            "attribute values": "\"ada\"",
            "consistent": "true",
            "operation": "query"
        })),
    }
}

fn describe_result(statement: &str) -> RunResult {
    let pattern = statement
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .trim_matches('"')
        .trim_matches('\'');
    let exact = pattern.eq_ignore_ascii_case("widgets");
    let message = if pattern.is_empty() || exact {
        "Hash Key id\nCREATE TABLE widgets (id STRING HASH KEY);".into()
    } else {
        format!("No exact match for \"{pattern}\"\nshowing similar names")
    };
    RunResult {
        ok: true,
        kind: "describe".into(),
        message,
        items: Vec::new(),
        affected: None,
        help: Some(serde_json::json!({
            "capacity": "12 RCU",
            "elapsed": "4 ms"
        })),
    }
}

fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = relative.trim_start_matches('/');
    if relative.split('/').any(|part| part == ".." || part == ".") {
        return Err("path escapes the workspace".into());
    }
    Ok(root.join(relative))
}

fn rel(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|err| err.to_string())
}

fn normalize_dql(path: &str) -> String {
    if path.ends_with(".dql") {
        path.to_string()
    } else {
        format!("{path}.dql")
    }
}

fn unique_copy(src: &Path) -> PathBuf {
    let parent = src.parent().unwrap_or(Path::new("."));
    let stem = src.file_stem().unwrap_or_default().to_string_lossy();
    let ext = src
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let first = parent.join(format!("{stem} copy{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = parent.join(format!("{stem} copy {n}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem} copy{ext}"))
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|err| err.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let to = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn foreign_entry(dir: &Path) -> Result<Option<String>, String> {
    for entry in std::fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = foreign_entry(&path)? {
                return Ok(Some(name));
            }
        } else {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !name.ends_with(".dql") {
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}
