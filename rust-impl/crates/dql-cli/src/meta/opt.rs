use crate::session::Session;
use std::collections::HashMap;
use std::io::Write;

pub fn handle(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    if args.is_empty() && kwargs.is_empty() {
        let keys: Vec<_> = session
            .config
            .public_keys()
            .into_iter()
            .filter(|key| !key.starts_with('_'))
            .collect();
        let largest = keys.iter().map(|key| key.len()).max().unwrap_or(0);
        for key in keys {
            let value = session.config.get_value(&key);
            writeln!(out, "{:>width$} : {}", key, value, width = largest)
                .map_err(|err| err.to_string())?;
        }
        return Ok(());
    }
    if args.is_empty() {
        return Err("opt requires an option name".to_string());
    }
    let option = &args[0];
    let rest = &args[1..];
    if rest.is_empty() && kwargs.is_empty() {
        print_option(session, option, out)?;
    } else {
        set_option(session, option, rest, kwargs, out)?;
        session.config.save().map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn print_option(session: &Session, option: &str, out: &mut dyn Write) -> Result<(), String> {
    match option {
        "width" => writeln!(out, "width: {}", session.config.width)
            .map_err(|err| err.to_string())?,
        "pagesize" => writeln!(out, "pagesize: {}", session.config.pagesize)
            .map_err(|err| err.to_string())?,
        "display" => writeln!(out, "display: {}", session.config.display)
            .map_err(|err| err.to_string())?,
        "format" => writeln!(out, "format: {}", session.config.format)
            .map_err(|err| err.to_string())?,
        "allow_select_scan" => {
            writeln!(out, "allow_select_scan: {}", session.config.allow_select_scan)
                .map_err(|err| err.to_string())?
        }
        "lossy_json_float" => {
            writeln!(out, "lossy_json_float: {}", session.config.lossy_json_float)
                .map_err(|err| err.to_string())?
        }
        other => return Err(format!("Unrecognized option {other:?}")),
    }
    Ok(())
}

fn set_option(
    session: &mut Session,
    option: &str,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    match option {
        "width" => {
            let value = args.first().map(String::as_str).unwrap_or("auto");
            session.config.width = if value == "auto" {
                serde_json::Value::String("auto".to_string())
            } else {
                serde_json::Value::Number(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid width {value}"))?
                        .into(),
                )
            };
        }
        "pagesize" => {
            let value = args.first().map(String::as_str).unwrap_or("auto");
            session.config.pagesize = if value == "auto" {
                serde_json::Value::String("auto".to_string())
            } else {
                serde_json::Value::Number(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid pagesize {value}"))?
                        .into(),
                )
            };
        }
        "display" => {
            let value = args.first().map(String::as_str).unwrap_or_default();
            if dql_output::DisplayMode::from_name(value).is_some() {
                session.config.display = value.to_string();
                writeln!(out, "Set display {value:?}").map_err(|err| err.to_string())?;
            } else {
                return Err(format!("Unknown display {value:?}"));
            }
        }
        "format" => {
            let value = args.first().map(String::as_str).unwrap_or_default();
            if dql_output::OutputFormat::from_name(value).is_some() {
                session.config.format = value.to_string();
                writeln!(out, "Set format {value:?}").map_err(|err| err.to_string())?;
            } else {
                return Err(format!("Unknown format {value:?}"));
            }
        }
        "allow_select_scan" => {
            let value = parse_bool(args, kwargs, "allow_select_scan")?;
            session.config.allow_select_scan = value;
            session.engine.apply_allow_select_scan(value);
        }
        "lossy_json_float" => {
            let value = parse_bool(args, kwargs, "lossy_json_float")?;
            session.config.lossy_json_float = value;
        }
        other => return Err(format!("Unrecognized option {other:?}")),
    }
    Ok(())
}

fn parse_bool(
    args: &[String],
    kwargs: &HashMap<String, String>,
    key: &str,
) -> Result<bool, String> {
    let raw = args
        .first()
        .map(String::as_str)
        .or_else(|| kwargs.get(key).map(String::as_str))
        .ok_or_else(|| format!("missing value for {key}"))?;
    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => Err(format!("invalid boolean {other}")),
    }
}
