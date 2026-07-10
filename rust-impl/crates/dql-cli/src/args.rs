use clap::Parser;
use std::env;

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "dql",
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

    #[arg(long = "version", help = "Print the version and exit")]
    pub version: bool,

    #[arg(short = 'h', long = "help", help = "Print help")]
    pub help: bool,
}

fn default_region() -> String {
    env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".to_string())
}

pub fn help_text() -> String {
    "Start the DQL client.\n\n\
     Options:\n\
       -c, --command <command>  Run this command and exit\n\
       -r, --region <region>    AWS region to connect to\n\
       -H, --host <host>        Host to connect to if using a local instance\n\
       -p, --port <port>        Port to connect to\n\
           --json               When used with --command, format results as JSON\n\
           --version            Print the version and exit\n\n\
     Environment:\n\
       AWS_REGION               Default region (else us-west-1)\n\
       DQL_BACKEND=memory       Use in-memory backend instead of live AWS"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_flag() {
        let args = CliArgs::try_parse_from(["dql", "--version"]).unwrap();
        assert!(args.version);
    }

    #[test]
    fn parses_command_and_local_flags() {
        let args = CliArgs::try_parse_from([
            "dql",
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
