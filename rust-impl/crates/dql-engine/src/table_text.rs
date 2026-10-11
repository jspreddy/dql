//! Plain-text table description shared by `DESCRIBE` and `ls`.

use crate::format_throughput;
use dql_models::{ProjectionType, TableMeta, TableStatus};
use dql_parser::AttributeType;
use humansize::{format_size, BINARY};

/// Detail view for one table. `ls` prints this when a pattern matches one table.
pub fn format_table_description(meta: &TableMeta, item_count: usize, size_bytes: u64) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Name: {}", meta.name));
    lines.push(format!("Status: {}", status_label(meta.status)));
    lines.push(format!("Items: {item_count}"));
    lines.push(format!("Size: {}", format_size(size_bytes, BINARY)));

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
    let throughput = throughput?;
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
