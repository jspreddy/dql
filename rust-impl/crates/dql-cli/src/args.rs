use clap::Parser;
use std::env;

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "dqlrs",
    about = "Start the DQL client.",
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct CliArgs {
    #[arg(short = 'c', long = "command", help = "Run this command and exit")]
    pub command: Option<String>,

    #[arg(
        short = 'r',
        long = "region",
        default_value_t = default_region(),
        help = "AWS region to connect to"
    )]
    pub region: String,

    #[arg(
        short = 'H',
        long = "host",
        help = "Host to connect to if using a local instance"
    )]
    pub host: Option<String>,

    #[arg(
        short = 'p',
        long = "port",
        default_value_t = 8000,
        help = "Port to connect to"
    )]
    pub port: u16,

    #[arg(
        long = "json",
        help = "When used with --command, format the results as JSON"
    )]
    pub json: bool,

    #[arg(
        long = "serve",
        help = "Headless JSON-lines worker (stdio, or --bind on loopback). Not a TTY REPL."
    )]
    pub serve: bool,

    #[arg(
        long = "bind",
        value_name = "ADDR",
        help = "With --serve, listen on loopback HOST:PORT (or PORT as 127.0.0.1:PORT)"
    )]
    pub bind: Option<String>,

    /// Set by the `web` subcommand (`dqlrs web …`), not a flag. See [`take_web_subcommand`].
    #[arg(skip)]
    pub web: bool,

    #[arg(
        long = "mcp",
        help = "MCP tools. Alone, stdio. With `web`, HTTP at /mcp on the web host."
    )]
    pub mcp: bool,

    #[arg(
        long = "listen",
        value_name = "ADDR",
        help = "With `web`, listen on loopback HOST:PORT (default 127.0.0.1:8765)"
    )]
    pub listen: Option<String>,

    #[arg(
        long = "dir",
        value_name = "DIR",
        help = "With `web` or --mcp, folder of .dql files (default: current directory)"
    )]
    pub dir: Option<String>,

    #[arg(
        long = "load-tables",
        help = "With `web`, load every table and its definition in the background"
    )]
    pub load_tables: bool,

    #[arg(
        long = "init-examples",
        help = "With `web`, after a confirm: git init an empty folder, copy the examples in, commit"
    )]
    pub init_examples: bool,

    #[arg(
        long = "yes",
        help = "With `web --init-examples`, skip the confirmation"
    )]
    pub yes: bool,

    #[arg(long = "version", help = "Print the version and exit")]
    pub version: bool,

    #[arg(short = 'h', long = "help", help = "Print help")]
    pub help: bool,
}

/// Flags that take a value. Their values are skipped when looking for a subcommand word.
const VALUE_FLAGS: &[&str] = &[
    "-c",
    "--command",
    "-r",
    "--region",
    "-H",
    "--host",
    "-p",
    "--port",
    "--bind",
    "--listen",
    "--dir",
];

/// Remove a `web` subcommand word from `argv` and say whether it was there. The word can sit
/// before or after the options (`dqlrs web --dir x` or `dqlrs --dir x web`); a value that
/// follows a flag such as `-c web` is not the subcommand.
pub fn take_web_subcommand(argv: &mut Vec<String>) -> bool {
    let mut index = 1;
    while index < argv.len() {
        let arg = argv[index].as_str();
        if VALUE_FLAGS.contains(&arg) {
            index += 2;
            continue;
        }
        if arg == "web" {
            argv.remove(index);
            return true;
        }
        index += 1;
    }
    false
}

fn default_region() -> String {
    env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".to_string())
}

