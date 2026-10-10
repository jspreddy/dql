//! Binary-level stdio tests for `dqlrs --serve`.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn dql_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_dqlrs") {
        return PathBuf::from(path);
    }
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join(profile).join("dqlrs")
}

fn read_envelope(stdout: &mut BufReader<impl std::io::Read>) -> serde_json::Value {
    loop {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        if value.get("ok").is_some() {
            return value;
        }
    }
}

#[test]
fn stdio_ping_exec_shutdown() {
    let mut child = Command::new(dql_bin())
        .args(["--serve"])
        .env("DQL_BACKEND", "memory")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dqlrs --serve");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    writeln!(stdin, r#"{{"id":"1","op":"ping"}}"#).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let ping: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(ping["ok"], true);
    assert_eq!(ping["id"], "1");

    writeln!(
        stdin,
        r#"{{"id":"2","op":"exec","dql":"CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t;"}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let exec = read_envelope(&mut stdout);
    assert_eq!(exec["ok"], true, "{exec}");
    assert_eq!(exec["kind"], "items");
    assert_eq!(exec["items"][0]["id"], "a");

    writeln!(stdin, r#"{{"id":"3","op":"shutdown"}}"#).unwrap();
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}
