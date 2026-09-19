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
use std::path::PathBuf;
use std::process::Command;

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
}
