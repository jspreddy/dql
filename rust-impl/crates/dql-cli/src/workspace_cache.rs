//! Everything dqlrs saves for a workspace lives in `<workspace>/.dqlrs-cache/`.
//!
//! The workspace is the folder dqlrs was started in, or `--dir`. Each connection gets its own
//! folder, named by the SHA-256 of its key (region, endpoint, and caller identity):
//!
//! ```text
//! <workspace>/.dqlrs-cache/
//!   .gitignore                      "*", so nothing here is committed
//!   <sha256 of connection key>/
//!     tables/                       table list and DynamoDB definitions
//!     local-tables.seal             CREATE LOCALTABLE definitions
//!     results/<sha256 of .dql path>/  web query results for that file
//! ```
//!
//! Files are sealed with the cache key and directories are `0700`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::cache_seal;

pub const DIR: &str = ".dqlrs-cache";

const GITIGNORE: &str =
    "# dqlrs workspace cache: machine-specific and encrypted. Do not commit.\n*\n";

/// `<workspace>/.dqlrs-cache`.
pub fn root(workspace: &Path) -> PathBuf {
    workspace.join(DIR)
}

/// Create `.dqlrs-cache` (`0700`) with a `.gitignore` that ignores everything in it.
pub fn ensure_root(workspace: &Path) -> io::Result<PathBuf> {
    let root = root(workspace);
    cache_seal::ensure_owner_dir(&root)?;
    let ignore = root.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, GITIGNORE)?;
    }
    Ok(root)
}

/// The folder for one connection, `<workspace>/.dqlrs-cache/<sha256 of key>`. Not created.
pub fn connection_dir(workspace: &Path, connection_key: &str) -> PathBuf {
    root(workspace).join(sha256_hex(connection_key))
}

/// Create the connection folder and its parents, each `0700`.
pub fn ensure_connection_dir(workspace: &Path, connection_key: &str) -> io::Result<PathBuf> {
    ensure_root(workspace)?;
    let dir = connection_dir(workspace, connection_key);
    cache_seal::ensure_owner_dir(&dir)?;
    Ok(dir)
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_connection_gets_its_own_folder_under_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let local =
            ensure_connection_dir(workspace.path(), "local|us-west-1|localhost|8000|a").unwrap();
        let other = connection_dir(workspace.path(), "local|us-west-1|localhost|8000|b");
        assert_ne!(local, other);
        assert_eq!(
            local.parent().unwrap(),
            workspace.path().join(".dqlrs-cache")
        );
        assert!(local.is_dir());
        assert!(!other.exists());
    }

    #[test]
    fn the_cache_folder_ignores_itself_and_is_private() {
        let workspace = tempfile::tempdir().unwrap();
        let root = ensure_root(workspace.path()).unwrap();
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(ignore.lines().any(|line| line == "*"), "{ignore}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        // A second call leaves an edited .gitignore alone.
        fs::write(root.join(".gitignore"), "*\n!keep\n").unwrap();
        ensure_root(workspace.path()).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "*\n!keep\n"
        );
    }
}
