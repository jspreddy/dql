pub mod args;
pub mod config;
pub mod error;
pub mod help;
pub mod history;
pub mod meta;
pub mod repl;
pub mod serve;
pub mod session;
pub mod throttle;

use args::{help_text, CliArgs};
use clap::Parser;
use color_eyre::eyre::{bail, WrapErr};
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
    "--serve",
    "--bind",
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
                "-c" | "--command"
                    | "-r"
                    | "--region"
                    | "-H"
                    | "--host"
                    | "-p"
                    | "--port"
                    | "--bind"
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

    if args.bind.is_some() && !args.serve {
        bail!("--bind requires --serve");
    }
    if args.serve && args.command.is_some() {
        bail!("--serve cannot be used with -c/--command");
    }

    if args.serve {
        let bind = match args.bind.as_deref() {
            Some(spec) => {
                Some(serve::parse_bind(spec).map_err(|err| color_eyre::eyre::eyre!("{err}"))?)
            }
            None => None,
        };
        let mut session = Session::new_for_serve(&args).wrap_err("failed to start session")?;
        serve::run(&mut session, bind)?;
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
