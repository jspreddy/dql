//! One directory per query file: `workspace/.dqlrs-web-cache/<sha256 of the workspace-relative path>/`.
//! Each `query_result_<unix-ms>.json` is a single statement:
//! `{ "query": "<one statement>", "result": { ... }, "success": bool }`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::files::{self, PathError};

const MAX_RESULTS: usize = 50;
const CACHE_DIR: &str = ".dqlrs-web-cache";

/// One executed statement. `result` is that statement's result object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredQuery {
    pub query: String,
    pub result: Value,
    pub success: bool,
}

pub fn list(root: &Path, relative: &str) -> Result<Vec<StoredQuery>, PathError> {
    let dir = cache_dir(root, relative)?;
    let mut files = result_files(&dir)?;
    files.sort_by_key(|(stamp, _)| *stamp);
    let mut stored = Vec::with_capacity(files.len());
    for (_, path) in files {
        let text = fs::read_to_string(&path).map_err(|err| PathError(err.to_string()))?;
        let Ok(entry) = serde_json::from_str::<StoredQuery>(&text) else {
            continue;
        };
        stored.push(entry);
    }
    Ok(stored)
}

pub fn save(
    root: &Path,
    relative: &str,
    query: &str,
    result: Value,
    success: bool,
) -> Result<StoredQuery, PathError> {
    let dir = cache_dir(root, relative)?;
    fs::create_dir_all(&dir).map_err(|err| PathError(err.to_string()))?;
    let stored = StoredQuery {
        query: query.to_string(),
        result,
        success,
    };
    let body = serde_json::to_string(&stored).map_err(|err| PathError(err.to_string()))?;
    let mut stamp = unix_ms();
    let path = loop {
        let path = dir.join(format!("query_result_{stamp}.json"));
        if !path.exists() {
            break path;
        }
        stamp += 1;
    };
    fs::write(&path, body).map_err(|err| PathError(err.to_string()))?;
    prune(&dir)?;
    Ok(stored)
}

pub fn clear(root: &Path, relative: &str) -> Result<(), PathError> {
    let dir = cache_dir(root, relative)?;
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|err| PathError(err.to_string()))?;
    }
    Ok(())
}

/// Drop the cache directories for a file, or for every `.dql` file under a folder.
/// The directory name is the sha256 of the workspace-relative path.
pub fn clear_at(root: &Path, relative: &str) -> Result<Vec<String>, PathError> {
    let paths = covered_paths(root, relative)?;
    for path in &paths {
        clear(root, path)?;
    }
    Ok(paths)
}

fn covered_paths(root: &Path, relative: &str) -> Result<Vec<String>, PathError> {
    let target = relative.trim().trim_matches('/');
    let target = if target.is_empty() { "." } else { target };
    let tree = files::tree(root)?;
    let node = find_node(&tree, target)
        .ok_or_else(|| PathError("choose a file or folder in the workspace".to_string()))?;
    let mut paths = Vec::new();
    collect_files(node, &mut paths);
    Ok(paths)
}

fn find_node<'a>(node: &'a files::Node, target: &str) -> Option<&'a files::Node> {
    if node.path == target {
        return Some(node);
    }
    for child in node.children.as_deref().unwrap_or(&[]) {
        if let Some(found) = find_node(child, target) {
            return Some(found);
        }
    }
    None
}

fn collect_files(node: &files::Node, paths: &mut Vec<String>) {
    if node.kind == "file" {
        paths.push(node.path.clone());
        return;
    }
    for child in node.children.as_deref().unwrap_or(&[]) {
        collect_files(child, paths);
    }
}

fn cache_dir(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let relative = files::normalized_dql(root, relative)?;
    let digest = Sha256::digest(relative.as_bytes());
    Ok(root.join(CACHE_DIR).join(hex_encode(&digest)))
}

fn result_files(dir: &Path) -> Result<Vec<(u128, PathBuf)>, PathError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| PathError(err.to_string()))? {
        let entry = entry.map_err(|err| PathError(err.to_string()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stamp) = name
            .strip_prefix("query_result_")
            .and_then(|rest| rest.strip_suffix(".json"))
            .and_then(|rest| rest.parse::<u128>().ok())
        else {
            continue;
        };
        files.push((stamp, entry.path()));
    }
    Ok(files)
}

