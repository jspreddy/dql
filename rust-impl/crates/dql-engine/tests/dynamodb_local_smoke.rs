use dql_engine::{Engine, SdkBackend, SdkConfig, StatementResult};

fn local_config() -> SdkConfig {
    SdkConfig::local(
        std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".to_string()),
        std::env::var("DQL_LOCAL_HOST").unwrap_or_else(|_| "localhost".to_string()),
        std::env::var("DQL_LOCAL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8000),
    )
}

fn local_available() -> bool {
    SdkBackend::connect(local_config()).is_ok()
}

#[test]
#[ignore = "requires DynamoDB Local on localhost:8000"]
fn create_insert_scan_against_local() {
    if !local_available() {
        return;
    }
    let backend = SdkBackend::connect(local_config()).expect("connect to DynamoDB Local");
    let mut engine = Engine::new(backend);
    let table = format!(
        "dql_phase4_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    engine
        .execute(&format!(
            "DROP TABLE {table} IF EXISTS;
             CREATE TABLE {table} (id STRING HASH KEY);
             INSERT INTO {table} (id, score) VALUES ('a', 1), ('b', 2);
             SCAN * FROM {table};
             DROP TABLE {table};"
        ))
        .expect("script should succeed");
    match engine.execute(&format!("SCAN * FROM {table}")).unwrap() {
        StatementResult::Items(items) => assert!(items.is_empty()),
        other => panic!("unexpected result: {other:?}"),
    }
}
