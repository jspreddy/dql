pub mod connect;
pub mod file;
pub mod lifecycle;
pub mod ls;
pub mod opt;
pub mod throttle;

use crate::session::Session;
use dql_engine::StatementResult;
use std::collections::HashMap;

pub struct ReplCommand {
    pub handler: fn(&mut Session, &[String], &HashMap<String, String>) -> Result<(), String>,
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
) -> Result<Option<StatementResult>, dql_engine::EngineError> {
    let (command, arglist) = match line.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, rest.trim()),
        None => (line, ""),
    };
    let name = command.to_ascii_lowercase();
    if let Some(entry) = COMMANDS.get(name.as_str()) {
        let (args, kwargs) = parse_repl_args(arglist);
        (entry.handler)(session, &args, &kwargs).map_err(dql_engine::EngineError::Runtime)?;
        return Ok(None);
    }
    session.apply_rate_limit()?;
    session.engine.execute_fragment(line)
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
    register!("use", crate::meta::connect::handle_use);
    register!("local", crate::meta::connect::handle_local);
    register!("file", crate::meta::file::handle);
    register!("ls", crate::meta::ls::handle);
    register!("throttle", crate::meta::throttle::handle_throttle);
    register!("unthrottle", crate::meta::throttle::handle_unthrottle);
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
}
