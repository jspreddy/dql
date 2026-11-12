//! Web query results, per connection and per query file:
//! `<workspace>/.dqlrs-cache/<sha256 of connection key>/results/<sha256 of the workspace-relative path>/`.
//! Each `query_result_<unix-ms>.json` is one statement, sealed with AES-256-GCM.
//! The plaintext JSON is `{ "query": "<one statement>", "result": { ... }, "success": bool }`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::files::{self, PathError};
use crate::cache_seal;
use crate::workspace_cache;

const MAX_RESULTS: usize = 50;
const RESULTS_DIR: &str = "results";

/// One connection's results in one workspace.
#[derive(Clone, Copy, Debug)]
pub struct Store<'a> {
    pub workspace: &'a Path,
    pub connection_key: &'a str,
}

impl Store<'_> {
    /// `<workspace>/.dqlrs-cache/<connection>/results`.
    fn base(&self) -> PathBuf {
        workspace_cache::connection_dir(self.workspace, self.connection_key).join(RESULTS_DIR)
    }
}

/// One executed statement. `result` is that statement's result object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredQuery {
    pub query: String,
    pub result: Value,
    pub success: bool,
}

/// A stored statement plus the id the structure API uses to read it back.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ListedQuery {
    pub id: String,
    pub query: String,
    pub result: Value,
    pub success: bool,
}

pub fn list(store: Store<'_>, relative: &str) -> Result<Vec<ListedQuery>, PathError> {
    let dir = cache_dir(store, relative)?;
    let digest = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();
    let mut files = result_files(&dir)?;
    files.sort_by_key(|(stamp, _)| *stamp);
    restrict_query_dirs(store, &dir);
    let mut stored = Vec::with_capacity(files.len());
    for (stamp, path) in files {
        let Some(entry) = read_stored(&path) else {
            continue;
        };
        stored.push(ListedQuery {
            id: format!("{digest}:{stamp}"),
            query: entry.query,
            result: entry.result,
            success: entry.success,
        });
    }
    Ok(stored)
}

/// Load one cached statement by the id returned from [`list`].
pub fn load_by_id(store: Store<'_>, id: &str) -> Result<ListedQuery, PathError> {
    let (digest, stamp) = id
        .split_once(':')
        .ok_or_else(|| PathError("query result is not loaded".to_string()))?;
    if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(PathError("query result is not loaded".to_string()));
    }
    let stamp = stamp
        .parse::<u128>()
        .map_err(|_| PathError("query result is not loaded".to_string()))?;
    let path = store
        .base()
        .join(digest)
        .join(format!("query_result_{stamp}.json"));
    let entry =
        read_stored(&path).ok_or_else(|| PathError("query result is not loaded".to_string()))?;
    Ok(ListedQuery {
        id: id.to_string(),
        query: entry.query,
        result: entry.result,
        success: entry.success,
    })
}

fn read_stored(path: &Path) -> Option<StoredQuery> {
    let bytes = cache_seal::read_bytes(path)?;
    let text = String::from_utf8(bytes).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(
    store: Store<'_>,
    relative: &str,
    query: &str,
    result: Value,
    success: bool,
) -> Result<StoredQuery, PathError> {
    let dir = cache_dir(store, relative)?;
    ensure_query_dirs(store, &dir)?;
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
    cache_seal::write_sealed(&path, body.as_bytes()).map_err(PathError)?;
    prune(&dir)?;
    Ok(stored)
}

/// Create `.dqlrs-cache` (with its `.gitignore`), the connection folder, `results`, and the
/// file's folder, each `0700`.
fn ensure_query_dirs(store: Store<'_>, dir: &Path) -> Result<(), PathError> {
    let error = |err: std::io::Error| PathError(err.to_string());
    workspace_cache::ensure_connection_dir(store.workspace, store.connection_key).map_err(error)?;
    cache_seal::ensure_owner_dir(&store.base()).map_err(error)?;
    cache_seal::ensure_owner_dir(dir).map_err(error)
}

fn restrict_query_dirs(store: Store<'_>, dir: &Path) {
    let _ = cache_seal::restrict_dir(&store.base());
    let _ = cache_seal::restrict_dir(dir);
}

pub fn clear(store: Store<'_>, relative: &str) -> Result<(), PathError> {
    let dir = cache_dir(store, relative)?;
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|err| PathError(err.to_string()))?;
    }
    Ok(())
}

