//! Package smoke tests for the release `dql` binary.
//!
//! Verifies `--version`, one-shot memory commands, JSON output, and optional
//! DynamoDB Local integration when reachable.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TABLE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn dql_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_dql") {
        return PathBuf::from(path);
    }
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join(profile).join("dql")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(dql_bin())
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run dql {args:?}: {err}"))
}

fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed (exit {:?})\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unique_table(prefix: &str) -> String {
    let n = TABLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_smoke_{n}")
}

fn local_available() -> bool {
    let host = std::env::var("DQL_LOCAL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = std::env::var("DQL_LOCAL_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8000);
    std::net::TcpStream::connect((host.as_str(), port)).is_ok()
}

fn require_local() -> bool {
    std::env::var("DQL_REQUIRE_LOCAL")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[test]
fn smoke_version() {
    let output = run(&["--version"]);
    assert_success(&output, "dql --version");
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        !version.is_empty(),
        "version output should not be empty: {version:?}"
    );
}

#[test]
fn smoke_memory_one_shot() {
    let table = unique_table("pkg");
    let script = format!(
        "CREATE TABLE {table} (id STRING HASH KEY); \
         INSERT INTO {table} (id) VALUES ('a'); \
         SCAN * FROM {table}; \
         DROP TABLE {table};"
    );
    let output = run(&["-c", &script]);
    assert_success(&output, "memory one-shot script");
}

#[test]
fn smoke_memory_json() {
    let table = unique_table("pkg_json");
    let script = format!(
        "CREATE TABLE {table} (id STRING HASH KEY); \
         INSERT INTO {table} (id) VALUES ('a'); \
         SCAN * FROM {table}"
    );
    let output = run(&["--json", "-c", &script]);
    assert_success(&output, "memory json one-shot");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"id\""),
        "json output should include item fields: {stdout}"
    );
}

#[test]
fn smoke_dynamodb_local() {
    if !local_available() {
        if require_local() {
            panic!("DQL_REQUIRE_LOCAL is set but DynamoDB Local is not reachable");
        }
        eprintln!("skipping: DynamoDB Local not available");
        return;
    }

    let host = std::env::var("DQL_LOCAL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = std::env::var("DQL_LOCAL_PORT").unwrap_or_else(|_| "8000".to_string());
    let table = unique_table("pkg_local");
    let script = format!(
        "CREATE TABLE {table} (id STRING HASH KEY, score NUMBER); \
         INSERT INTO {table} (id, score) VALUES ('a', 1); \
         SELECT * FROM {table} WHERE id = 'a'; \
         DROP TABLE {table};"
    );
    let output = run(&["-H", &host, "-p", &port, "-c", &script]);
    assert_success(&output, "DynamoDB Local one-shot script");
}
