//! Workspace tree of `.dql` files. Paths cannot escape the root.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// One file at a time. Web saves compare the revision under this lock so two
/// windows cannot both pass the check and then overwrite each other.
static FILE_IO: Mutex<()> = Mutex::new(());

fn file_io() -> std::sync::MutexGuard<'static, ()> {
    FILE_IO.lock().unwrap_or_else(|err| err.into_inner())
}

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".venv",
    "venv",
    "node_modules",
    "target",
    "__pycache__",
    ".dynamo-local",
    "dist",
    ".pytest_cache",
    "mockups",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError(pub String);

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PathError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Node {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
}

pub fn resolve_inside(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let raw = relative.trim().replace('\\', "/");
    if raw.starts_with('/') || raw.starts_with('~') {
        return Err(PathError("path must stay inside the workspace".to_string()));
    }
    let root = canonical_root(root)?;
    let mut candidate = root.clone();
    if !raw.is_empty() && raw != "." {
        for part in raw.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                if candidate == root {
                    return Err(PathError("path must stay inside the workspace".to_string()));
                }
                candidate.pop();
                continue;
            }
            candidate.push(part);
        }
    }
    if candidate.exists() {
        let canon = candidate
            .canonicalize()
            .map_err(|err| PathError(err.to_string()))?;
        if canon != root && !canon.starts_with(&root) {
            return Err(PathError("path must stay inside the workspace".to_string()));
        }
        return Ok(canon);
    }
    if !candidate.starts_with(&root) {
        return Err(PathError("path must stay inside the workspace".to_string()));
    }
    Ok(candidate)
}

pub fn list_dql_files(root: &Path) -> Result<Vec<(String, String)>, PathError> {
    let node = tree(root)?;
    let mut files = Vec::new();
    collect_dql_files(&node, &mut files);
    files.sort();
    Ok(files)
}

fn collect_dql_files(node: &Node, files: &mut Vec<(String, String)>) {
    if node.kind == "file" {
        files.push((node.path.clone(), node.name.clone()));
    }
    if let Some(children) = &node.children {
        for child in children {
            collect_dql_files(child, files);
        }
    }
}

pub fn tree(root: &Path) -> Result<Node, PathError> {
    fs::create_dir_all(root).map_err(|err| PathError(err.to_string()))?;
    let root = canonical_root(root)?;
    let (node, _hidden) = dir_node(&root, &root, true)?;
    node.ok_or_else(|| PathError("workspace is empty".to_string()))
}

pub fn make_dir(root: &Path, relative: &str) -> Result<String, PathError> {
    let path = dir_path(root, relative)?;
    if path.exists() {
        return Err(PathError(format!("{} already exists", display(relative))));
    }
    fs::create_dir_all(&path).map_err(|err| PathError(err.to_string()))?;
    rel(root, &path)
}

pub fn move_path(
    root: &Path,
    source: &str,
    dest: &str,
) -> Result<(String, &'static str), PathError> {
    let src = existing(root, source)?;
    let kind = if src.is_dir() { "dir" } else { "file" };
    let dst = if kind == "dir" {
        dir_path(root, dest)?
    } else {
        dql_path(root, dest)?
    };
    if dst == src {
        return Ok((rel(root, &src)?, kind));
    }
    if kind == "dir" && is_inside(&src, &dst) {
        return Err(PathError("cannot move a folder into itself".to_string()));
    }
    if dst.exists() {
        return Err(PathError(format!("{} already exists", rel(root, &dst)?)));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|err| PathError(err.to_string()))?;
    }
    fs::rename(&src, &dst).map_err(|err| PathError(err.to_string()))?;
    Ok((rel(root, &dst)?, kind))
}

pub fn duplicate_path(root: &Path, source: &str) -> Result<(String, &'static str), PathError> {
    let src = existing(root, source)?;
    let kind = if src.is_dir() { "dir" } else { "file" };
    let dest = unique_copy(&src);
    if kind == "file" {
        fs::copy(&src, &dest).map_err(|err| PathError(err.to_string()))?;
    } else {
        copy_tree(&src, &dest)?;
    }
    Ok((rel(root, &dest)?, kind))
}

pub fn delete_path(root: &Path, relative: &str) -> Result<(), PathError> {
    let path = existing(root, relative)?;
    if path.is_file() {
        if !is_dql(&path) {
            return Err(PathError("only .dql files can be deleted".to_string()));
        }
        fs::remove_file(path).map_err(|err| PathError(err.to_string()))?;
        return Ok(());
    }
    if let Some(blocked) = foreign_entry(&path)? {
        return Err(PathError(format!(
            "folder contains {blocked}, which is not a .dql file"
        )));
    }
    fs::remove_dir_all(path).map_err(|err| PathError(err.to_string()))?;
    Ok(())
}

