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
        let status = status_label(meta.status);
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

/// Rich detail view for a single table — information parity with Python `pformat(rich)`.
pub fn format_table_detail(meta: &TableMeta, stats: &TableStats) -> String {
    dql_engine::format_table_description(meta, stats.item_count, stats.size_bytes)
}

fn status_label(status: TableStatus) -> &'static str {
    match status {
        TableStatus::Active => "ACTIVE",
        TableStatus::Creating => "CREATING",
        TableStatus::Updating => "UPDATING",
        TableStatus::Deleting => "DELETING",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_models::{ConsumedCapacity, TableMeta};
    use dql_parser::parse_statement;

    #[test]
    fn summary_table_shows_full_long_names() {
        let name = "parity_explain_1783478299253891557";
        let statement =
            parse_statement(&format!("CREATE TABLE {name} (id STRING HASH KEY)")).unwrap();
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

    #[test]
    fn detail_includes_keys_indexes_and_consumed_capacity() {
        let statement = parse_statement(
            "CREATE TABLE foobar (id STRING HASH KEY, ts NUMBER INDEX('ts-index'), \
             baz STRING, THROUGHPUT (10, 5)) \
             GLOBAL INDEX ('baz-index', baz, THROUGHPUT (2, 1))",
        )
        .unwrap();
        let mut meta = TableMeta::from_create_statement(&statement).unwrap();
        meta.consumed_capacity.insert(
            "__table__".to_string(),
            ConsumedCapacity {
                read: 3.0,
                write: 1.0,
            },
        );
        meta.consumed_capacity.insert(
            "baz-index".to_string(),
            ConsumedCapacity {
                read: 0.5,
                write: 0.0,
            },
        );
        let output = format_table_detail(
            &meta,
            &TableStats {
                item_count: 42,
                size_bytes: 1024,
            },
        );
        assert!(output.contains("Name: foobar"));
        assert!(output.contains("Status: ACTIVE"));
        assert!(output.contains("Items: 42"));
        assert!(output.contains("Hash Key: id (STRING)"));
        assert!(output.contains("Read: 3/10 (30%)"));
        assert!(output.contains("Write: 1/5 (20%)"));
        assert!(output.contains("Local Indexes:"));
        assert!(output.contains("ts-index"));
        assert!(output.contains("Global Indexes:"));
        assert!(output.contains("baz-index"));
        assert!(
            output.contains("0/2 (25%)"),
            "unexpected GSI read format:\n{output}"
        );
        assert!(output.contains("CREATE TABLE foobar"));
        // Keep a stable golden fragment for layout regressions.
        let expected_prefix = "\
Name: foobar
Status: ACTIVE
Items: 42";
        assert!(
            output.starts_with(expected_prefix),
            "unexpected detail prefix:\n{output}"
        );
    }
}
