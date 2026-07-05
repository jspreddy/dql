use dql_engine::{InMemoryEngine, StatementResult};
use std::env;
use std::error::Error;
use std::io::{self, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    command: Option<String>,
    region: String,
    host: Option<String>,
    port: u16,
    json: bool,
    version: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args().skip(1))?;
    if args.version {
        println!("{VERSION}");
        return Ok(());
    }

    let mut engine = InMemoryEngine::new();
    if let Some(command) = args.command {
        let result = engine.execute(command.trim())?;
        write_result(&result, args.json)?;
    } else {
        repl(&mut engine, &args)?;
    }
    Ok(())
}

fn parse_args<I, S>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut parsed = Args {
        command: None,
        region: env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".to_string()),
        host: None,
        port: 8000,
        json: false,
        version: false,
    };

    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--command" => {
                parsed.command = Some(
                    args.next()
                        .ok_or_else(|| format!("{arg} requires a command string"))?,
                );
            }
            "-r" | "--region" => {
                parsed.region = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a region"))?;
            }
            "-H" | "--host" => {
                parsed.host = Some(
                    args.next()
                        .ok_or_else(|| format!("{arg} requires a host"))?,
                );
            }
            "-p" | "--port" => {
                let port = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a port"))?;
                parsed.port = port.parse().map_err(|_| format!("invalid port '{port}'"))?;
            }
            "--json" => parsed.json = true,
            "--version" => parsed.version = true,
            "-h" | "--help" => return Err(help_text()),
            other => return Err(format!("unknown argument '{other}'\n\n{}", help_text())),
        }
    }
    Ok(parsed)
}

fn write_result(result: &StatementResult, json: bool) -> io::Result<()> {
    if json {
        print!("{}", result.to_json_lines());
    } else {
        print!("{result}");
    }
    io::stdout().flush()
}

fn repl(engine: &mut InMemoryEngine, args: &Args) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut buffer = String::new();
    loop {
        print_prompt(args, !buffer.is_empty())?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            println!();
            break;
        }
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("exit") {
            break;
        }
        buffer.push_str(&line);
        if !trimmed.ends_with(';') {
            continue;
        }
        match engine.execute(&buffer) {
            Ok(result) => print!("{result}"),
            Err(err) => eprintln!("{err}"),
        }
        buffer.clear();
    }
    Ok(())
}

fn print_prompt(args: &Args, partial: bool) -> io::Result<()> {
    if partial {
        print!("   | ");
    } else if let Some(host) = &args.host {
        print!("\n({host}:{}) {}\n   ===> ", args.port, args.region);
    } else {
        print!("\n{}\n   ===> ", args.region);
    }
    io::stdout().flush()
}

fn help_text() -> String {
    "Start the DQL client.\n\n\
     Options:\n\
       -c, --command <command>  Run this command and exit\n\
       -r, --region <region>    AWS region to connect to\n\
       -H, --host <host>        Host to connect to if using a local instance\n\
       -p, --port <port>        Port to connect to\n\
           --json               When used with --command, format results as JSON\n\
           --version            Print the version and exit"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_flag() {
        let args = parse_args(["--version"]).unwrap();
        assert!(args.version);
    }

    #[test]
    fn parses_command_and_local_flags() {
        let args = parse_args([
            "-H",
            "localhost",
            "-p",
            "8000",
            "-r",
            "us-east-1",
            "--json",
            "-c",
            "SCAN * FROM t",
        ])
        .unwrap();
        assert_eq!(args.host, Some("localhost".to_string()));
        assert_eq!(args.port, 8000);
        assert_eq!(args.region, "us-east-1");
        assert_eq!(args.command, Some("SCAN * FROM t".to_string()));
        assert!(args.json);
    }
}
