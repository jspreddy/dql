pub mod args;
pub mod config;
pub mod error;
pub mod help;
pub mod history;
pub mod meta;
pub mod repl;
pub mod session;
pub mod throttle;

use args::{help_text, CliArgs};
use clap::Parser;
use color_eyre::eyre::WrapErr;
use session::Session;
use std::io::{self, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const KNOWN_FLAGS: &[&str] = &[
    "-c",
    "--command",
    "-r",
    "--region",
    "-H",
    "--host",
    "-p",
    "--port",
    "--json",
    "--version",
    "-h",
    "--help",
];

pub fn run() -> color_eyre::Result<()> {
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

    let args = CliArgs::try_parse_from(&argv).wrap_err("failed to parse CLI arguments")?;

    if args.help {
        let mut stderr = io::stderr();
        stderr.write_all(help_text().as_bytes())?;
        return Ok(());
    }

    if args.version {
        println!("{VERSION}");
        return Ok(());
    }

    let mut session = Session::new(&args).wrap_err("failed to start session")?;
    if let Some(command) = args.command {
        if let Err(err) = session.run_command(command.trim(), args.json) {
            eprintln!("{err}");
        }
    } else {
        repl::run_repl(&mut session).wrap_err("repl failed")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::args::CliArgs;
    use clap::Parser;

    #[test]
    fn parses_version_flag() {
        let args = CliArgs::try_parse_from(["dqlrs", "--version"]).unwrap();
        assert!(args.version);
    }
}
