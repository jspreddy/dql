//! Create a user-local venv and install JupyterLab + ipykernel.

use color_eyre::eyre::{bail, eyre, WrapErr};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn python_bin(data_dir: &Path) -> PathBuf {
    data_dir.join("venv/bin/python")
}

pub fn jupyter_bin(data_dir: &Path) -> PathBuf {
    data_dir.join("venv/bin/jupyter")
}

pub fn ensure_jupyter(data_dir: &Path) -> color_eyre::Result<PathBuf> {
    let python = python_bin(data_dir);
    if python.is_file() && jupyter_import_ok(&python) {
        return Ok(python);
    }
    eprintln!(
        "Preparing JupyterLab in {} (first run downloads packages)...",
        data_dir.display()
    );
    fs_create_venv(data_dir)?;
    let python = python_bin(data_dir);
    install_packages(&python)?;
    if !jupyter_import_ok(&python) {
        bail!(
            "JupyterLab installed but could not be imported from {}",
            python.display()
        );
    }
    Ok(python)
}

fn jupyter_import_ok(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import jupyterlab, ipykernel, jupyter_client"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn fs_create_venv(data_dir: &Path) -> color_eyre::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let venv = data_dir.join("venv");
    if let Some(uv) = find_on_path("uv") {
        let mut cmd = Command::new(uv);
        cmd.args([
            "venv",
            "--no-project",
            "--seed",
            "--python",
            "3.11",
            "--allow-existing",
        ]);
        cmd.arg(&venv);
        cmd.env("UV_NO_PROJECT", "1");
        let status = cmd.status().wrap_err("uv venv")?;
        if status.success() {
            return Ok(());
        }
        eprintln!("uv venv failed ({status}); trying python3 -m venv");
    }
    let python = find_python().ok_or_else(|| {
        eyre!(
            "dqlrs notebook needs Python 3.10+ or uv.\n\
             Install uv: https://docs.astral.sh/uv/\n\
             or install Python 3.11 and retry."
        )
    })?;
    let status = Command::new(&python)
        .args(["-m", "venv"])
        .arg(&venv)
        .status()
        .wrap_err_with(|| format!("{} -m venv", python.display()))?;
    if !status.success() {
        bail!(
            "{python} -m venv failed ({status}). On Debian/Ubuntu install python3-venv, or install uv.",
            python = python.display()
        );
    }
    Ok(())
}

fn install_packages(python: &Path) -> color_eyre::Result<()> {
    if let Some(uv) = find_on_path("uv") {
        let status = Command::new(uv)
            .args(["pip", "install", "--python"])
            .arg(python)
            .args(["jupyterlab>=4.2,<5", "ipykernel>=6.29"])
            .env("UV_NO_PROJECT", "1")
            .current_dir(
                python
                    .parent()
                    .and_then(|p| p.parent())
                    .unwrap_or(Path::new("/")),
            )
            .status()
            .wrap_err("uv pip install jupyterlab")?;
        if status.success() {
            return Ok(());
        }
        eprintln!("uv pip install failed ({status}); trying python -m pip");
    }
    let status = Command::new(python)
        .args([
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "jupyterlab>=4.2,<5",
            "ipykernel>=6.29",
        ])
        .status()
        .wrap_err("python -m pip install jupyterlab")?;
    if !status.success() {
        bail!(
            "could not install JupyterLab with {} -m pip (exit {status}). Check network access.",
            python.display()
        );
    }
    Ok(())
}

fn find_python() -> Option<PathBuf> {
    for name in [
        "python3.11",
        "python3.12",
        "python3.10",
        "python3",
        "python",
    ] {
        if let Some(path) = find_on_path(name) {
            if python_supported(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn python_supported(python: &Path) -> bool {
    Command::new(python)
        .args([
            "-c",
            "import sys; raise SystemExit(0 if sys.version_info[:2] >= (3, 10) else 1)",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
