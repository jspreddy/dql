//! Encrypted definitions for `CREATE LOCALTABLE`.
//!
//! The file is the table cache key's AES-256-GCM seal. Temp tables are not written here.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dql_engine::{LocalColumn, LocalDefinition, LocalDefinitionStore};
use serde::{Deserialize, Serialize};

use crate::cache_seal;
use crate::table_cache::cache_root;

#[derive(Serialize, Deserialize)]
struct StoredDefinition {
    name: String,
    path: String,
    columns: Vec<StoredColumn>,
}

#[derive(Serialize, Deserialize)]
struct StoredColumn {
    name: String,
    data_type: String,
    key: String,
}

pub struct SealedLocalStore {
    path: PathBuf,
}

impl SealedLocalStore {
    pub fn at(path: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self { path: path.into() })
    }
}

impl LocalDefinitionStore for SealedLocalStore {
    fn load(&self) -> Vec<LocalDefinition> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(_) => return Vec::new(),
        };
        let Ok(key) = cache_seal::cache_key() else {
            return Vec::new();
        };
        let Ok(plain) = cache_seal::open(&key, &bytes) else {
            return Vec::new();
        };
        let stored: Vec<StoredDefinition> = match serde_json::from_slice(&plain) {
            Ok(stored) => stored,
            Err(_) => return Vec::new(),
        };
        stored
            .into_iter()
            .map(|definition| LocalDefinition {
                name: definition.name,
                path: definition.path,
                columns: definition
                    .columns
                    .into_iter()
                    .map(|column| LocalColumn {
                        name: column.name,
                        data_type: column.data_type,
                        key: column.key,
                    })
                    .collect(),
            })
            .collect()
    }

    fn save(&self, definitions: &[LocalDefinition]) {
        let Ok(key) = cache_seal::cache_key() else {
            return;
        };
        let stored: Vec<StoredDefinition> = definitions
            .iter()
            .map(|definition| StoredDefinition {
                name: definition.name.clone(),
                path: definition.path.clone(),
                columns: definition
                    .columns
                    .iter()
                    .map(|column| StoredColumn {
                        name: column.name.clone(),
                        data_type: column.data_type.clone(),
                        key: column.key.clone(),
                    })
                    .collect(),
            })
            .collect();
        let Ok(plain) = serde_json::to_vec(&stored) else {
            return;
        };
        let Ok(sealed) = cache_seal::seal(&key, &plain) else {
            return;
        };
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = cache_seal::ensure_owner_dir(parent);
            }
        }
        let _ = fs::write(&self.path, sealed);
    }
}

/// Store used by a session. Tests get a fresh file so they do not share definitions.
pub fn open_store() -> Arc<SealedLocalStore> {
    if let Some(path) = std::env::var_os("DQLRS_LOCAL_TABLES_FILE") {
        return SealedLocalStore::at(PathBuf::from(path));
    }
    if cfg!(test) {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("dqlrs-local-test-{}-{}", std::process::id(), id));
        return SealedLocalStore::at(dir.join("local-tables.seal"));
    }
    SealedLocalStore::at(cache_root().join("local-tables.seal"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_file_hides_the_path_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("dqlrs-local-seal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("local-tables.seal");
        let store = SealedLocalStore::at(&path);
        let definitions = vec![LocalDefinition {
            name: "users".to_string(),
            path: "/tmp/users.csv".to_string(),
            columns: vec![LocalColumn {
                name: "id".to_string(),
                data_type: "STRING".to_string(),
                key: "hash".to_string(),
            }],
        }];
        store.save(&definitions);
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(5).any(|window| window == b"users"));
        assert!(!bytes.windows(9).any(|window| window == b"users.csv"));
        let loaded = store.load();
        assert_eq!(loaded, definitions);
        let _ = fs::remove_dir_all(&dir);
    }
}
