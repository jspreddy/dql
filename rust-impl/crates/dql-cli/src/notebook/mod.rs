//! `dqlrs notebook` / `dqlrs --notebook`: JupyterLab + DQL (Rust) kernel.
//!
//! Jupyter itself is not inside the binary (wheels are huge and platform-
//! specific). This command extracts the small Python kernel shipped in the
//! binary, puts Jupyter in a user-local venv, and points `DQLRS_BIN` at this
//! `dqlrs`.

mod args;
mod layout;
mod venv;

pub use args::{help_text, is_notebook_invocation, parse_args, NotebookArgs};

use color_eyre::eyre::{bail, WrapErr};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run(argv: &[String]) -> color_eyre::Result<()> {
    let args = parse_args(argv).map_err(|err| color_eyre::eyre::eyre!("{err}"))?;
    if args.help {
        let mut stderr = io::stderr();
        stderr.write_all(help_text().as_bytes())?;
        stderr.write_all(b"\n")?;
        return Ok(());
    }
    if args.conflict_with_serve_or_command {
        bail!("notebook cannot be used with --serve or -c/--command");
    }
    if args.shutdown {
        return shutdown_notebook_servers(&layout::data_dir());
    }

    let dqlrs = std::env::current_exe().wrap_err("could not resolve this dqlrs binary")?;
    let dqlrs = dqlrs
        .canonicalize()
        .unwrap_or(dqlrs)
        .into_os_string()
        .into_string()
        .unwrap_or_else(|raw| PathBuf::from(raw).display().to_string());

    let data_dir = layout::data_dir();
    let notebook_dir = std::env::current_dir().wrap_err("current directory")?;
    layout::materialize(&data_dir, &dqlrs, &notebook_dir, args.no_browser)
        .wrap_err("failed to write notebook kernel files")?;

    if args.local {
        apply_local_env(&args);
    }
    if let Some(region) = args.region.as_deref() {
        std::env::set_var("AWS_REGION", region);
    }
    if let Some(host) = args.host.as_deref() {
        std::env::set_var("DQL_HOST", host);
        let port = args.dynamo_port.unwrap_or(8000);
        std::env::set_var("DQL_PORT", port.to_string());
    }
    std::env::set_var("DQLRS_BIN", &dqlrs);
    std::env::set_var("DQL_NOTEBOOK_JSON", "1");
    std::env::set_var("DQL_NOTEBOOK_SERVE", "1");

    let python = venv::ensure_jupyter(&data_dir).wrap_err("failed to prepare JupyterLab")?;
    layout::refresh_kernelspec(&data_dir, &dqlrs)
        .wrap_err("failed to refresh the DQL (Rust) kernelspec")?;
    layout::install_kernel_into_site_packages(&python, &data_dir)
        .wrap_err("failed to install dql_notebook_kernel into the venv")?;

    print_banner(&dqlrs, &data_dir, &notebook_dir, &args);

    if args.dry_run {
        let jupyter = venv::jupyter_bin(&data_dir);
        let output = Command::new(&jupyter)
            .args(["kernelspec", "list"])
            .env("JUPYTER_PATH", layout::jupyter_path(&data_dir))
            .env("JUPYTER_DATA_DIR", data_dir.join("jupyter-data"))
            .output()
            .wrap_err("jupyter kernelspec list")?;
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
        println!("Dry run complete.");
        return Ok(());
    }

    if args.local {
        warn_if_local_down(&args);
    }

    let jupyter = venv::jupyter_bin(&data_dir);
    let mut cmd = Command::new(&jupyter);
    cmd.arg("lab");
    cmd.arg("-y");
    cmd.arg("--config");
    cmd.arg(layout::config_path(&data_dir));
    cmd.arg("--notebook-dir");
    cmd.arg(&notebook_dir);
    cmd.arg("--ip");
    cmd.arg("127.0.0.1");
    if args.no_browser {
        cmd.arg("--no-browser");
    }
    if let Some(port) = args.jupyter_port {
        cmd.arg("--port");
        cmd.arg(port.to_string());
    }
    for extra in &args.jupyter_extra {
        cmd.arg(extra);
    }
    cmd.env("DQLRS_BIN", &dqlrs);
    cmd.env("JUPYTER_PATH", layout::jupyter_path(&data_dir));
    cmd.env("JUPYTER_DATA_DIR", data_dir.join("jupyter-data"));
    cmd.env("JUPYTER_RUNTIME_DIR", data_dir.join("jupyter-runtime"));
    cmd.env("JUPYTER_CONFIG_DIR", data_dir.join("jupyter-config"));
    let status = cmd.status().wrap_err("failed to start JupyterLab")?;
    if !status.success() {
        bail!("JupyterLab exited with {status}");
    }
    Ok(())
}

