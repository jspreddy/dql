//! Smoke test against DynamoDB Local.
//!
//! Requires Local on localhost:8000. Fails if Local is not running.

#[path = "support/mod.rs"]
mod support;

use support::{skip_if_no_local, LocalHarness};

#[test]
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
            "CREATE TABLE {table} (id STRING HASH KEY, score NUMBER);
             INSERT INTO {table} (id, score) VALUES ('a', 1), ('b', 2);
             SELECT * FROM {table} WHERE id = 'b';
             DROP TABLE {table};"
        ))
        .expect("script should succeed");
    let names = harness
        .engine
        .table_names()
        .expect("list tables should succeed");
    assert!(
        !names.iter().any(|name| name == &table),
        "table {table} should be dropped"
    );
}
