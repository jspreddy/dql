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

#[test]
fn drop_if_exists_missing_then_create_against_local() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("dql_drop_if");
    harness
        .query(&format!(
            "DROP TABLE IF EXISTS {table};
             CREATE TABLE {table} (id STRING HASH KEY);
             DROP TABLE IF EXISTS {table};"
        ))
        .expect("DROP IF EXISTS of a missing table should not fail");
    let names = harness
        .engine
        .table_names()
        .expect("list tables should succeed");
    assert!(
        !names.iter().any(|name| name == &table),
        "table {table} should be dropped"
    );
}

#[test]
fn scan_single_page_skips_read_progress() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("dql_scan_one_page");
    harness
        .query(&format!(
            "CREATE TABLE {table} (id STRING HASH KEY);
             INSERT INTO {table} (id) VALUES ('a'), ('b');"
        ))
        .expect("setup should succeed");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_cb = std::sync::Arc::clone(&seen);
    harness.engine.progress().set(Some(Box::new(move |event| {
        seen_cb.lock().unwrap().push(event);
    })));
    harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed");
    harness.engine.progress().clear();
    let events = seen.lock().unwrap().clone();
    assert!(
        events.iter().all(|event| event.phase != "read"),
        "single-page SCAN should not emit read progress: {events:?}"
    );
}

#[test]
fn scan_multi_page_completes_read_progress() {
    if skip_if_no_local() {
        return;
    }
    let Some(mut harness) = LocalHarness::try_new() else {
        return;
    };
    let table = LocalHarness::unique_table_name("dql_scan_pages");
    harness
        .query(&format!("CREATE TABLE {table} (id STRING HASH KEY);"))
        .expect("create should succeed");
    let mut values = Vec::new();
    for i in 0..120 {
        values.push(format!("('{i}')"));
    }
    harness
        .query(&format!(
            "INSERT INTO {table} (id) VALUES {}",
            values.join(", ")
        ))
        .expect("insert should succeed");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_cb = std::sync::Arc::clone(&seen);
    harness.engine.progress().set(Some(Box::new(move |event| {
        seen_cb.lock().unwrap().push(event);
    })));
    harness
        .query(&format!("SCAN * FROM {table}"))
        .expect("scan should succeed");
    harness.engine.progress().clear();
    let reads: Vec<_> = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|event| event.phase == "read")
        .cloned()
        .collect();
    assert!(
        reads.len() >= 2,
        "paged SCAN should emit more than one read event: {reads:?}"
    );
    let last = reads.last().unwrap();
    assert_eq!(
        last.total,
        Some(last.done),
        "last read event should complete: {reads:?}"
    );
}