fn apply_local_env(args: &NotebookArgs) {
    if std::env::var_os("DQL_HOST").is_none() && args.host.is_none() {
        std::env::set_var("DQL_HOST", "localhost");
    }
    if std::env::var_os("DQL_PORT").is_none() && args.dynamo_port.is_none() {
        std::env::set_var("DQL_PORT", "8000");
    }
    if std::env::var_os("AWS_ACCESS_KEY_ID").is_none() {
        std::env::set_var("AWS_ACCESS_KEY_ID", "fakeid");
    }
    if std::env::var_os("AWS_SECRET_ACCESS_KEY").is_none() {
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "fakekey");
    }
}

fn warn_if_local_down(args: &NotebookArgs) {
    let host = args
        .host
        .clone()
        .or_else(|| std::env::var("DQL_HOST").ok())
        .unwrap_or_else(|| "localhost".into());
    let port = args.dynamo_port.unwrap_or_else(|| {
        std::env::var("DQL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8000)
    });
    if std::net::TcpStream::connect((host.as_str(), port)).is_err() {
        eprintln!(
            "warning: nothing is listening on {host}:{port}. Start DynamoDB Local or omit --local to use live AWS."
        );
    }
}

fn print_banner(
    dqlrs: &str,
    data_dir: &std::path::Path,
    notebook_dir: &std::path::Path,
    args: &NotebookArgs,
) {
    println!("dqlrs:     {dqlrs}");
    println!("kernel:    DQL (Rust)  (files in {})", data_dir.display());
    println!("notebooks: {}", notebook_dir.display());
    if let Some(host) = std::env::var_os("DQL_HOST") {
        let port = std::env::var("DQL_PORT").unwrap_or_else(|_| "8000".into());
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".into());
        println!(
            "endpoint:  {}:{}  region={region}",
            host.to_string_lossy(),
            port
        );
    } else {
        let region = args
            .region
            .clone()
            .or_else(|| std::env::var("AWS_REGION").ok())
            .unwrap_or_else(|| "us-west-1".into());
        println!("endpoint:  live AWS  region={region}");
    }
    let example = data_dir.join("examples/getting-started-rust.ipynb");
    if example.is_file() {
        println!("example:   {}", example.display());
    }
    let starter = notebook_dir.join("getting-started-dqlrs.ipynb");
    if starter.is_file() {
        println!("starter:   {}", starter.display());
    }
    println!("stop:      Ctrl-C, or `dqlrs notebook --shutdown` from another terminal");
}

#[derive(Clone)]
struct RunningServer {
    pid: u32,
    port: u16,
    root_dir: Option<String>,
}

fn running_notebook_servers(runtime_dir: &Path) -> io::Result<Vec<RunningServer>> {
    let mut servers = Vec::new();
    let entries = match std::fs::read_dir(runtime_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(servers),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with("jpserver-") && name.ends_with(".json")) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path())?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(pid) = value.get("pid").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(port) = value.get("port").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let pid = u32::try_from(pid).unwrap_or(0);
        let Ok(port) = u16::try_from(port) else {
            continue;
        };
        if pid == 0 || !pid_is_running(pid) {
            continue;
        }
        let root_dir = value
            .get("root_dir")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        servers.push(RunningServer {
            pid,
            port,
            root_dir,
        });
    }
    servers.sort_by_key(|server| server.port);
    Ok(servers)
}

