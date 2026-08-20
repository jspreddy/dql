use crate::meta::lifecycle;
use crate::session::Session;
use std::collections::HashMap;
use std::io::Write;

const USAGE: &str = "Use: history | history search <string> | history dedupe | history remove --exact \"string\" | history edit <editor-name>";

pub fn handle(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => list_history(session, out),
        Some("search") => {
            let query = args
                .get(1..)
                .map(|parts| parts.join(" "))
                .unwrap_or_default();
            if query.trim().is_empty() {
                return Err("usage: history search <string>".to_string());
            }
            search_history(session, &query, out)
        }
        Some("dedupe") => dedupe_history(session, out),
        Some("remove") => {
            let exact = parse_exact_flag(&args[1..], kwargs)?;
            remove_exact_history(session, &exact, out)
        }
        Some("edit") => {
            let editor = args
                .get(1)
                .map(String::as_str)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "usage: history edit <editor-name>".to_string())?;
            if args.len() > 2 {
                return Err("usage: history edit <editor-name>".to_string());
            }
            edit_history(session, editor, out)
        }
        Some(other) => Err(format!("Unknown history subcommand {other:?}. {USAGE}")),
    }
}

fn list_history(session: &mut Session, out: &mut dyn Write) -> Result<(), String> {
    let entries = session.history.snapshot();
    if entries.is_empty() {
        writeln!(out, "(empty)").map_err(|err| err.to_string())?;
        return Ok(());
    }
    for (idx, entry) in entries.iter().enumerate() {
        write_entry(out, idx + 1, entry)?;
    }
    Ok(())
}

fn search_history(session: &mut Session, query: &str, out: &mut dyn Write) -> Result<(), String> {
    let entries = session.history.snapshot();
    let needle = query.to_ascii_lowercase();
    let mut matched = 0usize;
    for (idx, entry) in entries.iter().enumerate() {
        if entry.to_ascii_lowercase().contains(&needle) {
            write_entry(out, idx + 1, entry)?;
            matched += 1;
        }
    }
    if matched == 0 {
        writeln!(out, "No matches for {query:?}").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn dedupe_history(session: &mut Session, out: &mut dyn Write) -> Result<(), String> {
    let (kept, removed) = session.history.dedupe()?;
    writeln!(
        out,
        "Deduped history: kept {kept}, removed {removed} (duplicates and skippable commands)"
    )
    .map_err(|err| err.to_string())
}

fn remove_exact_history(
    session: &mut Session,
    exact: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    let removed = session.history.remove_exact(exact)?;
    writeln!(
        out,
        "Removed {removed} matching history entr{}.",
        if removed == 1 { "y" } else { "ies" }
    )
    .map_err(|err| err.to_string())
}

fn edit_history(session: &mut Session, editor: &str, out: &mut dyn Write) -> Result<(), String> {
    session.history.try_to_write_history();
    let path = session.history.history_file();
    if !path.is_file() {
        return Err(format!("History file not found: {}", path.display()));
    }
    writeln!(
        out,
        "Opening {} with {editor}; exiting dql to avoid history races.",
        path.display()
    )
    .map_err(|err| err.to_string())?;
    lifecycle::request_history_edit(editor.to_string(), path);
    Ok(())
}

fn parse_exact_flag(args: &[String], kwargs: &HashMap<String, String>) -> Result<String, String> {
    if let Some(value) = kwargs
        .get("--exact")
        .or_else(|| kwargs.get("exact"))
        .map(String::as_str)
        .filter(|value| !value.is_empty())
    {
        return Ok(value.to_string());
    }

    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == "--exact" {
            let value = args
                .get(idx + 1)
                .map(String::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "usage: history remove --exact \"string\"".to_string())?;
            return Ok(value.to_string());
        }
        if let Some(value) = arg.strip_prefix("--exact=") {
            if value.is_empty() {
                return Err("usage: history remove --exact \"string\"".to_string());
            }
            return Ok(value.to_string());
        }
        idx += 1;
    }

    Err("usage: history remove --exact \"string\"".to_string())
}

fn write_entry(out: &mut dyn Write, number: usize, entry: &str) -> Result<(), String> {
    let mut lines = entry.lines();
    let first = lines.next().unwrap_or("");
    writeln!(out, "{number:>5}  {first}").map_err(|err| err.to_string())?;
    for line in lines {
        writeln!(out, "       {line}").map_err(|err| err.to_string())?;
    }
    Ok(())
}
