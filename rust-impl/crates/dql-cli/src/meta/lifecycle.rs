use crate::help;
use crate::session::Session;
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::Command;

thread_local! {
    static SHOULD_EXIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn take_exit_request() -> bool {
    SHOULD_EXIT.with(|flag| {
        let value = flag.get();
        flag.set(false);
        value
    })
}

pub fn version(
    _session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    writeln!(out, "{}", env!("CARGO_PKG_VERSION")).map_err(|err| err.to_string())
}

pub fn exit(
    session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
    _out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    let _ = session.history.remove_items(1);
    SHOULD_EXIT.with(|flag| flag.set(true));
    Ok(())
}

pub fn clear(
    session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
    _out: &mut dyn Write,
    repl: bool,
) -> Result<(), String> {
    let _ = session.history.remove_items(1);
    if !repl {
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn shell(
    _session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    if args.is_empty() {
        return Err("shell requires a command".to_string());
    }
    let output = Command::new(&args[0])
        .args(&args[1..])
        .output()
        .map_err(|err| err.to_string())?;
    out.write_all(&output.stdout)
        .map_err(|err| err.to_string())?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn whoami(
    session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    writeln!(out, "{}", session.engine.session_identity()).map_err(|err| err.to_string())
}

pub fn help(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    if let Some(topic) = args.first() {
        if let Some(text) = help::statement_help(topic) {
            write!(out, "{text}").map_err(|err| err.to_string())?;
            return Ok(());
        }
        return Err(format!("No help available for {topic}"));
    }
    writeln!(out, "{}", help::GENERAL).map_err(|err| err.to_string())?;
    let _ = session;
    Ok(())
}

pub fn watch_disabled(
    _session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    writeln!(
        out,
        "watch is not enabled in this build (rebuild with --features watch)"
    )
    .map_err(|err| err.to_string())
}
