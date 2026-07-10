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
        "width" => {
            writeln!(out, "width: {}", session.config.width).map_err(|err| err.to_string())?
        }
        "pagesize" => {
            writeln!(out, "pagesize: {}", session.config.pagesize).map_err(|err| err.to_string())?
        }
        "display" => print_enum_option(
            out,
            &session.config.display,
            dql_output::DisplayMode::all_names(),
        )?,
        "format" => print_enum_option(
            out,
            &session.config.format,
            dql_output::OutputFormat::all_names(),
        )?,
        "allow_select_scan" => writeln!(
            out,
            "allow_select_scan: {}",
            session.config.allow_select_scan
        )
        .map_err(|err| err.to_string())?,
        "lossy_json_float" => {
            writeln!(out, "lossy_json_float: {}", session.config.lossy_json_float)
                .map_err(|err| err.to_string())?
        }
        other => return Err(format!("Unrecognized option {other:?}")),
    }
    Ok(())
}

fn print_enum_option(out: &mut dyn Write, current: &str, choices: &[&str]) -> Result<(), String> {
    for choice in choices {
        let marker = if *choice == current { "* " } else { "  " };
        writeln!(out, "{marker}{choice}").map_err(|err| err.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CliConfig;
    use crate::history::HistoryManager;
    use crate::session::Session;
    use crate::throttle::TableLimits;

    fn test_session() -> Session {
        Session {
            config: CliConfig::default(),
            engine: crate::session::RuntimeEngine::in_memory(true),
            local_endpoint: None,
            region: "memory".to_string(),
            history: HistoryManager::new(),
            throttle: TableLimits::default(),
        }
    }

    #[test]
    fn set_format_rich_is_accepted() {
        let mut session = test_session();
        let mut out = Vec::new();
        set_option(
            &mut session,
            "format",
            &["rich".to_string()],
            &HashMap::new(),
            &mut out,
        )
        .unwrap();
        assert_eq!(session.config.format, "rich");
        print_option(&session, "format", &mut out).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(rendered.contains("* rich"));
        assert!(rendered.contains("  smart"));
        assert!(rendered.contains("  column"));
    }

    #[test]
    fn print_format_lists_available_formats() {
        let session = test_session();
        let mut out = Vec::new();
        print_option(&session, "format", &mut out).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(rendered.contains("* smart"));
        for name in dql_output::OutputFormat::all_names() {
            assert!(rendered.contains(name), "missing format {name}");
        }
    }
}
