//! Disk cache for the table name list and definitions that have already been fetched.
//!
//! A refresh reloads those entries. It does not describe tables that have never
//! been fetched.

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use dql_engine::{CachedDefinition, CachedNameList, SharedTableCache, TableCacheStore};
use dql_models::{TableMeta, TableStatus};
use dql_parser::parse_statement;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::session::RuntimeEngine;

#[derive(Clone)]
pub struct CacheEndpoint {
    pub region: String,
    pub host: Option<String>,
    pub port: u16,
    pub allow_select_scan: bool,
}

pub struct DiskTableCache {
    dir: PathBuf,
    endpoint: CacheEndpoint,
    generation: AtomicU64,
    job_id: AtomicU64,
    refreshing: AtomicBool,
    last_error: Mutex<Option<String>>,
    lock: Mutex<()>,
    me: OnceLock<Weak<Self>>,
}

#[derive(Serialize, Deserialize)]
struct NameFile {
    saved_at_ms: u64,
    names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct DefinitionFile {
    saved_at_ms: u64,
    status: String,
    schema: String,
}

impl DiskTableCache {
    pub fn open(key: &str, endpoint: CacheEndpoint) -> Arc<Self> {
        Self::open_at(cache_root().join(sha256_hex(key)), endpoint)
    }

    pub fn open_at(dir: PathBuf, endpoint: CacheEndpoint) -> Arc<Self> {
        let cache = Arc::new(Self {
            dir,
            endpoint,
            generation: AtomicU64::new(1),
            job_id: AtomicU64::new(1),
            refreshing: AtomicBool::new(false),
            last_error: Mutex::new(None),
            lock: Mutex::new(()),
            me: OnceLock::new(),
        });
        let _ = cache.me.set(Arc::downgrade(&cache));
        cache
    }

    fn stored_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn last_error(&self) -> Option<String> {
        self.stored_error()
    }

    /// Starts a background refresh of the cached list and cached definitions.
    ///
    /// Returns false when there is nothing cached to refresh. Returns true when
    /// a refresh is already running.
    pub fn start_refresh(self: &Arc<Self>) -> bool {
        if !self.has_entries() {
            return false;
        }
        if self
            .refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return true;
        }
        let cache = Arc::clone(self);
        let job = self.job_id.load(Ordering::Acquire);
        std::thread::spawn(move || cache.refresh_job(job));
        true
    }

    fn has_entries(&self) -> bool {
        self.get_names().is_some() || !self.definition_names().is_empty()
    }

    fn refresh_job(self: Arc<Self>, job: u64) {
        REFRESH_JOB.with(|slot| slot.set(job));
        let result = self.refresh_from_endpoint();
        REFRESH_JOB.with(|slot| slot.set(0));
        if self.job_id.load(Ordering::Acquire) != job {
            return;
        }
        if let Ok(mut error) = self.last_error.lock() {
            *error = result.err();
        }
        self.refreshing.store(false, Ordering::Release);
    }

    fn refresh_from_endpoint(self: &Arc<Self>) -> Result<(), String> {
        let mut engine = RuntimeEngine::build(
            &self.endpoint.region,
            self.endpoint.host.as_deref(),
            self.endpoint.port,
            self.endpoint.allow_select_scan,
        )
        .map_err(|err| err.to_string())?;
        let store: SharedTableCache = Arc::clone(self) as SharedTableCache;
        engine.set_table_cache(Some(store));
        let had_list = self.get_names().is_some();
        let definitions = self.definition_names();
        if had_list {
            engine
                .cached_table_names(true)
                .map_err(|err| err.to_string())?;
        }
        for name in definitions {
            match engine
                .describe(&name, true)
                .map_err(|err| err.to_string())?
            {
                Some(_) => {}
                None => self.remove_definition(&name),
            }
        }
        Ok(())
    }
}

impl TableCacheStore for DiskTableCache {
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn refreshing(&self) -> bool {
        self.refreshing.load(Ordering::Acquire)
    }

    fn get_names(&self) -> Option<CachedNameList> {
        let _guard = self.lock.lock().ok()?;
        let text = fs::read_to_string(self.dir.join("list.json")).ok()?;
        let file: NameFile = serde_json::from_str(&text).ok()?;
        Some(CachedNameList {
            saved_at_ms: file.saved_at_ms,
            names: file.names,
        })
    }

    fn put_names(&self, names: &[String]) -> u64 {
        let saved_at_ms = now_ms();
        let _guard = self.lock.lock().expect("table cache");
        if !self.writes_allowed() {
            return saved_at_ms;
        }
        if fs::create_dir_all(&self.dir).is_ok() {
            let file = NameFile {
                saved_at_ms,
                names: names.to_vec(),
            };
            if let Ok(text) = serde_json::to_string(&file) {
                let _ = fs::write(self.dir.join("list.json"), text);
            }
        }
        self.bump();
        saved_at_ms
    }

    fn forget_names(&self) {
        let _guard = self.lock.lock().expect("table cache");
        let _ = fs::remove_file(self.dir.join("list.json"));
        self.bump();
    }

    fn get_definition(&self, name: &str) -> Option<CachedDefinition> {
        let _guard = self.lock.lock().ok()?;
        let text = fs::read_to_string(self.definition_path(name)).ok()?;
        let file: DefinitionFile = serde_json::from_str(&text).ok()?;
        let mut meta = meta_from_schema(&file.schema)?;
        meta.status = status_from_label(&file.status);
        Some(CachedDefinition {
            saved_at_ms: file.saved_at_ms,
            meta,
        })
    }