/// Drop this connection's result directories for a file, or for every `.dql` file under a
/// folder. The directory name is the sha256 of the workspace-relative path.
pub fn clear_at(store: Store<'_>, relative: &str) -> Result<Vec<String>, PathError> {
    let paths = covered_paths(store.workspace, relative)?;
    for path in &paths {
        clear(store, path)?;
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
        if node.path.to_ascii_lowercase().ends_with(".dql") {
            paths.push(node.path.clone());
        }
        return;
    }
    for child in node.children.as_deref().unwrap_or(&[]) {
        collect_files(child, paths);
    }
}

fn cache_dir(store: Store<'_>, relative: &str) -> Result<PathBuf, PathError> {
    let relative = files::normalized_dql(store.workspace, relative)?;
    let digest = Sha256::digest(relative.as_bytes());
    Ok(store.base().join(hex_encode(&digest)))
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

    const KEY: &str = "local|us-west-1|localhost|8000|test";

    fn store(workspace: &Path) -> Store<'_> {
        Store {
            workspace,
            connection_key: KEY,
        }
    }

    /// `results/<file>` sits in this connection's folder in `<workspace>/.dqlrs-cache`.
    fn assert_results_layout(workspace: &Path, dir: &Path) {
        let results = dir.parent().unwrap();
        assert_eq!(results.file_name(), Some(std::ffi::OsStr::new(RESULTS_DIR)));
        assert_eq!(
            results.parent().unwrap(),
            workspace_cache::connection_dir(workspace, KEY)
        );
        assert_eq!(
            results.parent().unwrap().parent().unwrap(),
            workspace.join(".dqlrs-cache")
        );
    }

    #[test]
    fn stores_each_file_separately_and_keeps_the_latest_50() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("notes.dql"), "").unwrap();
        fs::create_dir(root.path().join("queries")).unwrap();
        fs::write(root.path().join("queries/other.dql"), "").unwrap();

        for index in 0..51 {
            save(
                store(root.path()),
                "notes.dql",
                &format!("SELECT {index};"),
                json!({"statement": format!("SELECT {index};"), "ok": index != 3}),
                index != 3,
            )
            .unwrap();
        }
        save(
            store(root.path()),
            "queries/other.dql",
            "SELECT 'other';",
            json!({"statement": "SELECT 'other';", "ok": true}),
            true,
        )
        .unwrap();

        let notes = list(store(root.path()), "notes.dql").unwrap();
        assert_eq!(notes.len(), 50);
        assert_eq!(notes[0].query, "SELECT 1;");
        assert_eq!(notes[49].query, "SELECT 50;");
        assert!(!notes.iter().any(|entry| entry.query == "SELECT 0;"));
        assert!(!notes[2].success);
        assert!(notes[2].result.is_object());
        assert_eq!(notes[2].result["ok"], json!(false));

        let names: Vec<_> = fs::read_dir(cache_dir(store(root.path()), "notes.dql").unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|name| {
            name.strip_prefix("query_result_")
                .and_then(|rest| rest.strip_suffix(".json"))
                .and_then(|rest| rest.parse::<u128>().ok())
                .is_some()
        }));
        let notes_dir = cache_dir(store(root.path()), "notes.dql").unwrap();
        assert_results_layout(root.path(), &notes_dir);
        assert!(!root.path().join(".cache").exists());
        let hash = notes_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));

        let other = list(store(root.path()), "queries/other.dql").unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].query, "SELECT 'other';");
        assert!(other[0].success);
        let loaded = load_by_id(store(root.path()), &other[0].id).unwrap();
        assert_eq!(loaded.query, "SELECT 'other';");
        assert!(load_by_id(store(root.path()), "not-an-id").is_err());

        clear(store(root.path()), "notes.dql").unwrap();
        assert!(list(store(root.path()), "notes.dql").unwrap().is_empty());
        assert!(!cache_dir(store(root.path()), "notes.dql").unwrap().exists());
        assert_eq!(
            list(store(root.path()), "queries/other.dql").unwrap().len(),
            1
        );
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
            save(
                store(root.path()),
                path,
                "SELECT 1;",
                json!({"ok": true}),
                true,
            )
            .unwrap();
        }

        let cleared = clear_at(store(root.path()), "queries").unwrap();
        assert_eq!(
            cleared,
            vec![
                "queries/nested/two.dql".to_string(),
                "queries/one.dql".to_string(),
            ]
        );
        assert!(list(store(root.path()), "queries/one.dql")
            .unwrap()
            .is_empty());
        assert!(list(store(root.path()), "queries/nested/two.dql")
            .unwrap()
            .is_empty());
        assert!(!cache_dir(store(root.path()), "queries/one.dql")
            .unwrap()
            .exists());
        assert_eq!(list(store(root.path()), "keep.dql").unwrap().len(), 1);

        let one = clear_at(store(root.path()), "keep.dql").unwrap();
        assert_eq!(one, vec!["keep.dql".to_string()]);
        assert!(list(store(root.path()), "keep.dql").unwrap().is_empty());
    }

    #[test]
    fn seals_results_without_leaving_the_query_in_the_file() {
        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let before = crate::cache_seal::mode_bits(root.path());
        fs::write(root.path().join("notes.dql"), "").unwrap();
        let query = "SELECT marker_row FROM posts;";
        save(
            store(root.path()),
            "notes.dql",
            query,
            json!({"ok": true, "label": "alpha"}),
            true,
        )
        .unwrap();
        let listed = list(store(root.path()), "notes.dql").unwrap();
        assert_eq!(listed[0].query, query);
        assert_eq!(listed[0].result["label"], json!("alpha"));
        assert!(listed[0].success);
        let dir = cache_dir(store(root.path()), "notes.dql").unwrap();
        assert_results_layout(root.path(), &dir);
        let file = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
        let bytes = fs::read(&file).unwrap();
        assert_eq!(bytes[0], 1);
        assert!(bytes.len() >= 29);
        assert!(hides(&bytes, b"marker_row"));
        assert!(hides(&bytes, b"alpha"));
        assert!(hides(&bytes, b"SELECT"));
        #[cfg(unix)]
        {
            assert_eq!(crate::cache_seal::mode_bits(&file), 0o600);
            assert_eq!(crate::cache_seal::mode_bits(&dir), 0o700);
            assert_eq!(crate::cache_seal::mode_bits(dir.parent().unwrap()), 0o700);
            assert_eq!(crate::cache_seal::mode_bits(root.path()), before);
        }
    }

    #[test]
    fn plaintext_result_is_rewritten_in_place() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("notes.dql"), "").unwrap();
        let dir = cache_dir(store(root.path()), "notes.dql").unwrap();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("query_result_1.json");
        let body = r#"{"query":"SELECT marker_row;","result":{"ok":true},"success":true}"#;
        fs::write(&path, body).unwrap();
        let listed = list(store(root.path()), "notes.dql").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].query, "SELECT marker_row;");
        assert!(listed[0].success);
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes[0], 1);
        assert!(hides(&bytes, b"marker_row"));
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["query_result_1.json".to_string()]);
    }

    #[test]
    fn bad_ciphertext_is_skipped() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("notes.dql"), "").unwrap();
        save(
            store(root.path()),
            "notes.dql",
            "SELECT kept;",
            json!({"ok": true}),
            true,
        )
        .unwrap();
        let dir = cache_dir(store(root.path()), "notes.dql").unwrap();
        fs::write(dir.join("query_result_1.json"), vec![1u8; 40]).unwrap();
        let listed = list(store(root.path()), "notes.dql").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].query, "SELECT kept;");
    }

    #[test]
    fn each_connection_keeps_its_own_results_for_the_same_file() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("notes.dql"), "").unwrap();
        let local = store(root.path());
        let aws = Store {
            workspace: root.path(),
            connection_key: "aws|us-east-1|arn:aws:iam::123456789012:user/me",
        };
        save(
            local,
            "notes.dql",
            "SCAN * FROM local_only;",
            json!({"ok": true}),
            true,
        )
        .unwrap();
        save(
            aws,
            "notes.dql",
            "SCAN * FROM aws_only;",
            json!({"ok": true}),
            true,
        )
        .unwrap();

        let from_local: Vec<_> = list(local, "notes.dql")
            .unwrap()
            .into_iter()
            .map(|e| e.query)
            .collect();
        let from_aws: Vec<_> = list(aws, "notes.dql")
            .unwrap()
            .into_iter()
            .map(|e| e.query)
            .collect();
        assert_eq!(from_local, vec!["SCAN * FROM local_only;".to_string()]);
        assert_eq!(from_aws, vec!["SCAN * FROM aws_only;".to_string()]);

        // An id from one connection does not read the other connection's result.
        let id = list(local, "notes.dql").unwrap()[0].id.clone();
        assert!(load_by_id(aws, &id).is_err());

        clear_at(local, "notes.dql").unwrap();
        assert!(list(local, "notes.dql").unwrap().is_empty());
        assert_eq!(list(aws, "notes.dql").unwrap().len(), 1);

        let ignore = fs::read_to_string(root.path().join(".dqlrs-cache/.gitignore")).unwrap();
        assert!(ignore.lines().any(|line| line == "*"), "{ignore}");
    }

    fn hides(bytes: &[u8], secret: &[u8]) -> bool {
        !bytes.windows(secret.len()).any(|window| window == secret)
    }
}
