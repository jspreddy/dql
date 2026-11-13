//! DynamoDB Local for `dqlrs web --init-examples`: find Java or Docker, ask which to use, then
//! download and start DynamoDB Local with Java, or start it with the examples'
//! `docker-compose.yml`.
//!
//! Nothing is bundled. The Java path downloads the official archive from AWS and checks it
//! against AWS's published SHA-256. The Docker path pulls `amazon/dynamodb-local`.

use std::io::{self, BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use color_eyre::eyre::{bail, eyre, WrapErr};
use sha2::{Digest, Sha256};

use super::progress::Progress;

pub(crate) const HOST: &str = "localhost";
pub(crate) const PORT: u16 = 8000;
/// DynamoDB Local 2.6 and later needs Java 17 or newer.
const MIN_JAVA: u32 = 17;
const ARCHIVE_URL: &str = "https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.tar.gz";
const ARCHIVE_NAME: &str = "dynamodb_local_latest.tar.gz";
/// Under the workspace; the examples' .gitignore lists it.
pub(crate) const INSTALL_DIR: &str = ".dynamodb-local";
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// How DynamoDB Local runs for the examples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Runtime {
    Java,
    Docker,
    Skip,
}

impl FromStr for Runtime {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "java" => Ok(Self::Java),
            "docker" => Ok(Self::Docker),
            "skip" | "none" => Ok(Self::Skip),
            other => Err(format!(
                "--dynamodb takes java, docker, or skip, not '{other}'"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum JavaState {
    Missing,
    TooOld(String),
    Ready(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DockerState {
    Missing,
    /// The CLI is installed but the daemon does not answer (Docker Desktop not started).
    NotRunning(String),
    /// The daemon runs but neither `docker compose` nor `docker-compose` works.
    NoCompose(String),
    Ready {
        version: String,
        compose: Compose,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compose {
    /// `docker compose`
    Plugin,
    /// `docker-compose`
    Standalone,
}

/// What this machine has for running DynamoDB Local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Detected {
    pub(crate) java: JavaState,
    pub(crate) docker: DockerState,
    /// DynamoDB Local (or something like it) already answers on localhost:8000.
    pub(crate) running: bool,
}

impl Detected {
    pub(crate) fn available(&self) -> Vec<Runtime> {
        let mut options = Vec::new();
        if matches!(self.java, JavaState::Ready(_)) {
            options.push(Runtime::Java);
        }
        if matches!(self.docker, DockerState::Ready { .. }) {
            options.push(Runtime::Docker);
        }
        options
    }
}

/// Look for a running DynamoDB Local, Java, and Docker, one progress line each.
pub(crate) fn detect(progress: &Progress) -> Detected {
    let running = answers_like_dynamodb(PORT);
    if running {
        progress.say_check(&format!(
            "DynamoDB Local is already running on {HOST}:{PORT}"
        ));
        return Detected {
            java: JavaState::Missing,
            docker: DockerState::Missing,
            running,
        };
    }
    let java = detect_java();
    match &java {
        JavaState::Ready(version) => progress.say_check(&format!(
            "Java {version} (DynamoDB Local needs {MIN_JAVA} or newer)"
        )),
        JavaState::TooOld(version) => progress.note(&format!(
            "Java {version} is too old for DynamoDB Local, which needs {MIN_JAVA} or newer"
        )),
        JavaState::Missing => progress.note("Java not found"),
    }
    let docker = detect_docker();
    match &docker {
        DockerState::Ready { version, compose } => progress.say_check(&format!(
            "Docker {version} with {}",
            match compose {
                Compose::Plugin => "docker compose",
                Compose::Standalone => "docker-compose",
            }
        )),
        DockerState::NotRunning(version) => progress.note(&format!(
            "Docker {version} is installed, but Docker is not running"
        )),
        DockerState::NoCompose(version) => progress.note(&format!(
            "Docker {version} is installed, but docker compose is not"
        )),
        DockerState::Missing => progress.note("Docker not found"),
    }
    Detected {
        java,
        docker,
        running,
    }
}

fn detect_java() -> JavaState {
    let output = match Command::new("java").arg("-version").output() {
        Ok(output) if output.status.success() => output,
        _ => return JavaState::Missing,
    };
    // `java -version` writes to stderr, sometimes after a "Picked up JAVA_TOOL_OPTIONS" line.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    match parse_java_version(&text) {
        Some((version, major)) if major >= MIN_JAVA => JavaState::Ready(version),
        Some((version, _)) => JavaState::TooOld(version),
        None => JavaState::Missing,
    }
}

/// `openjdk version "21.0.2"` gives ("21.0.2", 21); `java version "1.8.0_392"` gives 8.
pub(crate) fn parse_java_version(text: &str) -> Option<(String, u32)> {
    let line = text.lines().find(|line| line.contains(" version \""))?;
    let version = line.split('"').nth(1)?.to_string();
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 {
        parts.next()?.parse().ok()?
    } else {
        first
    };
    Some((version, major))
}

fn detect_docker() -> DockerState {
    let version = match Command::new("docker").arg("--version").output() {
        Ok(output) if output.status.success() => {
            parse_docker_version(&String::from_utf8_lossy(&output.stdout))
        }
        _ => return DockerState::Missing,
    };
    let daemon = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !matches!(daemon, Ok(status) if status.success()) {
        return DockerState::NotRunning(version);
    }
    let works = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    if works("docker", &["compose", "version"]) {
        DockerState::Ready {
            version,
            compose: Compose::Plugin,
        }
    } else if works("docker-compose", &["version"]) {
        DockerState::Ready {
            version,
            compose: Compose::Standalone,
        }
    } else {
        DockerState::NoCompose(version)
    }
}

/// `Docker version 27.1.1, build 6312585` gives `27.1.1`.
pub(crate) fn parse_docker_version(text: &str) -> String {
    text.split_whitespace()
        .nth(2)
        .map(|version| version.trim_end_matches(',').to_string())
        .unwrap_or_else(|| text.trim().to_string())
}

/// Something on `port` answers HTTP the way DynamoDB Local does (it returns an AWS request id).
pub(crate) fn answers_like_dynamodb(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request = format!("GET / HTTP/1.1\r\nHost: {HOST}:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = Vec::new();
    let _ = stream.take(4096).read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response).to_ascii_lowercase();
    text.contains("x-amzn-requestid") || text.contains("missingauthenticationtoken")
}

fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Pick how to run DynamoDB Local. `preset` (from `--dynamodb`) wins; otherwise ask when
/// `ask` is true, else skip. An option this machine cannot run is refused.
pub(crate) fn choose(
    detected: &Detected,
    preset: Option<Runtime>,
    ask: bool,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> color_eyre::Result<Runtime> {
    if detected.running {
        return Ok(Runtime::Skip);
    }
    let available = detected.available();
    if let Some(runtime) = preset {
        if runtime != Runtime::Skip && !available.contains(&runtime) {
            bail!(
                "--dynamodb {} is not available here: {}",
                name(runtime),
                why_unavailable(detected, runtime)
            );
        }
        return Ok(runtime);
    }
    if available.is_empty() || !ask {
        return Ok(Runtime::Skip);
    }
    let mut options = available.clone();
    options.push(Runtime::Skip);
    writeln!(
        out,
        "\nThe examples run against DynamoDB Local. How should it run?"
    )?;
    for (index, runtime) in options.iter().enumerate() {
        writeln!(out, "  {}. {}", index + 1, describe(*runtime))?;
    }
    write!(out, "Choose [1]: ")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    let answer = line.trim();
    if answer.is_empty() {
        return Ok(options[0]);
    }
    if let Ok(number) = answer.parse::<usize>() {
        if (1..=options.len()).contains(&number) {
            return Ok(options[number - 1]);
        }
    }
    if let Ok(runtime) = answer.parse::<Runtime>() {
        if options.contains(&runtime) {
            return Ok(runtime);
        }
    }
    bail!("'{answer}' is not one of the choices")
}

fn name(runtime: Runtime) -> &'static str {
    match runtime {
        Runtime::Java => "java",
        Runtime::Docker => "docker",
        Runtime::Skip => "skip",
    }
}

fn describe(runtime: Runtime) -> String {
    match runtime {
        Runtime::Java => {
            format!("Java: download DynamoDB Local from AWS into {INSTALL_DIR}/ and start it")
        }
        Runtime::Docker => "Docker: start it with docker compose (docker-compose.yml)".to_string(),
        Runtime::Skip => "Skip: set it up later".to_string(),
    }
}

fn why_unavailable(detected: &Detected, runtime: Runtime) -> String {
    match runtime {
        Runtime::Java => match &detected.java {
            JavaState::TooOld(version) => format!("Java {version} is older than {MIN_JAVA}"),
            _ => "Java was not found".to_string(),
        },
        Runtime::Docker => match &detected.docker {
            DockerState::NotRunning(_) => "Docker is not running".to_string(),
            DockerState::NoCompose(_) => "docker compose is not installed".to_string(),
            _ => "Docker was not found".to_string(),
        },
        Runtime::Skip => String::new(),
    }
}

/// The plan line for the choice, or None for skip.
pub(crate) fn plan_line(runtime: Runtime) -> Option<String> {
    match runtime {
        Runtime::Java => Some(format!(
            "Download DynamoDB Local from AWS into {INSTALL_DIR}/ and start it on {HOST}:{PORT}."
        )),
        Runtime::Docker => Some(format!(
            "Start DynamoDB Local with docker compose on {HOST}:{PORT}."
        )),
        Runtime::Skip => None,
    }
}

/// Install (Java) and start DynamoDB Local in `dir`. Returns true when it answers on
/// localhost:8000. A failure is reported with a hint and returns false; it never stops the
/// rest of `--init-examples`.
pub(crate) fn set_up(
    dir: &Path,
    runtime: Runtime,
    detected: &Detected,
    progress: &Progress,
) -> bool {
    let result = match runtime {
        Runtime::Skip => return false,
        Runtime::Java => set_up_java(dir, progress),
        Runtime::Docker => {
            let compose = match &detected.docker {
                DockerState::Ready { compose, .. } => *compose,
                _ => Compose::Plugin,
            };
            set_up_docker(dir, compose, progress)
        }
    };
    match result {
        Ok(()) => true,
        Err(err) => {
            progress.say(&format!("    {err}"));
            false
        }
    }
}

fn set_up_java(dir: &Path, progress: &Progress) -> color_eyre::Result<()> {
    let install = dir.join(INSTALL_DIR);
    std::fs::create_dir_all(install.join("data"))
        .wrap_err_with(|| format!("create {}", install.display()))?;
    let archive = install.join(ARCHIVE_NAME);
    progress.step("Downloading DynamoDB Local", |detail| {
        let total = content_length(ARCHIVE_URL);
        let mut child = Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&archive)
            .arg(ARCHIVE_URL)
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err("run curl (needed to download DynamoDB Local)")?;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            let done = std::fs::metadata(&archive)
                .map(|meta| meta.len())
                .unwrap_or(0);
            match total {
                Some(total) => {
                    detail.set_bar_only(done as usize, total as usize);
                    detail.set(format!("{} of {}", megabytes(done), megabytes(total)));
                }
                None => detail.set(megabytes(done)),
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        if !status.success() {
            let mut err = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut err);
            }
            bail!("download failed: {}", err.trim());
        }
        let size = std::fs::metadata(&archive)?.len();
        Ok((
            (),
            format!("Downloaded DynamoDB Local ({})", megabytes(size)),
        ))
    })?;
    progress.step("Checking the download", |_| {
        let expected = fetch_text(&format!("{ARCHIVE_URL}.sha256"))?;
        let expected = expected
            .split_whitespace()
            .next()
            .ok_or_else(|| eyre!("AWS's checksum file is empty"))?
            .to_ascii_lowercase();
        let actual = sha256_file(&archive)?;
        if actual != expected {
            bail!("the download does not match AWS's SHA-256 ({actual} is not {expected})");
        }
        Ok(((), "Checksum matches AWS's SHA-256".to_string()))
    })?;
    progress.step("Unpacking DynamoDB Local", |_| {
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(ARCHIVE_NAME)
            .current_dir(&install)
            .status()
            .wrap_err("run tar")?;
        if !status.success() {
            bail!("tar could not unpack {ARCHIVE_NAME}");
        }
        let _ = std::fs::remove_file(&archive);
        Ok(((), format!("Unpacked into {INSTALL_DIR}/")))
    })?;
    let pid = progress.step("Starting DynamoDB Local", |detail| {
        let log_path = install.join("dynamodb-local.log");
        let log = std::fs::File::create(&log_path)
            .wrap_err_with(|| format!("create {}", log_path.display()))?;
        let mut command = java_command(&install);
        command
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        detach(&mut command);
        let mut child = command.spawn().wrap_err("start java")?;
        let pid = child.id();
        wait_until_up(detail, || match child.try_wait() {
            Ok(Some(status)) => Err(eyre!("java exited ({status}); see {}", log_path.display())),
            _ => Ok(()),
        })?;
        Ok((
            pid,
            format!("DynamoDB Local is running on {HOST}:{PORT} (pid {pid})"),
        ))
    })?;
    progress.say(&format!(
        "    Stop it with `kill {pid}`. Start it again from {INSTALL_DIR}/ with:\n    java -Djava.library.path=./DynamoDBLocal_lib -jar DynamoDBLocal.jar -sharedDb -dbPath ./data"
    ));
    Ok(())
}

fn java_command(install: &Path) -> Command {
    let mut command = Command::new("java");
    command
        .args([
            "-Djava.library.path=./DynamoDBLocal_lib",
            "-jar",
            "DynamoDBLocal.jar",
            "-sharedDb",
            "-dbPath",
            "./data",
        ])
        .current_dir(install);
    command
}

/// Keep DynamoDB Local running after `dqlrs` exits, and out of reach of the terminal's Ctrl+C.
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

fn set_up_docker(dir: &Path, compose: Compose, progress: &Progress) -> color_eyre::Result<()> {
    progress.step("Starting DynamoDB Local with docker compose", |detail| {
        let mut command = match compose {
            Compose::Plugin => {
                let mut command = Command::new("docker");
                command.arg("compose");
                command
            }
            Compose::Standalone => Command::new("docker-compose"),
        };
        detail.set("pulling amazon/dynamodb-local");
        let output = command
            .args(["up", "-d"])
            .current_dir(dir)
            .stdin(Stdio::null())
            .output()
            .wrap_err("run docker compose")?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let last = err
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            bail!("docker compose up failed: {last}");
        }
        detail.set("");
        wait_until_up(detail, || Ok(()))?;
        Ok((
            (),
            format!("DynamoDB Local is running on {HOST}:{PORT} (docker compose)"),
        ))
    })?;
    progress
        .say("    Stop it with `docker compose down`; start it again with `docker compose up -d`.");
    Ok(())
}

/// Wait for DynamoDB Local to answer, calling `still_ok` between tries so a crash stops the
/// wait early.
fn wait_until_up(
    detail: &super::progress::Detail,
    mut still_ok: impl FnMut() -> color_eyre::Result<()>,
) -> color_eyre::Result<()> {
    let started = Instant::now();
    while started.elapsed() < START_TIMEOUT {
        if answers_like_dynamodb(PORT) {
            return Ok(());
        }
        still_ok()?;
        detail.set(format!(
            "waiting for {HOST}:{PORT} ({}s)",
            started.elapsed().as_secs()
        ));
        std::thread::sleep(Duration::from_millis(250));
    }
    if port_open(PORT) {
        bail!("something answers on {HOST}:{PORT}, but it does not look like DynamoDB Local");
    }
    bail!(
        "DynamoDB Local did not answer on {HOST}:{PORT} within {}s",
        START_TIMEOUT.as_secs()
    )
}

fn content_length(url: &str) -> Option<u64> {
    let output = Command::new("curl").args(["-fsSIL", url]).output().ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .next_back()
}

fn fetch_text(url: &str) -> color_eyre::Result<String> {
    let output = Command::new("curl")
        .args(["-fsSL", url])
        .output()
        .wrap_err("run curl")?;
    if !output.status.success() {
        bail!("could not download {url}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn sha256_file(path: &PathBuf) -> color_eyre::Result<String> {
    let mut file =
        std::fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(java: JavaState, docker: DockerState) -> Detected {
        Detected {
            java,
            docker,
            running: false,
        }
    }

    fn ready_docker() -> DockerState {
        DockerState::Ready {
            version: "27.1.1".to_string(),
            compose: Compose::Plugin,
        }
    }

    #[test]
    fn java_versions_parse_old_and_new_styles() {
        let modern =
            "Picked up JAVA_TOOL_OPTIONS: -Dx=y\nopenjdk version \"21.0.12.1\" 2026-08-18\n";
        assert_eq!(
            parse_java_version(modern),
            Some(("21.0.12.1".to_string(), 21))
        );
        let legacy = "java version \"1.8.0_392\"\nJava(TM) SE Runtime Environment";
        assert_eq!(
            parse_java_version(legacy),
            Some(("1.8.0_392".to_string(), 8))
        );
        assert_eq!(
            parse_java_version("openjdk version \"17\" 2021-09-14"),
            Some(("17".to_string(), 17))
        );
        assert_eq!(parse_java_version("command not found"), None);
    }

    #[test]
    fn docker_version_is_the_third_word() {
        assert_eq!(
            parse_docker_version("Docker version 27.1.1, build 6312585\n"),
            "27.1.1"
        );
    }

    #[test]
    fn only_runnable_options_are_offered_and_enter_picks_the_first() {
        let both = detected(JavaState::Ready("21".into()), ready_docker());
        let mut out = Vec::new();
        let choice = choose(&both, None, true, &mut "\n".as_bytes(), &mut out).unwrap();
        assert_eq!(choice, Runtime::Java);
        let shown = String::from_utf8(out).unwrap();
        assert!(
            shown.contains("1. Java: download DynamoDB Local"),
            "{shown}"
        );
        assert!(
            shown.contains("2. Docker: start it with docker compose"),
            "{shown}"
        );
        assert!(shown.contains("3. Skip: set it up later"), "{shown}");

        let docker_only = detected(JavaState::TooOld("11.0.2".into()), ready_docker());
        let mut out = Vec::new();
        assert_eq!(
            choose(&docker_only, None, true, &mut "\n".as_bytes(), &mut out).unwrap(),
            Runtime::Docker
        );
        assert!(!String::from_utf8(out).unwrap().contains("Java:"));
        assert_eq!(
            choose(
                &docker_only,
                None,
                true,
                &mut "2\n".as_bytes(),
                &mut Vec::new()
            )
            .unwrap(),
            Runtime::Skip
        );
        assert_eq!(
            choose(
                &docker_only,
                None,
                true,
                &mut "skip\n".as_bytes(),
                &mut Vec::new()
            )
            .unwrap(),
            Runtime::Skip
        );
        assert!(choose(
            &docker_only,
            None,
            true,
            &mut "9\n".as_bytes(),
            &mut Vec::new()
        )
        .is_err());
    }

    #[test]
    fn nothing_is_asked_without_options_a_terminal_or_when_already_running() {
        let none = detected(JavaState::Missing, DockerState::NotRunning("27.1.1".into()));
        assert_eq!(
            choose(&none, None, true, &mut "".as_bytes(), &mut Vec::new()).unwrap(),
            Runtime::Skip
        );
        let both = detected(JavaState::Ready("21".into()), ready_docker());
        assert_eq!(
            choose(&both, None, false, &mut "".as_bytes(), &mut Vec::new()).unwrap(),
            Runtime::Skip
        );
        let running = Detected {
            running: true,
            ..both
        };
        assert_eq!(
            choose(
                &running,
                Some(Runtime::Java),
                true,
                &mut "".as_bytes(),
                &mut Vec::new()
            )
            .unwrap(),
            Runtime::Skip
        );
    }

    #[test]
    fn a_preset_that_cannot_run_here_says_why() {
        let no_docker = detected(
            JavaState::Ready("21".into()),
            DockerState::NotRunning("27.1.1".into()),
        );
        let err = choose(
            &no_docker,
            Some(Runtime::Docker),
            false,
            &mut "".as_bytes(),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            err,
            "--dynamodb docker is not available here: Docker is not running"
        );
        assert_eq!(
            choose(
                &no_docker,
                Some(Runtime::Java),
                false,
                &mut "".as_bytes(),
                &mut Vec::new()
            )
            .unwrap(),
            Runtime::Java
        );
        assert_eq!("Docker".parse::<Runtime>(), Ok(Runtime::Docker));
        assert!("podman".parse::<Runtime>().is_err());
    }

    #[test]
    fn the_examples_compose_file_is_bundled_and_the_install_dir_is_ignored() {
        let compose = super::super::init::bundled_file("docker-compose.yml").expect("compose file");
        assert!(compose.contains("amazon/dynamodb-local"));
        assert!(compose.contains("8000:8000"));
        let ignore = super::super::init::bundled_file(".gitignore").expect(".gitignore");
        assert!(ignore.lines().any(|line| line.trim() == ".dynamodb-local/"));
    }
}
