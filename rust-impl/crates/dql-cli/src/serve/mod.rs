pub mod protocol;
mod stdio;
pub mod tcp;

use crate::session::Session;
use protocol::{parse_request_line, ServeEnvelope};
use std::io::{self, BufRead, Write};
use std::net::SocketAddr;

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
pub fn run_framed<R, W>(mut reader: R, mut writer: W, session: &mut Session) -> io::Result<Control>
where
    R: BufRead,
    W: Write,
{
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(Control::Disconnect),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                write_envelope(
                    &mut writer,
                    &ServeEnvelope::err("protocol", "invalid UTF-8"),
                )?;
                continue;
            }
            Err(err) => return Err(err),
        }

        if line.trim().is_empty() {
            continue;
        }

        match handle_line(session, line.trim_end()) {
            LineResult::Reply(envelope) => write_envelope(&mut writer, &envelope)?,
            LineResult::Shutdown(envelope) => {
                write_envelope(&mut writer, &envelope)?;
                return Ok(Control::Shutdown);
            }
        }
    }
}

enum LineResult {
    Reply(ServeEnvelope),
    Shutdown(ServeEnvelope),
}

fn handle_line(session: &mut Session, line: &str) -> LineResult {
    let request = match parse_request_line(line) {
        Ok(request) => request,
        Err(message) => return LineResult::Reply(ServeEnvelope::err("protocol", message)),
    };
    let id = request.id.clone();
    let op = request
        .op
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match op.as_str() {
        "ping" => LineResult::Reply(ServeEnvelope::success("none").with_id(id)),
        "shutdown" => LineResult::Shutdown(ServeEnvelope::success("none").with_id(id)),
        "interrupt" => LineResult::Reply(ServeEnvelope::success("none").with_id(id)),
        "exec" => {
            let dql = request.dql.as_deref().unwrap_or("");
            LineResult::Reply(session.execute_for_serve(dql).with_id(id))
        }
        "" => LineResult::Reply(ServeEnvelope::err("protocol", "missing op").with_id(id)),
        other => LineResult::Reply(
            ServeEnvelope::err("protocol", format!("unknown op '{other}'")).with_id(id),
        ),
    }
}

fn write_envelope<W: Write>(writer: &mut W, envelope: &ServeEnvelope) -> io::Result<()> {
    let json = serde_json::to_string(envelope)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use std::io::Cursor;

    fn exchange(session: &mut Session, lines: &[&str]) -> (Vec<serde_json::Value>, Control) {
        let input = lines.join("\n") + "\n";
        let mut out = Vec::new();
        let control = run_framed(Cursor::new(input), &mut out, session).unwrap();
        let replies = String::from_utf8(out)
            .unwrap()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (replies, control)
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
}
