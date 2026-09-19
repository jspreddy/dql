pub mod protocol;
mod stdio;
pub mod tcp;

use crate::session::Session;
use protocol::{parse_request_line, ServeEnvelope, ServeProgress};
use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub use tcp::parse_bind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Shutdown,
    Disconnect,
}

pub fn run(session: &mut Session, bind: Option<SocketAddr>) -> io::Result<()> {
    match bind {
        Some(addr) => tcp::run(session, addr),
        None => stdio::run(session),
    }
}

/// Shared JSON-lines loop for stdio and TCP. One request at a time.
pub fn run_framed<R, W>(mut reader: R, writer: W, session: &mut Session) -> io::Result<Control>
where
    R: BufRead,
    W: Write + Send + 'static,
{
    let writer = Arc::new(Mutex::new(writer));
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(Control::Disconnect),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                write_envelope(
                    &mut *lock_writer(&writer)?,
                    &ServeEnvelope::err("protocol", "invalid UTF-8"),
                )?;
                continue;
            }
            Err(err) => return Err(err),
        }

        if line.trim().is_empty() {
            continue;
        }

        match handle_line(session, line.trim_end(), &writer)? {
            LineResult::Reply => {}
            LineResult::Shutdown => return Ok(Control::Shutdown),
        }
    }
}

enum LineResult {
    Reply,
    Shutdown,
}

fn handle_line<W: Write + Send + 'static>(
    session: &mut Session,
    line: &str,
    writer: &Arc<Mutex<W>>,
) -> io::Result<LineResult> {
    let request = match parse_request_line(line) {
        Ok(request) => request,
        Err(message) => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::err("protocol", message),
            )?;
            return Ok(LineResult::Reply);
        }
    };
    let id = request.id.clone();
    let op = request
        .op
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match op.as_str() {
        "ping" => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::success("none").with_id(id),
            )?;
            Ok(LineResult::Reply)
        }
        "shutdown" => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::success("none").with_id(id),
            )?;
            Ok(LineResult::Shutdown)
        }
        "interrupt" => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::success("none").with_id(id),
            )?;
            Ok(LineResult::Reply)
        }
        "exec" => {
            let dql = request.dql.clone().unwrap_or_default();
            let progress_writer = Arc::clone(writer);
            let progress_id = id.clone();
            session.progress().set(Some(Box::new(move |event| {
                if let Ok(mut locked) = progress_writer.lock() {
                    let _ = write_progress(&mut *locked, &progress_id, &event);
                }
            })));
            let envelope = session.execute_for_serve(&dql).with_id(id);
            session.progress().clear();
            write_envelope(&mut *lock_writer(writer)?, &envelope)?;
            Ok(LineResult::Reply)
        }
        "" => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::err("protocol", "missing op").with_id(id),
            )?;
            Ok(LineResult::Reply)
        }
        other => {
            write_envelope(
                &mut *lock_writer(writer)?,
                &ServeEnvelope::err("protocol", format!("unknown op '{other}'")).with_id(id),
            )?;
            Ok(LineResult::Reply)
        }
    }
}

fn lock_writer<W: Write>(writer: &Arc<Mutex<W>>) -> io::Result<std::sync::MutexGuard<'_, W>> {
    writer
        .lock()
        .map_err(|err| io::Error::other(format!("serve writer poisoned: {err}")))
}

