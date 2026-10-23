//! Title-bar search over table names, cached columns and indexes, and `.dql` queries.
//!
//! Table hits come from the table list. A saved definition is not required, and
//! a definition that is not on that list is not a table hit. Column and index
//! hits come only from definitions already saved. Search does not describe
//! tables that have not been fetched.

use std::path::Path;

use dql_engine::TableCacheStore;
use dql_models::TableMeta;
use serde::Serialize;

use super::files::{self, Node};
use crate::session::Session;

const PER_KIND: usize = 8;
const MAX_NEEDLE: usize = 200;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const TITLE_CHARS: usize = 80;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SearchResults {
    pub items: Vec<SearchHit>,
}

pub fn search(session: &mut Session, root: &Path, query: &str) -> SearchResults {
    let Some(needle) = needle(query) else {
        return SearchResults { items: Vec::new() };
    };
    let mut items = Vec::new();
    items.extend(table_hits(session, &needle));
    items.extend(schema_hits(session, &needle));
    items.extend(file_hits(root, &needle));
    SearchResults { items }
}

fn needle(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_NEEDLE).collect())
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn rank(title: &str, needle: &str) -> u8 {
    let title = title.to_lowercase();
    let needle = needle.to_lowercase();
    if title == needle {
        0
    } else if title.starts_with(&needle) {
        1
    } else {
        2
    }
}

fn sort_matches(mut hits: Vec<SearchHit>, needle: &str) -> Vec<SearchHit> {
    hits.sort_by(|left, right| {
        rank(&left.title, needle)
            .cmp(&rank(&right.title, needle))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.detail.to_lowercase().cmp(&right.detail.to_lowercase()))
    });
    hits
}

fn take_matches(hits: Vec<SearchHit>, needle: &str) -> Vec<SearchHit> {
    let mut hits = sort_matches(hits, needle);
    hits.truncate(PER_KIND);
    hits
}

fn table_hits(session: &mut Session, needle: &str) -> Vec<SearchHit> {
    let names = session.engine.cached_table_names(false).unwrap_or_default();
    let hits = names
        .into_iter()
        .filter(|name| contains_ci(name, needle))
        .map(|name| SearchHit {
            kind: "table".to_string(),
            title: name.clone(),
            detail: String::new(),
            table: Some(name),
            path: None,
            offset: None,
        })
        .collect();
    sort_matches(hits, needle)
}

fn schema_hits(session: &Session, needle: &str) -> Vec<SearchHit> {
    let Some(disk) = &session.table_disk else {
        return Vec::new();
    };
    let mut columns = Vec::new();
    let mut indexes = Vec::new();
    for name in disk.definition_names() {
        let Some(cached) = disk.get_definition(&name) else {
            continue;
        };
        if !cached.meta.has_definition() {
            continue;
        }
        columns.extend(column_hits(&cached.meta, needle));
        indexes.extend(index_hits(&cached.meta, needle));
    }
    let mut hits = take_matches(columns, needle);
    hits.extend(take_matches(indexes, needle));
    hits
}

fn column_names(meta: &TableMeta) -> Vec<String> {
    let mut names: Vec<String> = meta.attrs.keys().cloned().collect();
    for index in meta.local_indexes.values() {
        push_unique(&mut names, &index.hash_key);
        if let Some(range_key) = &index.range_key {
            push_unique(&mut names, range_key);
        }
    }
    for index in meta.global_indexes.values() {
        push_unique(&mut names, &index.hash_key.name);
        if let Some(range_key) = &index.range_key {
            push_unique(&mut names, &range_key.name);
        }
    }
    names
}

