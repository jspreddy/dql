//! Extract the embedded kernel, kernelspec, Jupyter config, and example notebook.

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

include!(concat!(env!("OUT_DIR"), "/embedded_notebook.rs"));

pub fn data_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os("DQLRS_NOTEBOOK_HOME") {
        return PathBuf::from(override_dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/dqlrs/notebook")
}

pub fn jupyter_path(data_dir: &Path) -> PathBuf {
    data_dir.join("share/jupyter")
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("jupyter_server_config.py")
}

pub fn materialize(
    data_dir: &Path,
    dqlrs_bin: &str,
    notebook_dir: &Path,
    no_browser: bool,
) -> color_eyre::Result<()> {
    let kernel_pkg = data_dir.join("src/dql_notebook_kernel");
    fs::create_dir_all(&kernel_pkg)?;
    for (name, contents) in KERNEL_FILES {
        fs::write(kernel_pkg.join(name), contents)?;
    }
    fs::create_dir_all(data_dir.join("examples"))?;
    if !EXAMPLE_NOTEBOOK.is_empty() {
        fs::write(
            data_dir.join("examples/getting-started-rust.ipynb"),
            EXAMPLE_NOTEBOOK,
        )?;
    }
    fs::create_dir_all(data_dir.join("jupyter-data"))?;
    fs::create_dir_all(data_dir.join("jupyter-runtime"))?;
    fs::create_dir_all(data_dir.join("jupyter-config"))?;
    refresh_kernelspec(data_dir, dqlrs_bin)?;
    write_jupyter_config(data_dir, notebook_dir, no_browser)?;
    write_starter_if_missing(notebook_dir)?;
    Ok(())
}

pub fn write_starter_if_missing(notebook_dir: &Path) -> color_eyre::Result<()> {
    if EXAMPLE_NOTEBOOK.is_empty() {
        return Ok(());
    }
    let dest = notebook_dir.join("getting-started-dqlrs.ipynb");
    if dest.exists() {
        return Ok(());
    }
    fs::write(&dest, EXAMPLE_NOTEBOOK)?;
    Ok(())
}

pub fn refresh_kernelspec(data_dir: &Path, dqlrs_bin: &str) -> color_eyre::Result<()> {
    write_kernelspec(data_dir, dqlrs_bin)
}

fn write_kernelspec(data_dir: &Path, dqlrs_bin: &str) -> color_eyre::Result<()> {
    let dest = jupyter_path(data_dir).join("kernels/dql-rust");
    fs::create_dir_all(&dest)?;
    let python = super::venv::python_bin(data_dir);
    let python = if python.exists() {
        python.display().to_string()
    } else {
        "python3".to_string()
    };
    let spec = json!({
        "argv": [
            python,
            "-m",
            "dql_notebook_kernel",
            "-f",
            "{connection_file}",
            "--backend",
            "rust"
        ],
        "display_name": "DQL (Rust)",
        "language": "dql",
        "interrupt_mode": "message",
        "metadata": { "debugger": false },
        "env": {
            "DQLRS_BIN": dqlrs_bin,
            "DQL_NOTEBOOK_SERVE": "1",
            "DQL_NOTEBOOK_JSON": "1"
        }
    });
    fs::write(
        dest.join("kernel.json"),
        serde_json::to_string_pretty(&spec)?,
    )?;
    Ok(())
}

fn write_jupyter_config(
    data_dir: &Path,
    notebook_dir: &Path,
    no_browser: bool,
) -> color_eyre::Result<()> {
    let open_browser = if no_browser { "False" } else { "True" };
    let root = notebook_dir
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    let body = format!(
        "c = get_config()  # noqa: F821\n\
         c.ServerApp.ip = '127.0.0.1'\n\
         c.ServerApp.allow_remote_access = False\n\
         c.ServerApp.open_browser = {open_browser}\n\
         c.ServerApp.root_dir = '{root}'\n\
         c.KernelSpecManager.allowed_kernelspecs = {{'dql-rust'}}\n"
    );
    fs::write(config_path(data_dir), body)?;
    Ok(())
}

/// Copy the extracted package onto the venv's `sys.path`.
pub fn install_kernel_into_site_packages(python: &Path, data_dir: &Path) -> color_eyre::Result<()> {
    let src = data_dir.join("src/dql_notebook_kernel");
    let src = src.display().to_string();
    let code = format!(
        "import pathlib, shutil, sysconfig\n\
         dest = pathlib.Path(sysconfig.get_path('purelib')) / 'dql_notebook_kernel'\n\
         src = pathlib.Path({src:?})\n\
         shutil.rmtree(dest, ignore_errors=True)\n\
         shutil.copytree(src, dest)\n\
         print(dest)\n"
    );
    let output = Command::new(python)
        .args(["-c", &code])
        .output()
        .map_err(|err| color_eyre::eyre::eyre!("python copy kernel: {err}"))?;
    if !output.status.success() {
        color_eyre::eyre::bail!(
            "could not install dql_notebook_kernel:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn materialize_writes_kernel_and_spec() {
        let tmp = TempDir::new().unwrap();
        let data = tmp.path().join("data");
        let notebooks = tmp.path().join("nb");
        fs::create_dir_all(&notebooks).unwrap();
        materialize(&data, "/opt/dqlrs", &notebooks, true).unwrap();
        assert!(data.join("src/dql_notebook_kernel/kernel.py").is_file());
        assert!(data
            .join("src/dql_notebook_kernel/serve_client.py")
            .is_file());
        let spec =
            fs::read_to_string(data.join("share/jupyter/kernels/dql-rust/kernel.json")).unwrap();
        assert!(spec.contains("/opt/dqlrs"));
        assert!(spec.contains("dql-rust") || spec.contains("DQL (Rust)"));
        assert!(spec.contains("DQL_NOTEBOOK_SERVE"));
        let config = fs::read_to_string(data.join("jupyter_server_config.py")).unwrap();
        assert!(config.contains("127.0.0.1"));
        assert!(config.contains("dql-rust"));
        assert!(!config.contains("preferred_dir"));
    }
}
