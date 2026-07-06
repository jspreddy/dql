use dql_engine::format_throughput;
use dql_models::{TableMeta, TableStatus};
use humansize::{format_size, BINARY};
use std::fmt::Write as _;

#[derive(Debug, Clone, Default)]
pub struct TableStats {
    pub item_count: usize,
    pub size_bytes: u64,
}

pub fn format_table_summary_table(tables: &[(TableMeta, TableStats)]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Tables");
    let _ = writeln!(
        output,
        "{:<24} {:>8} {:>8} {:>8} {:>10} {:>8}",
        "Name", "Items", "Read", "Write", "Status", "Size"
    );
    for (meta, stats) in tables {
        let read = meta
            .total_read_throughput()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let write = meta
            .total_write_throughput()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let status = match meta.status {
            TableStatus::Active => "ACTIVE",
            TableStatus::Creating => "CREATING",
            TableStatus::Updating => "UPDATING",
            TableStatus::Deleting => "DELETING",
        };
        let _ = writeln!(
            output,
            "{:<24} {:>8} {:>8} {:>8} {:>10} {:>8}",
            meta.name,
            stats.item_count,
            read,
            write,
            status,
            format_size(stats.size_bytes, BINARY)
        );
    }
    output
}

pub fn format_table_detail(meta: &TableMeta, stats: &TableStats) -> String {
    let mut lines = vec![format!("Table: {}", meta.name)];
    lines.push(format!("Status: {:?}", meta.status));
    lines.push(format!("Items: {}", stats.item_count));
    lines.push(format!("Size: {}", format_size(stats.size_bytes, BINARY)));
    if let Some(read) = meta.total_read_throughput() {
        lines.push(format!("Read: {}", format_throughput(Some(read), None)));
    }
    if let Some(write) = meta.total_write_throughput() {
        lines.push(format!("Write: {}", format_throughput(Some(write), None)));
    }
    lines.push(String::new());
    lines.push(meta.schema_dql());
    lines.join("\n")
}