fn push_unique(names: &mut Vec<String>, name: &str) {
    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn column_hits(meta: &TableMeta, needle: &str) -> Vec<SearchHit> {
    column_names(meta)
        .into_iter()
        .filter(|name| contains_ci(name, needle))
        .map(|name| SearchHit {
            kind: "column".to_string(),
            detail: column_detail(meta, &name),
            title: name,
            table: Some(meta.name.clone()),
            path: None,
            offset: None,
        })
        .collect()
}

fn column_detail(meta: &TableMeta, name: &str) -> String {
    let mut parts = vec![meta.name.clone()];
    if meta.hash_key == name {
        parts.push("hash key".to_string());
    } else if meta.range_key.as_deref() == Some(name) {
        parts.push("range key".to_string());
    }
    let mut indexes = Vec::new();
    for index in meta.local_indexes.values() {
        let on_index = index.hash_key == name || index.range_key.as_deref() == Some(name);
        if on_index {
            indexes.push(index.name.clone());
        }
    }
    for index in meta.global_indexes.values() {
        let on_range = index
            .range_key
            .as_ref()
            .is_some_and(|field| field.name == name);
        if index.hash_key.name == name || on_range {
            indexes.push(index.name.clone());
        }
    }
    if !indexes.is_empty() {
        parts.push(indexes.join(", "));
    }
    parts.join(" · ")
}

fn index_hits(meta: &TableMeta, needle: &str) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    for index in meta.local_indexes.values() {
        if !contains_ci(&index.name, needle) {
            continue;
        }
        hits.push(index_hit(
            &meta.name,
            &index.name,
            &index.hash_key,
            index.range_key.as_deref(),
        ));
    }
    for index in meta.global_indexes.values() {
        if !contains_ci(&index.name, needle) {
            continue;
        }
        hits.push(index_hit(
            &meta.name,
            &index.name,
            &index.hash_key.name,
            index.range_key.as_ref().map(|field| field.name.as_str()),
        ));
    }
    hits
}

fn index_hit(table: &str, name: &str, hash_key: &str, range_key: Option<&str>) -> SearchHit {
    let mut keys = vec![hash_key.to_string()];
    if let Some(range_key) = range_key {
        keys.push(range_key.to_string());
    }
    SearchHit {
        kind: "index".to_string(),
        title: name.to_string(),
        detail: format!("{table} · {}", keys.join(", ")),
        table: Some(table.to_string()),
        path: None,
        offset: None,
    }
}

fn file_hits(root: &Path, needle: &str) -> Vec<SearchHit> {
    let Ok(tree) = files::tree(root) else {
        return Vec::new();
    };
    let mut statements = Vec::new();
    let mut files = Vec::new();
    collect_queries(&tree, root, needle, &mut statements, &mut files);
    let mut hits = take_matches(statements, needle);
    if hits.len() < PER_KIND {
        let mut room = take_matches(files, needle);
        room.truncate(PER_KIND - hits.len());
        hits.extend(room);
    }
    hits
}

fn collect_queries(
    node: &Node,
    root: &Path,
    needle: &str,
    statements: &mut Vec<SearchHit>,
    files: &mut Vec<SearchHit>,
) {
    if node.kind == "file" {
        if let Ok(text) = files::read_text(root, &node.path) {
            let matched = statement_hits(&node.path, &text, needle);
            let file_matched = contains_ci(&node.path, needle) || contains_ci(&node.name, needle);
            if matched.is_empty() && file_matched {
                let offset = query_statements(&text)
                    .first()
                    .map(|(offset, _)| *offset)
                    .unwrap_or(0);
                files.push(SearchHit {
                    kind: "query".to_string(),
                    title: node.name.clone(),
                    detail: node.path.clone(),
                    table: None,
                    path: Some(node.path.clone()),
                    offset: Some(offset),
                });
            } else {
                statements.extend(matched);
            }
        }
    }
    if let Some(children) = &node.children {
        for child in children {
            collect_queries(child, root, needle, statements, files);
        }
    }
}