fn prune(dir: &Path) -> Result<(), PathError> {
    let mut files = result_files(dir)?;
    files.sort_by_key(|(stamp, _)| *stamp);
    let extra = files.len().saturating_sub(MAX_RESULTS);
    for (_, path) in files.into_iter().take(extra) {
        fs::remove_file(path).map_err(|err| PathError(err.to_string()))?;
    }
    Ok(())
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stores_each_file_separately_and_keeps_the_latest_50() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("notes.dql"), "").unwrap();
        fs::create_dir(root.path().join("queries")).unwrap();
        fs::write(root.path().join("queries/other.dql"), "").unwrap();

        for index in 0..51 {
            save(
                root.path(),
                "notes.dql",
                &format!("SELECT {index};"),
                json!({"statement": format!("SELECT {index};"), "ok": index != 3}),
                index != 3,
            )
            .unwrap();
        }
        save(
            root.path(),
            "queries/other.dql",
            "SELECT 'other';",
            json!({"statement": "SELECT 'other';", "ok": true}),
            true,
        )
        .unwrap();

        let notes = list(root.path(), "notes.dql").unwrap();
        assert_eq!(notes.len(), 50);
        assert_eq!(notes[0].query, "SELECT 1;");
        assert_eq!(notes[49].query, "SELECT 50;");
        assert!(!notes.iter().any(|entry| entry.query == "SELECT 0;"));
        assert!(!notes[2].success);
        assert!(notes[2].result.is_object());
        assert_eq!(notes[2].result["ok"], json!(false));

        let names: Vec<_> = fs::read_dir(cache_dir(root.path(), "notes.dql").unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|name| {
            name.strip_prefix("query_result_")
                .and_then(|rest| rest.strip_suffix(".json"))
                .and_then(|rest| rest.parse::<u128>().ok())
                .is_some()
        }));
        let notes_dir = cache_dir(root.path(), "notes.dql").unwrap();
        assert_eq!(
            notes_dir.parent().and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new(CACHE_DIR))
        );
        assert!(!root.path().join(".cache").exists());
        let hash = notes_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));

        let other = list(root.path(), "queries/other.dql").unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].query, "SELECT 'other';");
        assert!(other[0].success);

        clear(root.path(), "notes.dql").unwrap();
        assert!(list(root.path(), "notes.dql").unwrap().is_empty());
        assert!(!cache_dir(root.path(), "notes.dql").unwrap().exists());
        assert_eq!(list(root.path(), "queries/other.dql").unwrap().len(), 1);
    }

    #[test]
    fn clear_at_removes_the_hashes_for_the_chosen_paths() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("queries")).unwrap();
        fs::create_dir(root.path().join("queries/nested")).unwrap();
        fs::write(root.path().join("queries/one.dql"), "").unwrap();
        fs::write(root.path().join("queries/nested/two.dql"), "").unwrap();
        fs::write(root.path().join("keep.dql"), "").unwrap();
        for path in ["queries/one.dql", "queries/nested/two.dql", "keep.dql"] {
            save(root.path(), path, "SELECT 1;", json!({"ok": true}), true).unwrap();
        }

        let cleared = clear_at(root.path(), "queries").unwrap();
        assert_eq!(
            cleared,
            vec![
                "queries/nested/two.dql".to_string(),
                "queries/one.dql".to_string(),
            ]
        );
        assert!(list(root.path(), "queries/one.dql").unwrap().is_empty());
        assert!(list(root.path(), "queries/nested/two.dql")
            .unwrap()
            .is_empty());
        assert!(!cache_dir(root.path(), "queries/one.dql").unwrap().exists());
        assert_eq!(list(root.path(), "keep.dql").unwrap().len(), 1);

        let one = clear_at(root.path(), "keep.dql").unwrap();
        assert_eq!(one, vec!["keep.dql".to_string()]);
        assert!(list(root.path(), "keep.dql").unwrap().is_empty());
    }
}
