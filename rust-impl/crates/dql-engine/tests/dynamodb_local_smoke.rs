#[path = "support/mod.rs"]
mod support;

use dql_engine::StatementResult;
use support::{skip_if_no_local, LocalHarness};

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn create_insert_select_drop_against_local() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("dql_smoke");
    harness
        .query(&format!(
            "CREATE TABLE {table} (id STRING HASH KEY);
             INSERT INTO {table} (id, score) VALUES ('a', 1), ('b', 2);
             SELECT * FROM {table} WHERE id = 'b';
             DROP TABLE {table};"
        ))
        .expect("script should succeed");
    match harness.query(&format!("SCAN * FROM {table}")).unwrap() {
        StatementResult::Items(items) => assert!(items.is_empty()),
        other => panic!("unexpected result: {other:?}"),
    }
}
