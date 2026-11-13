//! `dqlrs web --init-examples`, run as the real binary.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn dql_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_dqlrs") {
        return PathBuf::from(path);
    }
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join("debug").join("dqlrs")
}

fn dqlrs(cwd: &Path) -> Command {
    let mut command = Command::new(dql_bin());
    command
        .current_dir(cwd)
        .env("DQL_BACKEND", "memory")
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0")
        // An identity for the commit, so the test does not depend on the machine's git config.
        .env("GIT_AUTHOR_NAME", "dqlrs test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "dqlrs test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com");
    command
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    dqlrs(cwd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run dqlrs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn init_examples_needs_the_web_subcommand_and_yes_needs_init_examples() {
    let cwd = tempfile::tempdir().unwrap();
    let output = run(cwd.path(), &["--init-examples"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--init-examples requires `dqlrs web`"),
        "{}",
        stderr(&output)
    );

    let output = run(cwd.path(), &["web", "--yes"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--yes requires `dqlrs web --init-examples`"),
        "{}",
        stderr(&output)
    );

    // The old two-flag spelling is not a flag.
    let output = run(cwd.path(), &["web", "--init"]);
    assert!(
        stderr(&output).contains("unknown argument '--init'"),
        "{}",
        stderr(&output)
    );
    assert!(!cwd.path().join(".git").exists());
}

#[test]
fn init_refuses_a_folder_that_is_not_empty() {
    let cwd = tempfile::tempdir().unwrap();
    std::fs::write(cwd.path().join("mine.dql"), "SCAN * FROM t;\n").unwrap();
    let output = run(cwd.path(), &["web", "--init-examples", "--yes"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("needs an empty folder"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("mine.dql"), "{}", stderr(&output));
    assert!(!cwd.path().join(".git").exists(), "nothing was initialized");
}

#[test]
fn init_without_a_terminal_asks_for_yes_and_changes_nothing() {
    let cwd = tempfile::tempdir().unwrap();
    let output = run(cwd.path(), &["web", "--init-examples", "--dir", "new"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("pass --yes"),
        "{}",
        stderr(&output)
    );
    assert!(
        !cwd.path().join("new").exists(),
        "the folder is created only after the confirm"
    );
}

#[test]
fn init_with_yes_copies_and_commits_the_examples_then_serves_them() {
    let cwd = tempfile::tempdir().unwrap();
    let port = free_port();
    let listen = format!("127.0.0.1:{port}");
    let mut child = dqlrs(cwd.path())
        .args([
            "web",
            "--init-examples",
            "--yes",
            "--dir",
            "workspace",
            "--listen",
            &listen,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn dqlrs web --init-examples");

    // Read stdout until the server says it is listening.
    let mut lines = Vec::new();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        lines.push(line.trim_end().to_string());
        if line.starts_with("DQLRS Web") {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let out = lines.join("\n");
    assert!(out.contains("1. Create a git repository"), "{out}");
    assert!(
        out.contains("3. Commit them as \"Initialized dqlrs web examples\""),
        "{out}"
    );
    assert!(
        out.contains(&format!("DQLRS Web  http://{listen}")),
        "{out}"
    );

    let workspace = cwd.path().join("workspace");
    assert_eq!(
        git(&workspace, &["log", "--format=%s"]).trim(),
        "Initialized dqlrs web examples"
    );
    assert!(workspace.join("AGENTS.md").is_file());
    assert!(workspace.join("queries/README.md").is_file());
    // The server's cache is ignored by the examples' .gitignore, so the tree stays clean.
    assert_eq!(git(&workspace, &["status", "--porcelain"]), "");
}
