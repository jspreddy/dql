//! Parse `dqlrs notebook` / `dqlrs --notebook` arguments.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotebookArgs {
    pub help: bool,
    pub local: bool,
    pub no_browser: bool,
    pub dry_run: bool,
    pub jupyter_port: Option<u16>,
    pub region: Option<String>,
    pub host: Option<String>,
    pub dynamo_port: Option<u16>,
    pub jupyter_extra: Vec<String>,
    pub conflict_with_serve_or_command: bool,
}

pub fn is_notebook_invocation(argv: &[String]) -> bool {
    let mut saw_command = false;
    for arg in argv.iter().skip(1) {
        if saw_command {
            saw_command = false;
            continue;
        }
        match arg.as_str() {
            "notebook" | "--notebook" => return true,
            "-c" | "--command" | "-r" | "--region" | "-H" | "--host" | "-p" | "--port"
            | "--bind" => {
                saw_command = true;
            }
            _ => {}
        }
    }
    false
}

pub fn help_text() -> String {
    "Start JupyterLab with a DQL (Rust) kernel backed by this dqlrs binary.\n\n\
     Usage:\n\
       dqlrs notebook [options]\n\
       dqlrs --notebook [options]\n\n\
     Options:\n\
       --local              DynamoDB Local (localhost:8000) and dummy AWS keys if unset\n\
       --no-browser         Do not open a browser\n\
       --dry-run            Write kernel files, ensure Jupyter, list kernels, exit\n\
       --jupyter-port PORT  Jupyter port (else Jupyter picks one)\n\
       --port PORT          Same as --jupyter-port\n\
       -r, --region REGION  AWS region (default AWS_REGION or us-west-1)\n\
       -H, --host HOST      DynamoDB host (implies local-style -p)\n\
       -p PORT              DynamoDB port (default 8000, with -H or --local)\n\
       -h, --help           Show this help\n\n\
     Extra arguments after -- are passed to jupyter lab.\n\n\
     JupyterLab is downloaded into ~/.local/share/dqlrs/notebook on first run\n\
     (needs Python 3.10+ or uv). This binary is not Jupyter; it only ships the\n\
     DQL kernel and then starts Lab pointed at DQLRS_BIN."
        .to_string()
}

pub fn parse_args(argv: &[String]) -> Result<NotebookArgs, String> {
    let mut args = NotebookArgs::default();
    let mut iter = argv.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "notebook" | "--notebook" => {}
            "-h" | "--help" => args.help = true,
            "--local" => args.local = true,
            "--no-browser" => args.no_browser = true,
            "--dry-run" => args.dry_run = true,
            "--serve" | "-c" | "--command" => args.conflict_with_serve_or_command = true,
            "--jupyter-port" | "--port" => {
                let value = iter.next().ok_or_else(|| format!("{arg} needs a value"))?;
                args.jupyter_port = Some(parse_port(value, arg)?);
            }
            "-r" | "--region" => {
                args.region = Some(
                    iter.next()
                        .ok_or_else(|| format!("{arg} needs a value"))?
                        .clone(),
                );
            }
            "-H" | "--host" => {
                args.host = Some(
                    iter.next()
                        .ok_or_else(|| format!("{arg} needs a value"))?
                        .clone(),
                );
            }
            "-p" => {
                let value = iter.next().ok_or_else(|| "-p needs a value".to_string())?;
                args.dynamo_port = Some(parse_port(value, "-p")?);
            }
            "--" => {
                args.jupyter_extra.extend(iter.cloned());
                break;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown argument '{other}'\n\n{}", help_text()));
            }
            other => {
                return Err(format!(
                    "unexpected argument {other:?} (use -- to pass flags to jupyter lab)\n\n{}",
                    help_text()
                ));
            }
        }
    }
    Ok(args)
}

fn parse_port(value: &str, flag: &str) -> Result<u16, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} expected a port number, got {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("dqlrs")
            .chain(args.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn detects_subcommand_and_flag() {
        assert!(is_notebook_invocation(&argv(&["notebook"])));
        assert!(is_notebook_invocation(&argv(&["--notebook", "--local"])));
        assert!(is_notebook_invocation(&argv(&[
            "-r",
            "us-east-1",
            "--notebook"
        ])));
        assert!(!is_notebook_invocation(&argv(&["-c", "notebook"])));
        assert!(!is_notebook_invocation(&argv(&["--serve"])));
        assert!(!is_notebook_invocation(&argv(&["--command", "notebook"])));
    }

    #[test]
    fn parses_local_and_ports() {
        let parsed = parse_args(&argv(&[
            "notebook",
            "--local",
            "--no-browser",
            "--jupyter-port",
            "8889",
            "-r",
            "us-east-1",
        ]))
        .unwrap();
        assert!(parsed.local);
        assert!(parsed.no_browser);
        assert_eq!(parsed.jupyter_port, Some(8889));
        assert_eq!(parsed.region.as_deref(), Some("us-east-1"));
    }

    #[test]
    fn extra_after_double_dash() {
        let parsed = parse_args(&argv(&["--notebook", "--", "--debug"])).unwrap();
        assert_eq!(parsed.jupyter_extra, vec!["--debug"]);
    }

    #[test]
    fn rejects_serve_combo() {
        let parsed = parse_args(&argv(&["--notebook", "--serve"])).unwrap();
        assert!(parsed.conflict_with_serve_or_command);
    }
}
