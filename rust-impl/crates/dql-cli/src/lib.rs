pub mod args;
pub mod config;
pub mod help;
pub mod history;
pub mod meta;
pub mod repl;
pub mod session;
pub mod throttle;

use clap::Parser;
use args::{help_text, CliArgs};
use session::Session;
use std::io::{self, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const KNOWN_FLAGS: &[&str] = &[
    "-c", "--command", "-r", "--region", "-H", "--host", "-p", "--port", "--json", "--version",
    "-h", "--help",
];

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();
    let mut args_iter = argv.iter().skip(1);
    while let Some(arg) = args_iter.next() {
        if arg.starts_with('-') {
            if !KNOWN_FLAGS.contains(&arg.as_str()) {
                eprintln!("unknown argument '{arg}'\n\n{}", help_text());
                return Ok(());
            }
            if matches!(
                arg.as_str(),
                "-c" | "--command" | "-r" | "--region" | "-H" | "--host" | "-p" | "--port"
            ) {
                args_iter.next();
            }
        }
    }

    let args = CliArgs::try_parse_from(&argv)?;

    if args.help {
        let mut stderr = io::stderr();
        stderr.write_all(help_text().as_bytes())?;
        return Ok(());
    }

    if args.version {
        println!("{VERSION}");
        return Ok(());
    }

    let mut session = Session::new(&args)?;
    if let Some(command) = args.command {
        if let Err(err) = session.run_command(command.trim(), args.json) {
            eprintln!("{err}");
        }
    } else if let Err(err) = repl::run_repl(&mut session) {
        eprintln!("{err}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::args::CliArgs;
    use clap::Parser;

    #[test]
    fn parses_version_flag() {
        let args = CliArgs::try_parse_from(["dql", "--version"]).unwrap();
        assert!(args.version);
    }
}
