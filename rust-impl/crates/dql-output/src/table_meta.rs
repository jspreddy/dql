use dql_engine::format_throughput;
use dql_models::{TableMeta, TableStatus};
use humansize::{format_size, BINARY};
use std::fmt::Write as _;

#[derive(Debug, Clone, Default)]
pub struct TableStats {
    pub item_count: usize,
    pub size_bytes: u64,
}

const MIN_NAME_WIDTH: usize = 24;

pub fn format_table_summary_table(tables: &[(TableMeta, TableStats)]) -> String {
    let name_width = tables
        .iter()
        .map(|(meta, _)| meta.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(MIN_NAME_WIDTH);
    let mut output = String::new();
    let _ = writeln!(output, "Tables");
    let _ = writeln!(
        output,
        "{:<name_width$} {:>8} {:>8} {:>8} {:>10} {:>8}",
        "Name",
        "Items",
        "Read",
        "Write",
        "Status",
        "Size",
        name_width = name_width
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
            "{:<name_width$} {:>8} {:>8} {:>8} {:>10} {:>8}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use dql_models::TableMeta;
    use dql_parser::parse_statement;

    #[test]
    fn summary_table_shows_full_long_names() {
        let name = "parity_explain_1783478299253891557";
        let statement = parse_statement(&format!(
            "CREATE TABLE {name} (id STRING HASH KEY)"
        ))
        .unwrap();
        let meta = TableMeta::from_create_statement(&statement).unwrap();
        let rows = [(
            meta,
            TableStats {
                item_count: 0,
                size_bytes: 0,
            },
        )];
        let output = format_table_summary_table(&rows);
        assert!(output.contains(name));
        assert!(!output.contains("..."));
    }
}