fn pid_is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn parse_server_selection(input: &str, count: usize) -> Result<Vec<usize>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input.eq_ignore_ascii_case("a") || input.eq_ignore_ascii_case("all") {
        return Ok((0..count).collect());
    }
    let mut chosen = Vec::new();
    for part in input
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
    {
        let number: usize = part
            .parse()
            .map_err(|_| format!("'{part}' is not a number"))?;
        if number == 0 || number > count {
            return Err(format!("choose a number from 1 to {count}"));
        }
        let index = number - 1;
        if !chosen.contains(&index) {
            chosen.push(index);
        }
    }
    Ok(chosen)
}

fn choose_servers(servers: Vec<RunningServer>) -> color_eyre::Result<Vec<RunningServer>> {
    if servers.len() <= 1 {
        return Ok(servers);
    }
    if !io::IsTerminal::is_terminal(&io::stdin()) {
        bail!(
            "several notebook servers are running; run `dqlrs notebook --shutdown` in a terminal to choose which ones to stop"
        );
    }
    println!("Notebook servers:");
    for (index, server) in servers.iter().enumerate() {
        match &server.root_dir {
            Some(root) => println!(
                "  {}) port {}  pid {}  {root}",
                index + 1,
                server.port,
                server.pid
            ),
            None => println!("  {}) port {}  pid {}", index + 1, server.port, server.pid),
        }
    }
    print!("Shut down which servers? Numbers (1,2), 'a' for all, or Enter to cancel: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let indexes = parse_server_selection(&line, servers.len())
        .map_err(|err| color_eyre::eyre::eyre!("{err}"))?;
    Ok(indexes
        .into_iter()
        .filter_map(|index| servers.get(index).cloned())
        .collect())
}

fn shutdown_notebook_servers(data_dir: &Path) -> color_eyre::Result<()> {
    let runtime = data_dir.join("jupyter-runtime");
    let servers = running_notebook_servers(&runtime)?;
    if servers.is_empty() {
        println!("No notebook servers are running.");
        return Ok(());
    }
    let servers = choose_servers(servers)?;
    if servers.is_empty() {
        println!("Nothing selected.");
        return Ok(());
    }
    let jupyter = venv::jupyter_bin(data_dir);
    let mut failed = false;
    for server in servers {
        println!(
            "Shutting down notebook server on port {} (pid {})",
            server.port, server.pid
        );
        let stopped = if jupyter.is_file() {
            Command::new(&jupyter)
                .args(["server", "stop", &server.port.to_string()])
                .env("JUPYTER_RUNTIME_DIR", &runtime)
                .status()
                .wrap_err("failed to run jupyter server stop")?
                .success()
        } else {
            false
        };
        if stopped {
            continue;
        }
        let signaled = Command::new("kill")
            .args(["-TERM", &server.pid.to_string()])
            .status()
            .wrap_err("failed to signal notebook server")?
            .success();
        if !signaled {
            eprintln!(
                "could not stop notebook server on port {} (pid {})",
                server.port, server.pid
            );
            failed = true;
        }
    }
    if failed {
        bail!("one or more notebook servers did not stop");
    }
    Ok(())
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    #[test]
    fn lists_live_server_and_skips_dead_pid_and_kernel_json() {
        let dir = tempfile::tempdir().unwrap();
        let live = std::process::id();
        std::fs::write(
            dir.path().join(format!("jpserver-{live}.json")),
            format!(r#"{{"pid":{live},"port":8888,"token":"secret"}}"#),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("jpserver-999999.json"),
            r#"{"pid":999999,"port":8889}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("kernel-abc.json"), r#"{"pid":1,"port":1}"#).unwrap();
        let servers = running_notebook_servers(dir.path()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].pid, live);
        assert_eq!(servers[0].port, 8888);
    }

    #[test]
    fn selection_accepts_numbers_all_and_blank() {
        assert_eq!(parse_server_selection("", 2).unwrap(), Vec::<usize>::new());
        assert_eq!(
            parse_server_selection("  ", 2).unwrap(),
            Vec::<usize>::new()
        );
        assert_eq!(parse_server_selection("a", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(parse_server_selection("ALL", 2).unwrap(), vec![0, 1]);
        assert_eq!(parse_server_selection("2, 1", 2).unwrap(), vec![1, 0]);
        assert_eq!(parse_server_selection("1 1", 2).unwrap(), vec![0]);
        assert!(parse_server_selection("3", 2).is_err());
        assert!(parse_server_selection("x", 2).is_err());
    }
}
