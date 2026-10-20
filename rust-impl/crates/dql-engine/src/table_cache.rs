//! Optional disk cache for the table name list and definitions already fetched.
//!
//! The engine does not choose a directory. The CLI installs a store so the same
//! snapshots are shared by the REPL, one-shot commands, the web UI, and the
//! notebook serve session.

use dql_models::TableMeta;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedNameList {
    pub saved_at_ms: u64,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedDefinition {
    pub saved_at_ms: u64,
    pub meta: TableMeta,
}

pub trait TableCacheStore: Send + Sync {
    fn generation(&self) -> u64;
    fn refreshing(&self) -> bool;
    fn get_names(&self) -> Option<CachedNameList>;
    fn put_names(&self, names: &[String]) -> u64;
    fn forget_names(&self);
    fn get_definition(&self, name: &str) -> Option<CachedDefinition>;
    fn put_definition(&self, meta: &TableMeta) -> u64;
    fn remove_definition(&self, name: &str);
    fn definition_names(&self) -> Vec<String>;
    fn clear(&self);
}

pub type SharedTableCache = Arc<dyn TableCacheStore>;