pub fn read_text(root: &Path, relative: &str) -> Result<String, PathError> {
    let _guard = file_io();
    read_text_unlocked(root, relative)
}

fn read_text_unlocked(root: &Path, relative: &str) -> Result<String, PathError> {
    let path = resolve_inside(root, relative)?;
    if !path.is_file() {
        return Err(PathError(format!("{relative} is not a file")));
    }
    if !is_dql(&path) {
        return Err(PathError("only .dql files can be opened".to_string()));
    }
    fs::read_to_string(path).map_err(|err| PathError(err.to_string()))
}

pub fn is_missing(err: &PathError) -> bool {
    err.0.contains("is not a file")
}

/// SHA-256 of the file text. The web client sends this back as `base` on save.
pub fn content_revision(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFile {
    pub path: String,
    pub text: String,
    pub revision: String,
    pub missing: bool,
}

pub enum SaveFile {
    Stored(DiskFile),
    Conflict(DiskFile),
}

pub fn read_file(root: &Path, relative: &str) -> Result<DiskFile, PathError> {
    let text = read_text(root, relative)?;
    Ok(present(relative, &text))
}

/// Write `text` when `base` is the current revision.
///
/// `base == None` writes unconditionally, which is what MCP and older clients do.
/// A present `base` that does not match leaves the file untouched.
pub fn save_file(
    root: &Path,
    relative: &str,
    text: &str,
    base: Option<&str>,
) -> Result<SaveFile, PathError> {
    let _guard = file_io();
    match read_text_unlocked(root, relative) {
        Ok(current) => {
            if let Some(expected) = base {
                if content_revision(&current) != expected {
                    return Ok(SaveFile::Conflict(present(relative, &current)));
                }
            }
        }
        Err(err) if is_missing(&err) => {
            if base.is_some() {
                return Ok(SaveFile::Conflict(absent(relative)));
            }
        }
        Err(err) => return Err(err),
    }
    write_text_unlocked(root, relative, text)?;
    Ok(SaveFile::Stored(present(relative, text)))
}

fn present(relative: &str, text: &str) -> DiskFile {
    DiskFile {
        path: relative.to_string(),
        text: text.to_string(),
        revision: content_revision(text),
        missing: false,
    }
}

fn absent(relative: &str) -> DiskFile {
    DiskFile {
        path: relative.to_string(),
        text: String::new(),
        revision: "missing".to_string(),
        missing: true,
    }
}

pub fn normalized_dql(root: &Path, relative: &str) -> Result<String, PathError> {
    let path = dql_path(root, relative)?;
    let root = canonical_root(root)?;
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| PathError("path must stay inside the workspace".to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub fn write_text(root: &Path, relative: &str, text: &str) -> Result<(), PathError> {
    let _guard = file_io();
    write_text_unlocked(root, relative, text)
}

fn write_text_unlocked(root: &Path, relative: &str, text: &str) -> Result<(), PathError> {
    let path = dql_path(root, relative)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| PathError(err.to_string()))?;
    }
    fs::write(path, text).map_err(|err| PathError(err.to_string()))
}

pub fn create_file(root: &Path, relative: &str) -> Result<String, PathError> {
    let path = dql_path(root, relative)?;
    if path.exists() {
        return Err(PathError(format!("{relative} already exists")));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| PathError(err.to_string()))?;
    }
    fs::write(&path, "").map_err(|err| PathError(err.to_string()))?;
    rel(root, &path)
}

fn canonical_root(root: &Path) -> Result<PathBuf, PathError> {
    root.canonicalize()
        .map_err(|err| PathError(err.to_string()))
}

fn dql_path(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let mut cleaned = relative.trim().replace('\\', "/");
    cleaned = cleaned.trim_start_matches('/').to_string();
    if cleaned.is_empty() || cleaned.ends_with('/') {
        return Err(PathError("choose a file name ending in .dql".to_string()));
    }
    if !cleaned.to_ascii_lowercase().ends_with(".dql") {
        cleaned.push_str(".dql");
    }
    resolve_inside(root, &cleaned)
}

fn dir_path(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    resolve_inside(root, &clean_relative(relative)?)
}

fn clean_relative(relative: &str) -> Result<String, PathError> {
    let cleaned = relative.trim().replace('\\', "/");
    let cleaned = cleaned.trim_matches('/');
    if cleaned.is_empty() {
        return Err(PathError("choose a name".to_string()));
    }
    if cleaned
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
    {
        return Err(PathError("invalid name".to_string()));
    }
    Ok(cleaned.to_string())
}