fn statement_hits(path: &str, text: &str, needle: &str) -> Vec<SearchHit> {
    if text.len() > MAX_FILE_BYTES as usize {
        return Vec::new();
    }
    query_statements(text)
        .into_iter()
        .filter(|(_, statement)| contains_ci(statement, needle))
        .map(|(offset, statement)| SearchHit {
            kind: "query".to_string(),
            title: one_line(&statement),
            detail: path.to_string(),
            table: None,
            path: Some(path.to_string()),
            offset: Some(offset),
        })
        .collect()
}

fn one_line(text: &str) -> String {
    let flat = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ");
    let flat = if flat.is_empty() {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        flat
    };
    let mut title: String = flat.chars().take(TITLE_CHARS).collect();
    if flat.chars().count() > TITLE_CHARS {
        title.push('…');
    }
    title
}

/// Semicolon-separated queries. The offset is a character index of the first
/// code character, matching the editor's statement start for BMP text.
fn query_statements(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut buf_start = 0usize;
    let mut quote: Option<char> = None;
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied().unwrap_or('\0');
        if quote.is_none() && ch == '-' && next == '-' {
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if let Some(open) = quote {
            if ch == open && chars.get(index.wrapping_sub(1)).copied() != Some('\\') {
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
            push_statement(&chars, &mut spans, buf_start, index + 1);
            buf_start = index + 1;
            index += 1;
            continue;
        }
        index += 1;
    }
    push_statement(&chars, &mut spans, buf_start, chars.len());
    spans
}

fn push_statement(chars: &[char], spans: &mut Vec<(usize, String)>, from: usize, to: usize) {
    if from >= to || from >= chars.len() {
        return;
    }
    let raw: String = chars[from..to.min(chars.len())].iter().collect();
    if !has_code(&raw) {
        return;
    }
    let lead = raw.chars().count() - raw.trim_start().chars().count();
    let statement = raw.trim().to_string();
    spans.push((from + lead, statement));
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
    use dql_parser::parse_statement;

    fn meta(schema: &str) -> TableMeta {
        let statement = parse_statement(schema).expect("schema");
        TableMeta::from_create_statement(&statement).expect("meta")
    }

    #[test]
    fn columns_include_index_keys_and_index_names_match_separately() {
        let meta = meta(
            "CREATE TABLE orders (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-apple', apple STRING, id STRING);",
        );
        let columns = column_hits(&meta, "apple");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].kind, "column");
        assert_eq!(columns[0].table.as_deref(), Some("orders"));
        assert!(columns[0].detail.contains("by-apple"));
        let indexes = index_hits(&meta, "apple");
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].title, "by-apple");

        let only_index = index_hits(&meta, "by-ap");
        assert_eq!(only_index.len(), 1);
        assert_eq!(only_index[0].title, "by-apple");
        assert_eq!(only_index[0].table.as_deref(), Some("orders"));
        assert!(only_index[0].detail.contains("apple"));
        assert!(column_hits(&meta, "by-ap").is_empty());
    }

    #[test]
    fn query_offset_lands_on_the_statement_after_a_comment() {
        let text =
            "-- header stays with the select\nSELECT * FROM pw_missing_table\nWHERE id = 'a';\n";
        let spans = query_statements(text);
        assert_eq!(spans.len(), 1);
        let (offset, statement) = &spans[0];
        assert!(statement.starts_with("-- header"));
        assert_eq!(text.chars().nth(*offset), Some('-'));
        let hits = statement_hits("queries/read.dql", text, "pw_missing");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].title,
            "SELECT * FROM pw_missing_table WHERE id = 'a';"
        );
        assert_eq!(hits[0].path.as_deref(), Some("queries/read.dql"));
        assert_eq!(hits[0].offset, Some(*offset));
    }

    #[test]
    fn table_hits_follow_the_table_list_not_cached_definitions() {
        let mut session = crate::session::Session::new_memory_headless("us-west-1");
        let mut statements = Vec::new();
        for suffix in 'a'..='j' {
            statements.push(format!("CREATE TABLE shelf_{suffix} (id STRING HASH KEY);"));
        }
        statements.push("CREATE TABLE bookshelf (id STRING HASH KEY);".to_string());
        statements.push("CREATE TABLE pre_shelf_post (id STRING HASH KEY);".to_string());
        statements.push("CREATE TABLE orders (shelf_id STRING HASH KEY);".to_string());
        statements.push("CREATE TABLE gamma (id STRING HASH KEY);".to_string());
        statements.push("CREATE TABLE shelf_defined_only (id STRING HASH KEY);".to_string());
        session
            .engine
            .execute_script(&statements.join("\n"))
            .unwrap();

        let orders = session.engine.cached_definition("orders").unwrap();
        let defined_only = session
            .engine
            .cached_definition("shelf_defined_only")
            .unwrap();
        let mut names = session.engine.cached_table_names(false).unwrap();
        names.retain(|name| name != "shelf_defined_only");

        let dir = tempfile::tempdir().unwrap();
        let cache = crate::table_cache::DiskTableCache::open_at(
            dir.path().join("cache"),
            crate::table_cache::CacheEndpoint {
                region: "us-west-1".to_string(),
                host: None,
                port: 8000,
                allow_select_scan: session.config.allow_select_scan,
            },
        );
        cache.put_names(&names);
        cache.put_definition(&orders);
        cache.put_definition(&defined_only);
        session.engine.set_table_cache(Some(cache.clone()));
        session.table_disk = Some(cache);

        assert!(session.engine.cached_definition("bookshelf").is_none());
        assert!(session.engine.cached_definition("shelf_a").is_none());
        assert!(session
            .engine
            .cached_definition("shelf_defined_only")
            .is_some());

        let found = search(&mut session, dir.path(), "shelf");
        let tables: Vec<_> = found
            .items
            .iter()
            .filter(|item| item.kind == "table")
            .map(|item| item.title.as_str())
            .collect();
        let mut expected: Vec<String> = ('a'..='j')
            .map(|suffix| format!("shelf_{suffix}"))
            .collect();
        expected.push("bookshelf".to_string());
        expected.push("pre_shelf_post".to_string());
        assert_eq!(
            tables,
            expected.iter().map(String::as_str).collect::<Vec<_>>()
        );

        let listed = crate::meta::ls::web_table_list(&mut session, "shelf", false, false).unwrap();
        let listed_names: Vec<_> = listed
            .tables
            .iter()
            .map(|table| table.name.as_str())
            .collect();
        let mut web_expected: Vec<String> = ('a'..='j')
            .map(|suffix| format!("shelf_{suffix}"))
            .collect();
        web_expected.push("pre_shelf_post".to_string());
        web_expected.push("bookshelf".to_string());
        web_expected.push("orders".to_string());
        assert_eq!(
            listed_names,
            web_expected.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert!(listed
            .tables
            .iter()
            .find(|table| table.name == "bookshelf")
            .unwrap()
            .keys
            .is_empty());
        assert_eq!(
            listed
                .tables
                .iter()
                .find(|table| table.name == "orders")
                .unwrap()
                .keys[0]
                .name,
            "shelf_id"
        );
        assert!(!listed_names.contains(&"shelf_defined_only"));
        assert!(!listed_names.contains(&"gamma"));
        assert!(listed.note.contains("similar names and related keys"));
    }

    #[test]
    fn file_name_match_opens_the_file_when_no_statement_matches() {
        let dir = tempfile::tempdir().expect("temp");
        std::fs::create_dir(dir.path().join("queries")).expect("dir");
        std::fs::write(dir.path().join("queries/read.dql"), "SELECT 1;\n").expect("write");
        let hits = file_hits(dir.path(), "read.dql");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "query");
        assert_eq!(hits[0].path.as_deref(), Some("queries/read.dql"));
        assert_eq!(hits[0].offset, Some(0));
    }
}