    fn put_definition(&self, meta: &TableMeta) -> u64 {
        let saved_at_ms = now_ms();
        let _guard = self.lock.lock().expect("table cache");
        if !self.writes_allowed() {
            return saved_at_ms;
        }
        let dir = self.dir.join("definitions");
        if fs::create_dir_all(&dir).is_ok() {
            let file = DefinitionFile {
                saved_at_ms,
                status: status_label(meta.status).to_string(),
                schema: meta.schema_dql(),
            };
            if let Ok(text) = serde_json::to_string(&file) {
                let _ = fs::write(self.definition_path(name_of(meta)), text);
            }
        }
        self.bump();
        saved_at_ms
    }

    fn remove_definition(&self, name: &str) {
        let _guard = self.lock.lock().expect("table cache");
        if !self.writes_allowed() {
            return;
        }
        let _ = fs::remove_file(self.definition_path(name));
        self.bump();
    }

    fn start_refresh(&self) -> bool {
        self.me
            .get()
            .and_then(Weak::upgrade)
            .is_some_and(|cache| DiskTableCache::start_refresh(&cache))
    }

    fn last_error(&self) -> Option<String> {
        self.stored_error()
    }

    fn clear(&self) {
        self.job_id.fetch_add(1, Ordering::AcqRel);
        let _guard = self.lock.lock().expect("table cache");
        let _ = fs::remove_file(self.dir.join("list.json"));
        if let Ok(entries) = fs::read_dir(self.dir.join("definitions")) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        self.bump();
        self.refreshing.store(false, Ordering::Release);
    }

    fn definition_names(&self) -> Vec<String> {
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let dir = self.dir.join("definitions");
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = serde_json::from_str::<DefinitionFile>(&text) else {
                continue;
            };
            if let Some(meta) = meta_from_schema(&file.schema) {
                names.push(meta.name);
            }
        }
        names.sort();
        names
    }
}

thread_local! {
    static REFRESH_JOB: Cell<u64> = const { Cell::new(0) };
}

impl DiskTableCache {
    fn writes_allowed(&self) -> bool {
        let job = REFRESH_JOB.with(|slot| slot.get());
        job == 0 || job == self.job_id.load(Ordering::Acquire)
    }

    fn bump(&self) {
        self.generation.fetch_add(1, Ordering::Release);
    }

    fn definition_path(&self, name: &str) -> PathBuf {
        self.dir
            .join("definitions")
            .join(format!("{}.json", sha256_hex(name)))
    }
}

fn name_of(meta: &TableMeta) -> &str {
    meta.name.as_str()
}

fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("DQLRS_TABLE_CACHE") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("dqlrs").join("tables");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home)
                .join(".cache")
                .join("dqlrs")
                .join("tables");
        }
    }
    PathBuf::from(".cache").join("dqlrs").join("tables")
}

fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn meta_from_schema(schema: &str) -> Option<TableMeta> {
    let statement = parse_statement(schema).ok()?;
    TableMeta::from_create_statement(&statement).ok()
}

fn status_label(status: TableStatus) -> &'static str {
    match status {
        TableStatus::Active => "ACTIVE",
        TableStatus::Creating => "CREATING",
        TableStatus::Updating => "UPDATING",
        TableStatus::Deleting => "DELETING",
    }
}

fn status_from_label(label: &str) -> TableStatus {
    match label {
        "CREATING" => TableStatus::Creating,
        "UPDATING" => TableStatus::Updating,
        "DELETING" => TableStatus::Deleting,
        _ => TableStatus::Active,
    }
}

pub fn endpoint_key(region: &str, host: Option<&str>, port: u16, identity: &str) -> String {
    match host {
        Some(host) => format!("local|{region}|{host}|{port}"),
        None => format!("aws|{region}|{identity}"),
    }
}

pub fn age_phrase(saved_at_ms: u64) -> String {
    dql_engine::age_phrase(saved_at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::parse_statement;

    fn endpoint() -> CacheEndpoint {
        CacheEndpoint {
            region: "us-west-1".to_string(),
            host: Some("localhost".to_string()),
            port: 8000,
            allow_select_scan: false,
        }
    }

    fn posts() -> TableMeta {
        let statement = parse_statement(
            "CREATE TABLE posts (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-sk', sk NUMBER, id STRING);",
        )
        .unwrap();
        let mut meta = TableMeta::from_create_statement(&statement).unwrap();
        meta.status = TableStatus::Updating;
        meta
    }

    #[test]
    fn list_and_definition_round_trip_and_refresh_sees_only_those() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskTableCache::open_at(dir.path().join("cache"), endpoint());
        assert!(!cache.start_refresh());
        assert!(cache.get_names().is_none());
        assert!(cache.definition_names().is_empty());

        let saved = cache.put_names(&["posts".to_string(), "other".to_string()]);
        let names = cache.get_names().unwrap();
        assert_eq!(names.saved_at_ms, saved);
        assert_eq!(names.names, vec!["posts".to_string(), "other".to_string()]);

        cache.put_definition(&posts());
        assert_eq!(cache.definition_names(), vec!["posts".to_string()]);
        let loaded = cache.get_definition("posts").unwrap();
        assert_eq!(loaded.meta.hash_key, "id");
        assert_eq!(loaded.meta.range_key.as_deref(), Some("sk"));
        assert!(loaded.meta.global_indexes.contains_key("by-sk"));
        assert_eq!(loaded.meta.status, TableStatus::Updating);
        assert!(cache.get_definition("other").is_none());

        cache.remove_definition("posts");
        assert!(cache.definition_names().is_empty());
        cache.forget_names();
        assert!(cache.get_names().is_none());
        assert!(!cache.start_refresh());

        cache.put_names(&["posts".to_string()]);
        cache.put_definition(&posts());
        cache.clear();
        assert!(cache.get_names().is_none());
        assert!(cache.get_definition("posts").is_none());
        assert!(cache.definition_names().is_empty());
        assert!(!cache.start_refresh());
    }
}