fn dir_node(
    root: &Path,
    directory: &Path,
    is_root: bool,
) -> Result<(Option<Node>, bool), PathError> {
    let mut children = Vec::new();
    let mut hidden = false;
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(|err| PathError(err.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| PathError(err.to_string()))?;
    entries.sort_by(|left, right| {
        let left_dir = left.path().is_dir();
        let right_dir = right.path().is_dir();
        right_dir.cmp(&left_dir).then_with(|| {
            left.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.file_name().to_string_lossy().to_lowercase())
        })
    });
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || (path.is_dir() && SKIP_DIRS.contains(&name.as_str())) {
            continue;
        }
        if path.is_dir() {
            let (node, child_hidden) = dir_node(root, &path, false)?;
            hidden |= child_hidden;
            if let Some(node) = node {
                children.push(node);
            }
            continue;
        }
        if path.is_file() && is_dql(&path) {
            children.push(Node {
                kind: "file",
                name,
                path: rel(root, &path)?,
                children: None,
            });
        } else if path.is_file() {
            hidden = true;
        }
    }
    if children.is_empty() && hidden && !is_root {
        return Ok((None, true));
    }
    let path = rel(root, directory).unwrap_or_else(|_| ".".to_string());
    let name = if path == "." {
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string())
    } else {
        directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    Ok((
        Some(Node {
            kind: "dir",
            name,
            path,
            children: Some(children),
        }),
        hidden,
    ))
}

fn existing(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let root = canonical_root(root)?;
    let path = resolve_inside(&root, relative)?;
    if path == root || !path.exists() || !(path.is_file() || path.is_dir()) {
        return Err(PathError(
            "choose a file or folder in the workspace".to_string(),
        ));
    }
    Ok(path)
}

