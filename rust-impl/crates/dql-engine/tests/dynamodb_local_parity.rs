//! DynamoDB Local integration parity suite.
//!
//! Run when Local is available:
//! `cargo test -p dql-engine --test dynamodb_local_parity -- --ignored`

#[path = "support/mod.rs"]
mod support;

use dql_engine::StatementResult;
use dql_parser::Value;
use support::{skip_if_no_local, LocalHarness};

fn run_script(harness: &mut LocalHarness, table: &str, script: &str) -> StatementResult {
    harness
        .query(&script.replace("{table}", table))
        .expect("script should succeed")
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn parity_create_insert_select() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("parity_select");
    run_script(
        &mut harness,
        &table,
        "CREATE TABLE {table} (id STRING HASH KEY, bar NUMBER);
         INSERT INTO {table} (id, bar) VALUES ('a', 1), ('b', 2);
         SELECT * FROM {table} WHERE id = 'b'",
    );
    match harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed")
    {
        StatementResult::Items(items) => {
            assert_eq!(items.len(), 2);
            let b = items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                .expect("missing item b");
            assert_eq!(b.get("bar"), Some(&Value::Number("2".to_string())));
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn parity_update_and_delete() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("parity_mutate");
    run_script(
        &mut harness,
        &table,
        "CREATE TABLE {table} (id STRING HASH KEY, bar NUMBER);
         INSERT INTO {table} (id, bar) VALUES ('a', 1), ('b', 2);
         UPDATE {table} SET bar = 3 WHERE id = 'a';
         DELETE FROM {table} KEYS IN 'b'",
    );
    match harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed")
    {
        StatementResult::Items(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].get("id"), Some(&Value::String("a".to_string())));
            assert_eq!(items[0].get("bar"), Some(&Value::Number("3".to_string())));
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn parity_index_query() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("parity_index");
    run_script(
        &mut harness,
        &table,
        "CREATE TABLE {table} (id STRING HASH KEY, bar NUMBER RANGE KEY, ts NUMBER INDEX('ts-index'));
         INSERT INTO {table} (id, bar, ts) VALUES ('a', 1, 100), ('a', 2, 200);
         SELECT * FROM {table} WHERE id = 'a' AND ts < 150 USING ts-index",
    );
    match harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed")
    {
        StatementResult::Items(items) => {
            assert_eq!(items.len(), 2);
            assert!(items
                .iter()
                .any(|item| item.get("ts") == Some(&Value::Number("100".to_string()))));
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn parity_scan_filter() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("parity_scan");
    run_script(
        &mut harness,
        &table,
        "CREATE TABLE {table} (id STRING HASH KEY, bar NUMBER);
         INSERT INTO {table} (id, bar) VALUES ('a', 1), ('b', 2);
         SCAN bar FROM {table} WHERE bar > 1",
    );
    match harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed")
    {
        StatementResult::Items(items) => assert_eq!(items.len(), 2),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn parity_explain_query() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("parity_explain");
    harness
        .query(&format!(
            "CREATE TABLE {table} (id STRING HASH KEY);
             INSERT INTO {table} (id) VALUES ('a')"
        ))
        .expect("setup should succeed");
    match harness
        .query(&format!("EXPLAIN SELECT * FROM {table} WHERE id = 'a'"))
        .expect("explain should succeed")
    {
        StatementResult::Schema(schema) => {
            assert!(schema.contains("query"));
            assert!(schema.contains(&table));
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