fn write_json_line<W: Write>(writer: &mut W, value: &impl serde::Serialize) -> io::Result<()> {
    let json = serde_json::to_string(value)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn write_envelope<W: Write>(writer: &mut W, envelope: &ServeEnvelope) -> io::Result<()> {
    write_json_line(writer, envelope)
}

fn write_progress<W: Write>(
    writer: &mut W,
    id: &Option<serde_json::Value>,
    event: &dql_engine::ProgressEvent,
) -> io::Result<()> {
    write_json_line(writer, &ServeProgress::from_event(id.clone(), event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use std::io::Cursor;

    fn exchange(session: &mut Session, lines: &[&str]) -> (Vec<serde_json::Value>, Control) {
        let input = lines.join("\n") + "\n";
        let buf = Capture::default();
        let control = run_framed(Cursor::new(input), buf.clone(), session).unwrap();
        let replies = buf
            .text()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|value| value.get("ok").is_some())
            .collect();
        (replies, control)
    }

    #[derive(Clone, Default)]
    struct Capture(std::sync::Arc<Mutex<Vec<u8>>>);

    impl Capture {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ping_and_shutdown() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, control) = exchange(
            &mut session,
            &[r#"{"id":"1","op":"ping"}"#, r#"{"id":"2","op":"shutdown"}"#],
        );
        assert_eq!(control, Control::Shutdown);
        assert_eq!(replies[0]["ok"], true);
        assert_eq!(replies[0]["id"], "1");
        assert_eq!(replies[1]["id"], "2");
    }

    #[test]
    fn eof_is_disconnect() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, control) = exchange(&mut session, &[r#"{"op":"ping"}"#]);
        assert_eq!(control, Control::Disconnect);
        assert_eq!(replies.len(), 1);
        assert!(replies[0]["ok"].as_bool().unwrap());
    }

    #[test]
    fn exec_create_insert_select_and_meta() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, _) = exchange(
            &mut session,
            &[
                r#"{"id":"c","op":"exec","dql":"CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SELECT * FROM t WHERE id = 'a';"}"#,
                r#"{"id":"opt","op":"exec","dql":"opt"}"#,
                r#"{"id":"ls","op":"exec","dql":"ls"}"#,
                r#"{"id":"w","op":"exec","dql":"watch"}"#,
                r#"{"id":"s","op":"shutdown"}"#,
            ],
        );
        assert_eq!(replies[0]["ok"], true);
        assert_eq!(replies[0]["kind"], "items");
        assert_eq!(replies[0]["items"][0]["id"], "a");
        assert_eq!(replies[1]["kind"], "text");
        assert!(replies[1]["message"].as_str().unwrap().contains("width"));
        assert_eq!(replies[2]["ok"], true);
        assert_eq!(replies[3]["ok"], false);
        assert_eq!(replies[3]["error"]["code"], "unsupported");
        assert!(session.history.entries().is_empty());
    }

    #[test]
    fn empty_exec_is_none() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, _) = exchange(&mut session, &[r#"{"op":"exec","dql":"  "}"#]);
        assert_eq!(replies[0]["kind"], "none");
        assert_eq!(replies[0]["ok"], true);
    }

    #[test]
    fn unknown_op_and_bad_json() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, _) = exchange(
            &mut session,
            &[r#"{"id":"1","op":"nope"}"#, "not json", r#"{"op":"ping"}"#],
        );
        assert_eq!(replies[0]["error"]["code"], "protocol");
        assert_eq!(replies[1]["error"]["code"], "protocol");
        assert!(replies[1]["id"].is_null());
        assert_eq!(replies[2]["ok"], true);
    }

    #[test]
    fn parse_error_does_not_poison_session() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, _) = exchange(
            &mut session,
            &[
                r#"{"op":"exec","dql":"DROP nope;"}"#,
                r#"{"op":"exec","dql":"CREATE TABLE t (id STRING HASH KEY);"}"#,
            ],
        );
        assert_eq!(replies[0]["error"]["code"], "parse");
        assert_eq!(replies[1]["ok"], true);
        assert_eq!(replies[1]["kind"], "status");
    }

    #[test]
    fn interrupt_is_noop() {
        let mut session = Session::new_memory_headless("us-west-1");
        let (replies, _) = exchange(&mut session, &[r#"{"id":"i","op":"interrupt"}"#]);
        assert_eq!(replies[0]["ok"], true);
        assert_eq!(replies[0]["kind"], "none");
    }

    #[test]
    fn exec_streams_write_progress_before_envelope() {
        let mut session = Session::new_memory_headless("us-west-1");
        let created = session.execute_for_serve("CREATE TABLE t (id STRING HASH KEY);");
        assert!(created.ok, "{created:?}");
        let mut values = Vec::new();
        for i in 0..30 {
            values.push(format!("('{i}')"));
        }
        let sql = format!("INSERT INTO t (id) VALUES {}", values.join(", "));
        let input = format!(
            "{}\n",
            serde_json::json!({"id":"bulk","op":"exec","dql":sql})
        );
        let buf = Capture::default();
        run_framed(Cursor::new(input), buf.clone(), &mut session).unwrap();
        let lines: Vec<serde_json::Value> = buf
            .text()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let progress: Vec<_> = lines
            .iter()
            .filter(|line| line.get("event") == Some(&serde_json::json!("progress")))
            .collect();
        assert_eq!(progress[0]["done"], 0);
        assert_eq!(progress[0]["total"], 30);
        assert_eq!(progress[0]["phase"], "write");
        assert_eq!(progress[0]["id"], "bulk");
        assert!(progress.iter().any(|line| line["done"] == 25));
        assert_eq!(progress.last().unwrap()["done"], 30);
        let envelope = lines
            .iter()
            .find(|line| line.get("ok") == Some(&serde_json::json!(true)))
            .unwrap();
        assert_eq!(envelope["kind"], "affected");
        assert_eq!(envelope["affected"], 30);
    }
}