pub fn help_text() -> String {
    // One line per entry: a `\` continuation would strip the leading spaces.
    [
        "Start the DQL client.",
        "",
        "Usage:",
        "  dqlrs [options]          Start the REPL (or run -c)",
        "  dqlrs web [options]      Serve the query editor and table browser",
        "  dqlrs web --init-examples",
        "                           Start a new workspace in an empty folder from the examples",
        "  dqlrs notebook           Start JupyterLab with a DQL (Rust) kernel (this binary)",
        "",
        "Options:",
        "  -c, --command <command>  Run this command and exit",
        "  -r, --region <region>    AWS region to connect to",
        "  -H, --host <host>        Host to connect to if using a local instance",
        "  -p, --port <port>        Port to connect to",
        "      --json               When used with --command, format results as JSON",
        "      --serve              Headless JSON-lines worker (stdio, or --bind on loopback). Not a TTY REPL.",
        "      --bind <ADDR>        With --serve, listen on loopback HOST:PORT (or PORT as 127.0.0.1:PORT)",
        "      --mcp                MCP tools. Alone, stdio. With web, HTTP at /mcp",
        "      --listen <ADDR>      With web, listen on loopback HOST:PORT (default 127.0.0.1:8765)",
        "      --dir <DIR>          With web or --mcp, folder of .dql files (default: current directory)",
        "      --load-tables        With web, load every table and its definition in the background",
        "      --init-examples      With web, after a confirm: git init an empty folder, copy the",
        "                           examples in, and commit them",
        "      --yes                With web --init-examples, skip the confirmation",
        "      --version            Print the version and exit",
        "",
        "Environment:",
        "  AWS_REGION               Default region (else us-west-1)",
        "  DQL_BACKEND=memory       Use in-memory backend instead of live AWS",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    /// Parse the way `run` does: take the `web` word, parse the rest, then set `web`.
    fn parse(args: &[&str]) -> CliArgs {
        let mut list = argv(args);
        let web = take_web_subcommand(&mut list);
        let mut parsed = CliArgs::try_parse_from(&list).unwrap();
        parsed.web = web;
        parsed
    }

    #[test]
    fn web_is_a_subcommand_before_or_after_the_options() {
        let mut first = argv(&["dqlrs", "web", "--dir", "queries", "--listen", "9000"]);
        assert!(take_web_subcommand(&mut first));
        assert_eq!(
            first,
            argv(&["dqlrs", "--dir", "queries", "--listen", "9000"])
        );
        let parsed = CliArgs::try_parse_from(&first).unwrap();
        assert_eq!(parsed.dir.as_deref(), Some("queries"));

        let mut last = argv(&["dqlrs", "-H", "localhost", "-p", "8000", "web", "--mcp"]);
        assert!(take_web_subcommand(&mut last));
        assert_eq!(
            last,
            argv(&["dqlrs", "-H", "localhost", "-p", "8000", "--mcp"])
        );
    }

    #[test]
    fn a_flag_value_named_web_is_not_the_subcommand() {
        for args in [
            &["dqlrs", "-c", "web"][..],
            &["dqlrs", "--dir", "web"],
            &["dqlrs", "-H", "web", "-c", "LS"],
        ] {
            let mut list = argv(args);
            assert!(!take_web_subcommand(&mut list), "{args:?}");
            assert_eq!(list, argv(args));
        }
        let mut both = argv(&["dqlrs", "--dir", "web", "web"]);
        assert!(take_web_subcommand(&mut both));
        assert_eq!(both, argv(&["dqlrs", "--dir", "web"]));
    }

    #[test]
    fn web_is_no_longer_a_flag() {
        assert!(CliArgs::try_parse_from(["dqlrs", "--web"]).is_err());
        assert!(!CliArgs::try_parse_from(["dqlrs"]).unwrap().web);
    }

    #[test]
    fn parses_version_flag() {
        let args = CliArgs::try_parse_from(["dqlrs", "--version"]).unwrap();
        assert!(args.version);
    }

    #[test]
    fn parses_command_and_local_flags() {
        let args = CliArgs::try_parse_from([
            "dqlrs",
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
        assert!(!args.serve);
        assert!(args.bind.is_none());
    }

    #[test]
    fn parses_serve_and_bind() {
        let args =
            CliArgs::try_parse_from(["dqlrs", "--serve", "--bind", "127.0.0.1:7400"]).unwrap();
        assert!(args.serve);
        assert_eq!(args.bind.as_deref(), Some("127.0.0.1:7400"));
        assert!(args.command.is_none());
        assert!(!args.web);
    }

    #[test]
    fn parses_web_listen_and_dir() {
        let args = parse(&[
            "dqlrs",
            "web",
            "--listen",
            "127.0.0.1:9000",
            "--dir",
            "/tmp/queries",
            "-H",
            "localhost",
        ]);
        assert!(args.web);
        assert_eq!(args.listen.as_deref(), Some("127.0.0.1:9000"));
        assert_eq!(args.dir.as_deref(), Some("/tmp/queries"));
        assert_eq!(args.host.as_deref(), Some("localhost"));
        assert!(!args.serve);
        assert!(!args.mcp);
    }

    #[test]
    fn parses_mcp_dir() {
        let args = CliArgs::try_parse_from([
            "dqlrs",
            "--mcp",
            "--dir",
            "/tmp/queries",
            "-H",
            "localhost",
            "-p",
            "8000",
        ])
        .unwrap();
        assert!(args.mcp);
        assert_eq!(args.dir.as_deref(), Some("/tmp/queries"));
        assert!(!args.web);
        assert!(!args.serve);
    }

    #[test]
    fn parses_web_and_mcp_together() {
        let args = parse(&[
            "dqlrs",
            "web",
            "--mcp",
            "--listen",
            "127.0.0.1:8765",
            "--dir",
            "/tmp/queries",
        ]);
        assert!(args.web);
        assert!(args.mcp);
        assert_eq!(args.listen.as_deref(), Some("127.0.0.1:8765"));
        assert_eq!(args.dir.as_deref(), Some("/tmp/queries"));
        assert!(!args.load_tables);
    }

    #[test]
    fn parses_web_load_tables() {
        let args = parse(&["dqlrs", "web", "--load-tables"]);
        assert!(args.web);
        assert!(args.load_tables);
        assert!(help_text().contains("--load-tables"));
        // Entries keep their indentation.
        assert!(help_text().contains("\n  dqlrs web [options]"));
        assert!(help_text().contains("\n      --json "));
    }
}
