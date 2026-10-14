pub mod connect;
pub mod file;
pub mod history_cmd;
pub mod lifecycle;
pub mod ls;
pub mod opt;
pub mod throttle;
#[cfg(feature = "watch")]
pub mod watch;

use crate::session::Session;
use dql_engine::StatementResult;
use std::collections::HashMap;
use std::io::Write;

type ReplHandler = fn(
    &mut Session,
    &[String],
    &HashMap<String, String>,
    &mut dyn Write,
    bool,
) -> Result<(), String>;

pub struct ReplCommand {
    pub handler: ReplHandler,
}

pub fn parse_repl_args(arglist: &str) -> (Vec<String>, HashMap<String, String>) {
    let mut args = Vec::new();
    let mut kwargs = HashMap::new();
    if arglist.trim().is_empty() {
        return (args, kwargs);
    }
    for arg in shell_words::split(arglist).unwrap_or_default() {
        if let Some((key, value)) = arg.split_once('=') {
            kwargs.insert(key.to_string(), value.to_string());
        } else {
            args.push(arg);
        }
    }
    (args, kwargs)
}

pub fn dispatch(
    session: &mut Session,
    line: &str,
    out: &mut dyn Write,
    repl: bool,
) -> Result<Option<StatementResult>, dql_engine::EngineError> {
    let trimmed = line.trim();
    // Meta-commands may be typed with a trailing ';' (`ls;`). Strip that mark
    // before matching the command name — it is not part of a flag value
    // (`refresh=True;`).
    let for_meta = trimmed.trim_end_matches(';').trim();
    let (command, arglist) = match for_meta.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, rest.trim()),
        None => (for_meta, ""),
    };
    let name = command.to_ascii_lowercase();
    if let Some(entry) = COMMANDS.get(name.as_str()) {
        let (args, kwargs) = parse_repl_args(arglist);
        (entry.handler)(session, &args, &kwargs, out, repl)
            .map_err(dql_engine::EngineError::Runtime)?;
        return Ok(None);
    }
    session.apply_rate_limit()?;
    // Keep the user's ';'. `parse_fragment` requires a terminator for Complete;
    // do not strip it here (meta matching used `for_meta` above) and do not
    // invent one — the REPL only submits SQL when the buffer ends with ';'.
    session.engine.execute_fragment(trimmed)
}

fn registry() -> HashMap<&'static str, ReplCommand> {
    let mut commands = HashMap::new();
    macro_rules! register {
        ($name:expr, $handler:expr) => {
            commands.insert($name, ReplCommand { handler: $handler });
        };
    }
    register!("opt", crate::meta::opt::handle);
    register!("version", crate::meta::lifecycle::version);
    register!("exit", crate::meta::lifecycle::exit);
    register!("quit", crate::meta::lifecycle::exit);
    register!("clear", crate::meta::lifecycle::clear);
    register!("cls", crate::meta::lifecycle::clear);
    register!("c", crate::meta::lifecycle::clear);
    register!("shell", crate::meta::lifecycle::shell);
    register!("whoami", crate::meta::lifecycle::whoami);
    register!("iam", crate::meta::lifecycle::whoami);
    register!("help", crate::meta::lifecycle::help);
    register!("history", crate::meta::history_cmd::handle);
    register!("use", crate::meta::connect::handle_use);
    register!("local", crate::meta::connect::handle_local);
    register!("file", crate::meta::file::handle);
    register!("ls", crate::meta::ls::handle);
    register!("throttle", crate::meta::throttle::handle_throttle);
    register!("unthrottle", crate::meta::throttle::handle_unthrottle);
    #[cfg(feature = "watch")]
    register!("watch", crate::meta::watch::handle);
    #[cfg(not(feature = "watch"))]
    register!("watch", crate::meta::lifecycle::watch_disabled);
    commands
}

static COMMANDS: std::sync::LazyLock<HashMap<&'static str, ReplCommand>> =
    std::sync::LazyLock::new(registry);

pub fn command_names() -> Vec<&'static str> {
    let mut names: Vec<_> = COMMANDS.keys().copied().collect();
    names.sort_unstable();
    names
}

pub fn is_meta_command(command: &str) -> bool {
    COMMANDS.contains_key(command.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_command_args() {
        let (args, kwargs) = parse_repl_args("a b");
        assert_eq!(args, vec!["a", "b"]);
        assert!(kwargs.is_empty());
    }

    #[test]
    fn test_repl_command_kwargs() {
        let (args, kwargs) = parse_repl_args("a second=b");
        assert_eq!(args, vec!["a"]);
        assert_eq!(kwargs.get("second"), Some(&"b".to_string()));
    }

    #[test]
    fn sql_dispatch_keeps_semicolon_for_fragment_parser() {
        // Regression: stripping ';' before execute_fragment left every SQL
        // statement Incomplete (parse_fragment requires a terminator).
        let trimmed = "SELECT name FROM t LIMIT 1;".trim();
        let for_meta = trimmed.trim_end_matches(';').trim();
        assert_eq!(for_meta, "SELECT name FROM t LIMIT 1");
        assert!(
            matches!(
                dql_parser::parse_fragment(trimmed),
                dql_parser::FragmentStatus::Complete(_)
            ),
            "SQL passed to execute_fragment must keep its ';'"
        );
        assert!(
            matches!(
                dql_parser::parse_fragment(for_meta),
                dql_parser::FragmentStatus::Incomplete
            ),
            "stripped SQL must not be used with execute_fragment"
        );
    }

    #[test]
    fn meta_commands_are_recognized_without_semicolon() {
        assert!(is_meta_command("ls"));
        assert!(is_meta_command("opt"));
        assert!(!is_meta_command("SELECT"));
        assert!(!is_meta_command("scan"));
    }
}