fn rel(root: &Path, path: &Path) -> Result<String, PathError> {
    let root = canonical_root(root)?;
    let path = if path.exists() {
        path.canonicalize()
            .map_err(|err| PathError(err.to_string()))?
    } else {
        path.to_path_buf()
    };
    if path == root {
        return Ok(".".to_string());
    }
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| PathError("path must stay inside the workspace".to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn display(relative: &str) -> String {
    let cleaned = relative.trim().replace('\\', "/");
    let cleaned = cleaned.trim_matches('/');
    if cleaned.is_empty() {
        "that path".to_string()
    } else {
        cleaned.to_string()
    }
}

fn is_inside(parent: &Path, child: &Path) -> bool {
    let parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    let child = if child.exists() {
        child.canonicalize().unwrap_or_else(|_| child.to_path_buf())
    } else {
        child.to_path_buf()
    };
    child == parent || child.starts_with(&parent)
}

fn unique_copy(src: &Path) -> PathBuf {
    let (stem, suffix) = if src.is_file() {
        (
            src.file_stem().and_then(|s| s.to_str()).unwrap_or("copy"),
            src.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!(".{ext}"))
                .unwrap_or_default(),
        )
    } else {
        (
            src.file_name().and_then(|s| s.to_str()).unwrap_or("copy"),
            String::new(),
        )
    };
    let parent = src.parent().unwrap_or(Path::new("."));
    let mut candidate = parent.join(format!("{stem} copy{suffix}"));
    let mut number = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{stem} copy {number}{suffix}"));
        number += 1;
    }
    candidate
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), PathError> {
    fs::create_dir_all(dest).map_err(|err| PathError(err.to_string()))?;
    for entry in fs::read_dir(src).map_err(|err| PathError(err.to_string()))? {
        let entry = entry.map_err(|err| PathError(err.to_string()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || SKIP_DIRS.contains(&name_str.as_ref()) {
            continue;
        }
        let path = entry.path();
        let target = dest.join(&name);
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else if path.is_file() && is_dql(&path) {
            fs::copy(path, target).map_err(|err| PathError(err.to_string()))?;
        }
    }
    Ok(())
}

fn foreign_entry(path: &Path) -> Result<Option<String>, PathError> {
    for entry in fs::read_dir(path).map_err(|err| PathError(err.to_string()))? {
        let entry = entry.map_err(|err| PathError(err.to_string()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            return Ok(Some(name));
        }
        let child = entry.path();
        if child.is_dir() {
            if let Some(found) = foreign_entry(&child)? {
                return Ok(Some(found));
            }
        } else if !child.is_file() || !is_dql(&child) {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn is_dql(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("dql")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tree_nests_dql_files_and_rejects_escape() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("queries")).unwrap();
        fs::write(root.path().join("queries/posts.dql"), "SCAN * FROM t;\n").unwrap();
        fs::create_dir(root.path().join("empty")).unwrap();
        fs::write(root.path().join("notes.txt"), "nope").unwrap();
        fs::create_dir(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/main.py"), "x").unwrap();
        let node = tree(root.path()).unwrap();
        let names: Vec<_> = node
            .children
            .unwrap()
            .iter()
            .map(|child| child.name.clone())
            .collect();
        assert_eq!(names, ["empty", "queries"]);
        let again = tree(root.path()).unwrap();
        let queries = again
            .children
            .unwrap()
            .into_iter()
            .find(|child| child.name == "queries")
            .unwrap();
        assert_eq!(queries.kind, "dir");
        assert_eq!(queries.children.unwrap()[0].path, "queries/posts.dql");
        assert!(resolve_inside(root.path(), "../secret.dql").is_err());
    }

    #[test]
    fn move_duplicate_and_delete() {
        let root = tempfile::tempdir().unwrap();
        create_file(root.path(), "queries/posts.dql").unwrap();
        write_text(root.path(), "queries/posts.dql", "SCAN * FROM t;\n").unwrap();
        assert_eq!(make_dir(root.path(), "archive").unwrap(), "archive");
        let names: Vec<_> = tree(root.path())
            .unwrap()
            .children
            .unwrap()
            .into_iter()
            .map(|child| child.name)
            .collect();
        assert!(names.iter().any(|name| name == "archive"));
        assert_eq!(
            move_path(root.path(), "queries/posts.dql", "archive/posts.dql").unwrap(),
            ("archive/posts.dql".to_string(), "file")
        );
        assert_eq!(
            read_text(root.path(), "archive/posts.dql").unwrap(),
            "SCAN * FROM t;\n"
        );
        assert_eq!(
            duplicate_path(root.path(), "archive/posts.dql").unwrap(),
            ("archive/posts copy.dql".to_string(), "file")
        );
        assert_eq!(
            duplicate_path(root.path(), "archive/posts.dql").unwrap().0,
            "archive/posts copy 2.dql"
        );
        assert_eq!(
            duplicate_path(root.path(), "archive").unwrap(),
            ("archive copy".to_string(), "dir")
        );
        assert!(root.path().join("archive copy/posts.dql").is_file());
        assert!(move_path(root.path(), "archive", "archive/nested").is_err());
        assert!(move_path(root.path(), "archive/posts.dql", "../out.dql").is_err());
        fs::write(root.path().join("archive/notes.txt"), "keep").unwrap();
        assert!(delete_path(root.path(), "archive").is_err());
        fs::remove_file(root.path().join("archive/notes.txt")).unwrap();
        delete_path(root.path(), "archive/posts copy.dql").unwrap();
        assert!(!root.path().join("archive/posts copy.dql").exists());
        delete_path(root.path(), "archive copy").unwrap();
        assert!(!root.path().join("archive copy").exists());
    }

    #[test]
    fn create_and_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        let path = create_file(root.path(), "queries/new").unwrap();
        assert_eq!(path, "queries/new.dql");
        write_text(root.path(), &path, "ls;\n").unwrap();
        assert_eq!(read_text(root.path(), &path).unwrap(), "ls;\n");
    }

    #[test]
    fn save_file_keeps_a_newer_disk_copy() {
        let root = tempfile::tempdir().unwrap();
        let path = create_file(root.path(), "notes.dql").unwrap();
        let stored = save_file(root.path(), &path, "SELECT 1;\n", None).unwrap();
        let SaveFile::Stored(first) = stored else {
            panic!("first write should store");
        };
        write_text(root.path(), &path, "SELECT 2;\n").unwrap();
        let stale = save_file(root.path(), &path, "SELECT 3;\n", Some(&first.revision)).unwrap();
        let SaveFile::Conflict(current) = stale else {
            panic!("stale base should conflict");
        };
        assert_eq!(current.text, "SELECT 2;\n");
        assert!(!current.missing);
        assert_eq!(read_text(root.path(), &path).unwrap(), "SELECT 2;\n");
        let saved = save_file(root.path(), &path, "SELECT 3;\n", Some(&current.revision)).unwrap();
        let SaveFile::Stored(next) = saved else {
            panic!("current base should store");
        };
        assert_eq!(next.text, "SELECT 3;\n");
        assert_eq!(read_text(root.path(), &path).unwrap(), "SELECT 3;\n");

        fs::remove_file(root.path().join(&path)).unwrap();
        let gone = save_file(root.path(), &path, "SELECT 3;\n", Some(&next.revision)).unwrap();
        let SaveFile::Conflict(missing) = gone else {
            panic!("deleted file should conflict");
        };
        assert!(missing.missing);
        assert!(!root.path().join(&path).exists());
    }
}
