use dql_engine::format_throughput;
use dql_models::{ProjectionType, TableMeta, TableStatus};
use dql_parser::AttributeType;
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
    let mut lines = Vec::new();
    lines.push(format!("Name: {}", meta.name));
    lines.push(format!("Status: {}", status_label(meta.status)));
    lines.push(format!("Items: {}", stats.item_count));
    lines.push(format!("Size: {}", format_size(stats.size_bytes, BINARY)));

    let cap = meta.consumed_capacity.get("__table__");
    lines.push(format!(
        "Read: {}",
        format_throughput(meta.table_read_throughput(), cap.map(|c| c.read))
    ));
    lines.push(format!(
        "Write: {}",
        format_throughput(meta.table_write_throughput(), cap.map(|c| c.write))
    ));

    if let Some(hash) = meta.attrs.get(&meta.hash_key) {
        lines.push(format!(
            "Hash Key: {} ({})",
            hash.name,
            type_label(&hash.data_type)
        ));
    }
    if let Some(range_key) = &meta.range_key {
        if let Some(range) = meta.attrs.get(range_key) {
            lines.push(format!(
                "Range Key: {} ({})",
                range.name,
                type_label(&range.data_type)
            ));
        }
    }

    if !meta.local_indexes.is_empty() {
        lines.push(String::new());
        lines.push("Local Indexes:".to_string());
        for index in meta.local_indexes.values() {
            let range = index.range_key.as_deref().unwrap_or("-");
            lines.push(format!(
                "  {}  hash={}  range={}  projection={}",
                index.name,
                index.hash_key,
                range,
                projection_label(&index.projection)
            ));
        }
    }

    if !meta.global_indexes.is_empty() {
        lines.push(String::new());
        lines.push("Global Indexes:".to_string());
        lines.push(format!(
            "  {:<16} {:<12} {:>8} {:>8} {:<20} {:<20} {:>8}",
            "Name", "Projection", "Read", "Write", "HashKey", "RangeKey", "Status"
        ));
        for (index_name, gindex) in &meta.global_indexes {
            let idx_cap = meta.consumed_capacity.get(index_name);
            let read = format_throughput(
                throughput_number(gindex.throughput.as_ref(), true),
                idx_cap.map(|c| c.read),
            );
            let write = format_throughput(
                throughput_number(gindex.throughput.as_ref(), false),
                idx_cap.map(|c| c.write),
            );
            let hash_key = format!(
                "{} ({})",
                gindex.hash_key.name,
                type_label(&gindex.hash_key.data_type)
            );
            let range_key = gindex
                .range_key
                .as_ref()
                .map(|field| format!("{} ({})", field.name, type_label(&field.data_type)))
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "  {:<16} {:<12} {:>8} {:>8} {:<20} {:<20} {:>8}",
                gindex.name,
                projection_label(&gindex.projection),
                read,
                write,
                hash_key,
                range_key,
                status_label(gindex.status)
            ));
        }
    }

    lines.push(String::new());
    lines.push(meta.schema_dql());
    lines.join("\n")
}

fn status_label(status: TableStatus) -> &'static str {
    match status {
        TableStatus::Active => "ACTIVE",
        TableStatus::Creating => "CREATING",
        TableStatus::Updating => "UPDATING",
        TableStatus::Deleting => "DELETING",
    }
}

fn type_label(data_type: &AttributeType) -> &str {
    match data_type {
        AttributeType::String => "STRING",
        AttributeType::Number => "NUMBER",
        AttributeType::Binary => "BINARY",
        AttributeType::Bool => "BOOL",
        AttributeType::Other(value) => value.as_str(),
    }
}

fn projection_label(projection: &ProjectionType) -> String {
    match projection {
        ProjectionType::All => "ALL".to_string(),
        ProjectionType::KeysOnly => "KEYS".to_string(),
        ProjectionType::Include(fields) => format!("INCLUDE({})", fields.join(",")),
    }
}

fn throughput_number(throughput: Option<&dql_parser::Throughput>, read: bool) -> Option<f64> {
    let Some(throughput) = throughput else {
        return None;
    };
    let value = if read {
        &throughput.read
    } else {
        &throughput.write
    };
    match value {
        dql_parser::Value::Number(number) => number.parse().ok(),
        _ => None,
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
