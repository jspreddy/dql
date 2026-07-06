use dql_engine::RateLimit;
use dql_models::TableMeta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableLimits {
    #[serde(default)]
    pub total: BTreeMap<String, String>,
    #[serde(default)]
    pub default: BTreeMap<String, String>,
    #[serde(default)]
    pub tables: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub indexes: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
}

impl TableLimits {
    pub fn is_active(&self) -> bool {
        !self.total.is_empty()
            || !self.default.is_empty()
            || !self.tables.is_empty()
            || !self.indexes.is_empty()
    }

    pub fn load(&mut self, data: &serde_json::Value) {
        if let Ok(parsed) = serde_json::from_value::<Self>(data.clone()) {
            *self = parsed;
        }
    }

    pub fn save(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn set_default_limit(&mut self, read: &str, write: &str) {
        if read == "0" && write == "0" {
            self.default.clear();
            return;
        }
        self.default.insert("read".to_string(), read.to_string());
        self.default.insert("write".to_string(), write.to_string());
    }

    pub fn set_total_limit(&mut self, read: &str, write: &str) -> Result<(), String> {
        if read == "0" && write == "0" {
            self.total.clear();
            return Ok(());
        }
        if !read.chars().all(|c| c.is_ascii_digit())
            || !write.chars().all(|c| c.is_ascii_digit())
        {
            return Err("Total read/write limits must be a point value".to_string());
        }
        self.total.insert("read".to_string(), read.to_string());
        self.total.insert("write".to_string(), write.to_string());
        Ok(())
    }

    pub fn set_table_limit(&mut self, table: &str, read: &str, write: &str) {
        set_limit(&mut self.tables, table, read, write);
    }

    pub fn set_index_limit(&mut self, table: &str, index: &str, read: &str, write: &str) {
        let entry = self.indexes.entry(table.to_string()).or_default();
        set_limit(entry, index, read, write);
        if entry.is_empty() {
            self.indexes.remove(table);
        }
    }

    pub fn get_limiter(&self, tables: &[TableMeta]) -> Option<RateLimit> {
        if !self.is_active() {
            return None;
        }
        if let (Some(read), Some(write)) = (self.total.get("read"), self.total.get("write")) {
            return Some(RateLimit::new(
                read.parse().unwrap_or(0.0),
                write.parse().unwrap_or(0.0),
            ));
        }
        let mut total_read = 0.0;
        let mut total_write = 0.0;
        for table in tables {
            let limit = self.tables.get(&table.name).or(Some(&self.default));
            if let Some(limit) = limit {
                if let (Some(read), Some(write)) = (limit.get("read"), limit.get("write")) {
                    if let Some(throughput) = table.total_read_throughput() {
                        total_read += compute_limit(read, throughput);
                    }
                    if let Some(throughput) = table.total_write_throughput() {
                        total_write += compute_limit(write, throughput);
                    }
                }
            }
        }
        if total_read > 0.0 || total_write > 0.0 {
            Some(RateLimit::new(total_read, total_write))
        } else {
            None
        }
    }
}

fn set_limit(
    data: &mut BTreeMap<String, BTreeMap<String, String>>,
    key: &str,
    read: &str,
    write: &str,
) {
    if read != "0" || write != "0" {
        let mut limits = BTreeMap::new();
        limits.insert("read".to_string(), read.to_string());
        limits.insert("write".to_string(), write.to_string());
        data.insert(key.to_string(), limits);
    } else {
        data.remove(key);
    }
}

fn compute_limit(limit: &str, throughput: f64) -> f64 {
    if let Some(pct) = limit.strip_suffix('%') {
        throughput * pct.parse::<f64>().unwrap_or(0.0) / 100.0
    } else {
        limit.parse().unwrap_or(0.0)
    }
}

impl std::fmt::Display for TableLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.is_active() {
            return write!(f, "No throttle");
        }
        let mut lines = Vec::new();
        if !self.total.is_empty() {
            lines.push(format!(
                "Total: {}, {}",
                self.total.get("read").map(String::as_str).unwrap_or("0"),
                self.total.get("write").map(String::as_str).unwrap_or("0")
            ));
        }
        if !self.default.is_empty() {
            lines.push(format!(
                "Default: {}, {}",
                self.default.get("read").map(String::as_str).unwrap_or("0"),
                self.default.get("write").map(String::as_str).unwrap_or("0")
            ));
        }
        for (table, limit) in &self.tables {
            lines.push(format!(
                "{}: {}, {}",
                table,
                limit.get("read").map(String::as_str).unwrap_or("0"),
                limit.get("write").map(String::as_str).unwrap_or("0")
            ));
        }
        write!(f, "{}", lines.join("\n"))
    }
}
