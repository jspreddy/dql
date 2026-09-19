//! Binary-level `--bind` tests for `dqlrs --serve`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

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

struct ServeChild {
    child: Child,
}

impl ServeChild {
    fn spawn_bind() -> (Self, SocketAddr) {
        let mut child = Command::new(dql_bin())
            .args(["--serve", "--bind", "127.0.0.1:0"])
            .env("DQL_BACKEND", "memory")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn dqlrs --serve --bind");
        let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));
        let mut line = String::new();
        stderr
            .read_line(&mut line)
            .expect("read listen line from stderr");
        let addr = parse_listen_line(&line);
        (Self { child }, addr)
    }

    fn shutdown_ok(mut self) {
        let status = self.child.wait().expect("wait serve");
        assert!(status.success(), "serve exit {status:?}");
    }
}

impl Drop for ServeChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_listen_line(line: &str) -> SocketAddr {
    let prefix = "dqlrs serve listen ";
    let rest = line
        .trim()
        .strip_prefix(prefix)
        .unwrap_or_else(|| panic!("expected '{prefix}…', got {line:?}"));
    rest.parse()
        .unwrap_or_else(|err| panic!("invalid listen addr {rest:?}: {err}"))
}

fn wait_connect(addr: SocketAddr) -> TcpStream {
    for _ in 0..50 {
        if let Ok(stream) = TcpStream::connect(addr) {
            let _ = stream.set_nodelay(true);
            return stream;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("could not connect to {addr}");
}

fn write_line(stream: &mut TcpStream, line: &str) {
    writeln!(stream, "{line}").unwrap();
    stream.flush().unwrap();
}

fn read_json(stream: &mut TcpStream) -> serde_json::Value {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[test]
fn bind_ephemeral_ping_and_exec() {
    let (server, addr) = ServeChild::spawn_bind();
    let mut client = wait_connect(addr);
    write_line(&mut client, r#"{"id":"1","op":"ping"}"#);
    let ping = read_json(&mut client);
    assert_eq!(ping["ok"], true);
    write_line(
        &mut client,
        r#"{"id":"2","op":"exec","dql":"CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t;"}"#,
    );
    let exec = read_json(&mut client);
    assert_eq!(exec["ok"], true, "{exec}");
    assert_eq!(exec["items"][0]["id"], "a");
    write_line(&mut client, r#"{"op":"shutdown"}"#);
    drop(client);
    server.shutdown_ok();
}

#[test]
fn bind_refuses_second_client() {
    let (server, addr) = ServeChild::spawn_bind();
    let mut client = wait_connect(addr);
    write_line(&mut client, r#"{"id":"1","op":"ping"}"#);
    assert_eq!(read_json(&mut client)["ok"], true);

    let mut second = wait_connect(addr);
    second
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buf = [0u8; 8];
    match second.read(&mut buf) {
        Ok(0) => {}
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof
            ) => {}
        other => panic!("expected second client refuse, got {other:?}"),
    }

    write_line(&mut client, r#"{"op":"shutdown"}"#);
    drop(client);
    server.shutdown_ok();
}
