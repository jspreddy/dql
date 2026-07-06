use crate::session::Session;
use crate::throttle::TableLimits;
use std::collections::HashMap;

pub fn handle_throttle(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    if args.is_empty() {
        println!("{}", session.throttle);
        return Ok(());
    }
    if args.len() < 2 {
        return Err("throttle requires read and write limits".to_string());
    }
    let read = &args[args.len() - 2];
    let write = &args[args.len() - 1];
    let prefix = &args[..args.len() - 2];
    match prefix.len() {
        0 => session.throttle.set_total_limit(read, write)?,
        1 if prefix[0] == "default" => session.throttle.set_default_limit(read, write),
        1 => session.throttle.set_table_limit(&prefix[0], read, write),
        2 => session
            .throttle
            .set_index_limit(&prefix[0], &prefix[1], read, write),
        _ => return Err("invalid throttle arguments".to_string()),
    }
    session.config.throttle = session.throttle.save();
    session.config.save().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn handle_unthrottle(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    if args.is_empty() {
        if promptyn("Remove all throttle limits?", false)? {
            session.throttle = TableLimits::default();
            session.config.throttle = session.throttle.save();
            session.config.save().map_err(|err| err.to_string())?;
        }
        return Ok(());
    }
    if args.len() == 1 {
        session.throttle.set_table_limit(&args[0], "0", "0");
    } else {
        session
            .throttle
            .set_index_limit(&args[0], &args[1], "0", "0");
    }
    session.config.throttle = session.throttle.save();
    session.config.save().map_err(|err| err.to_string())?;
    Ok(())
}

fn promptyn(message: &str, default: bool) -> Result<bool, String> {
    use std::io::{self, Write};
    let yes = if default { "Y" } else { "y" };
    let no = if default { "n" } else { "N" };
    print!("{message} [{yes}/{no}] ");
    io::stdout().flush().map_err(|err| err.to_string())?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|err| err.to_string())?;
    let confirm = line.trim().to_ascii_lowercase();
    match confirm.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        "" if default => Ok(true),
        "" if !default => Ok(false),
        _ => promptyn(message, default),
    }
}
