use super::{run_framed, Control};
use crate::session::Session;
use std::io::{self, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindError {
    pub message: String,
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BindError {}

/// Parse `--bind` into a loopback socket address.
///
/// Accepts `HOST:PORT` (`127.0.0.1:7400`, `[::1]:7400`, `localhost:7400`)
/// or a bare port (`7400` → `127.0.0.1:7400`). Non-loopback hosts are refused
/// before listen.
pub fn parse_bind(spec: &str) -> Result<SocketAddr, BindError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(BindError {
            message: "bind address is empty".to_string(),
        });
    }

    if let Ok(port) = spec.parse::<u16>() {
        return Ok(SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)));
    }

    let addrs: Vec<SocketAddr> = spec
        .to_socket_addrs()
        .map_err(|err| BindError {
            message: format!("invalid bind address '{spec}': {err}"),
        })?
        .collect();
    if addrs.is_empty() {
        return Err(BindError {
            message: format!("could not resolve bind address '{spec}'"),
        });
    }
    for addr in &addrs {
        if !addr.ip().is_loopback() {
            return Err(BindError {
                message: format!("refusing non-loopback bind address '{spec}' (resolved {addr})"),
            });
        }
    }
    Ok(addrs[0])
}

pub fn run(session: &mut Session, addr: SocketAddr) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    {
        let mut stderr = io::stderr();
        writeln!(stderr, "dqlrs serve listen {local}")?;
        stderr.flush()?;
    }

    let busy = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<TcpStream>();
    let busy_accept = Arc::clone(&busy);
    let shutdown_accept = Arc::clone(&shutdown);

    let accept = thread::Builder::new()
        .name("dqlrs-serve-accept".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                if shutdown_accept.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = incoming else {
                    continue;
                };
                if shutdown_accept.load(Ordering::SeqCst) {
                    break;
                }
                if busy_accept.swap(true, Ordering::SeqCst) {
                    eprintln!("dqlrs serve refuse: already connected");
                    let _ = io::stderr().flush();
                    drop(stream);
                    continue;
                }
                if tx.send(stream).is_err() {
                    break;
                }
            }
        })?;

    loop {
        let Ok(stream) = rx.recv() else {
            break;
        };
        let control = serve_client(session, stream).unwrap_or(Control::Disconnect);
        busy.store(false, Ordering::SeqCst);
        if matches!(control, Control::Shutdown) {
            break;
        }
    }

    shutdown.store(true, Ordering::SeqCst);
    let _ = TcpStream::connect(local);
    let _ = accept.join();
    Ok(())
}

fn serve_client(session: &mut Session, stream: TcpStream) -> io::Result<Control> {
    let _ = stream.set_nodelay(true);
    let reader = BufReader::new(stream.try_clone()?);
    run_framed(reader, stream, session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_only_is_loopback() {
        assert_eq!(
            parse_bind("7400").unwrap(),
            SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 7400))
        );
    }

    #[test]
    fn accepts_loopback_hosts() {
        assert_eq!(
            parse_bind("127.0.0.1:7400").unwrap(),
            "127.0.0.1:7400".parse().unwrap()
        );
        let localhost = parse_bind("localhost:7400").unwrap();
        assert!(localhost.ip().is_loopback());
        assert_eq!(localhost.port(), 7400);
        let v6 = parse_bind("[::1]:7400").unwrap();
        assert!(v6.ip().is_loopback());
        assert_eq!(v6.port(), 7400);
    }

    #[test]
    fn refuses_non_loopback() {
        let err = parse_bind("0.0.0.0:1").unwrap_err();
        assert!(
            err.message.contains("non-loopback"),
            "unexpected error: {}",
            err.message
        );
        let err = parse_bind("[::]:80").unwrap_err();
        assert!(err.message.contains("non-loopback"), "{}", err.message);
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_bind("").is_err());
        assert!(parse_bind("   ").is_err());
    }

    fn wait_connect(addr: SocketAddr) -> TcpStream {
        use std::time::Duration;
        for _ in 0..50 {
            if let Ok(stream) = TcpStream::connect(addr) {
                let _ = stream.set_nodelay(true);
                return stream;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("could not connect to {addr}");
    }

    fn read_json_line(stream: &mut TcpStream) -> serde_json::Value {
        try_read_json(stream).unwrap()
    }

    fn read_envelope(stream: &mut TcpStream) -> serde_json::Value {
        loop {
            let value = read_json_line(stream);
            if value.get("ok").is_some() {
                return value;
            }
        }
    }

    fn try_read_json(stream: &mut TcpStream) -> io::Result<serde_json::Value> {
        use std::io::BufRead;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
        }
        serde_json::from_str(line.trim())
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn reconnect(addr: SocketAddr) -> TcpStream {
        use std::io::Write;
        use std::time::Duration;
        for _ in 0..50 {
            let mut stream = wait_connect(addr);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            if writeln!(stream, r#"{{"op":"ping"}}"#).is_err() {
                continue;
            }
            if stream.flush().is_err() {
                continue;
            }
            if let Ok(reply) = try_read_json(&mut stream) {
                if reply["ok"] == true {
                    return stream;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("could not reconnect to {addr}");
    }

    #[test]
    fn tcp_loop_ping_refuse_second_and_shutdown() {
        use std::io::{Read, Write};
        use std::time::Duration;

        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let handle = thread::spawn(move || {
            let mut session = crate::session::Session::new_memory_headless("us-west-1");
            run(&mut session, addr).unwrap();
        });

        let mut client = wait_connect(addr);
        writeln!(client, r#"{{"id":"1","op":"ping"}}"#).unwrap();
        client.flush().unwrap();
        let ping = read_json_line(&mut client);
        assert_eq!(ping["ok"], true);
        assert_eq!(ping["id"], "1");

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
                    io::ErrorKind::ConnectionReset
                        | io::ErrorKind::BrokenPipe
                        | io::ErrorKind::UnexpectedEof
                ) => {}
            other => panic!("expected second client to be refused, got {other:?}"),
        }

        writeln!(client, r#"{{"id":"s","op":"shutdown"}}"#).unwrap();
        client.flush().unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn tcp_disconnect_keeps_session() {
        use std::io::Write;

        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let handle = thread::spawn(move || {
            let mut session = crate::session::Session::new_memory_headless("us-west-1");
            run(&mut session, addr).unwrap();
        });

        let mut client = wait_connect(addr);
        writeln!(
            client,
            r#"{{"op":"exec","dql":"CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a');"}}"#
        )
        .unwrap();
        client.flush().unwrap();
        let created = read_envelope(&mut client);
        assert_eq!(created["ok"], true);
        drop(client);

        let mut client = reconnect(addr);
        writeln!(
            client,
            r#"{{"op":"exec","dql":"SELECT * FROM t WHERE id = 'a';"}}"#
        )
        .unwrap();
        client.flush().unwrap();
        let selected = read_json_line(&mut client);
        assert_eq!(selected["ok"], true);
        assert_eq!(selected["kind"], "items");
        assert_eq!(selected["items"][0]["id"], "a");
        writeln!(client, r#"{{"op":"shutdown"}}"#).unwrap();
        client.flush().unwrap();
        handle.join().unwrap();
    }
}
