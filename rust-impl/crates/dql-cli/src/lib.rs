mod api_token;
pub mod args;
mod cache_seal;
pub mod config;
pub mod error;
pub mod help;
pub mod history;
mod local_tables;
pub mod mcp;
pub mod meta;
pub mod notebook;
pub mod repl;
pub mod serve;
pub mod session;
pub mod table_cache;
pub mod throttle;
pub mod web;
pub mod workspace_cache;

use args::{help_text, CliArgs};
use clap::Parser;
use color_eyre::eyre::{bail, WrapErr};
use session::Session;
use std::io::{self, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn set_default_env(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        std::env::set_var(key, value);
    }
}

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
    "--mcp",
    "--listen",
    "--dir",
    "--load-tables",
    "--init-examples",
    "--yes",
    "--version",
    "-h",
    "--help",
];

pub fn run() -> color_eyre::Result<()> {
    let mut argv: Vec<String> = std::env::args().collect();
    if notebook::is_notebook_invocation(&argv) {
        return notebook::run(&argv);
    }
    let web = args::take_web_subcommand(&mut argv);
    let mut args_iter = argv.iter().skip(1);
    while let Some(arg) = args_iter.next() {
        if arg == "--web" {
            bail!("--web is now a subcommand: run `dqlrs web` (with the same options)");
        }
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
                    | "--listen"
                    | "--dir"
            ) {
                args_iter.next();
            }
        }
    }

    let mut args = CliArgs::try_parse_from(&argv).wrap_err("failed to parse CLI arguments")?;
    args.web = web;

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
    if args.listen.is_some() && !args.web {
        bail!("--listen requires `dqlrs web`");
    }
    if args.dir.is_some() && !args.web && !args.mcp {
        bail!("--dir requires `dqlrs web` or --mcp");
    }
    if args.load_tables && !args.web {
        bail!("--load-tables requires `dqlrs web`");
    }
    if args.init_examples && !args.web {
        bail!("--init-examples requires `dqlrs web`");
    }
    if args.yes && !args.init_examples {
        bail!("--yes requires `dqlrs web --init-examples`");
    }
    if args.serve && args.command.is_some() {
        bail!("--serve cannot be used with -c/--command");
    }
    if args.web && args.serve {
        bail!("`dqlrs web` cannot be used with --serve");
    }
    if args.web && args.command.is_some() {
        bail!("`dqlrs web` cannot be used with -c/--command");
    }
    if args.mcp && (args.serve || args.command.is_some()) {
        bail!("--mcp cannot be used with --serve or -c/--command");
    }

    if (args.web || args.mcp) && args.host.is_some() {
        set_default_env("AWS_ACCESS_KEY_ID", "fakeid");
        set_default_env("AWS_SECRET_ACCESS_KEY", "fakekey");
        set_default_env("AWS_EC2_METADATA_DISABLED", "true");
    }

    if args.web {
        let listen = match args.listen.as_deref() {
            Some(spec) => {
                serve::parse_bind(spec).map_err(|err| color_eyre::eyre::eyre!("{err}"))?
            }
            None => "127.0.0.1:8765"
                .parse()
                .expect("default web listen address"),
        };
        // --init-examples runs first: the session creates .dqlrs-cache in the workspace, and
        // --init-examples needs the folder empty.
        if args.init_examples {
            // Absolute, so the plan names the real folder. It may not exist yet.
            let cwd = std::env::current_dir().wrap_err("current directory")?;
            let dir = match &args.dir {
                Some(dir) => cwd.join(dir),
                None => cwd,
            };
            if !web::init::run(&dir, args.yes)? {
                return Ok(());
            }
        }
        // The workspace is --dir or the current directory; Session resolves it once.
        let session = Session::new_for_serve(&args).wrap_err("failed to start session")?;
        let dir = session.workspace.clone();
        web::run(session, dir, listen, args.mcp, args.load_tables)?;
        return Ok(());
    }

    if args.mcp {
        // The workspace is --dir or the current directory; Session resolves it once.
        let session = Session::new_for_serve(&args).wrap_err("failed to start session")?;
        let dir = session.workspace.clone();
        mcp::run(session, dir)?;
        return Ok(());
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
