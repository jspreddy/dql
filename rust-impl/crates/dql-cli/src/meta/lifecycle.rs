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
) -> Result<(), String> {
    println!(env!("CARGO_PKG_VERSION"));
    Ok(())
}

pub fn exit(
    session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    let _ = session.history.remove_items(1);
    SHOULD_EXIT.with(|flag| flag.set(true));
    Ok(())
}

pub fn clear(
    session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    let _ = session.history.remove_items(1);
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn shell(
    _session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    if args.is_empty() {
        return Err("shell requires a command".to_string());
    }
    let output = Command::new(&args[0])
        .args(&args[1..])
        .output()
        .map_err(|err| err.to_string())?;
    io::stdout()
        .write_all(&output.stdout)
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
) -> Result<(), String> {
    println!("{}", session.engine.session_identity());
    Ok(())
}

pub fn help(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    if let Some(topic) = args.first() {
        if let Some(text) = help::statement_help(topic) {
            print!("{text}");
            return Ok(());
        }
        return Err(format!("No help available for {topic}"));
    }
    println!("{}", help::GENERAL);
    let _ = session;
    Ok(())
}

pub fn watch_disabled(
    _session: &mut Session,
    _: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    println!("watch is not enabled in this build (rebuild with --features watch)");
    Ok(())
}
