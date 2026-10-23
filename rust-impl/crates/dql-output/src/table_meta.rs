use dql_models::TableMeta;

#[derive(Debug, Clone, Default)]
pub struct TableStats {
    pub item_count: usize,
    pub size_bytes: u64,
}

pub fn format_table_summary_table(tables: &[(TableMeta, TableStats)]) -> String {
    let rows: Vec<(&TableMeta, usize, u64)> = tables
        .iter()
        .map(|(meta, stats)| (meta, stats.item_count, stats.size_bytes))
        .collect();
    dql_engine::format_table_summary(&rows)
}

/// Rich detail view for a single table — information parity with Python `pformat(rich)`.
pub fn format_table_detail(meta: &TableMeta, stats: &TableStats) -> String {
    dql_engine::format_table_description(meta, stats.item_count, stats.size_bytes)
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
        assert!(output.contains(
            "CREATE TABLE foobar (\n  id STRING HASH KEY,\n  baz STRING,\n  ts NUMBER INDEX('ts-index'),"
        ));
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
