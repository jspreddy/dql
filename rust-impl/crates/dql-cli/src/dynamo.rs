//! `dqlrs dynamo`: list, start, stop, clean, and read the logs of DynamoDB Local servers.
//!
//! One Java download is shared by every workspace (`~/.local/share/dqlrs/dynamodb-local/install`,
//! or `$DQLRS_DYNAMO_HOME/install`). Each server keeps its data per workspace
//! (`<dir>/.dynamodb-local/data`) unless `--data` says otherwise. Docker servers run with
//! docker compose: the examples' `docker-compose.yml` when the workspace has it, otherwise a
//! compose file this module writes under the shared folder.
//!
//! `start` writes a record per port (`instances/<port>.json`). `list` merges those records with
//! a scan of running `java … DynamoDBLocal.jar` processes and `amazon/dynamodb-local`
//! containers, so servers started some other way show up as `other`. Those are only touched
//! when named, and their data is never deleted.

use std::collections::BTreeMap;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use color_eyre::eyre::{bail, eyre, WrapErr};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::style::Style;
use crate::web::dynamodb_local::{self as local, Compose, DockerState, JavaState};
use crate::web::progress::Progress;

/// The port DynamoDB Local uses unless told otherwise.
pub(crate) const DEFAULT_PORT: u16 = local::PORT;
const IMAGE: &str = "amazon/dynamodb-local";
/// The examples' compose file names its container this.
const EXAMPLES_CONTAINER: &str = "dqlrs-dynamodb-local";

/// The shared folder: installs, records, logs, and generated compose files.
pub(crate) fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("DQLRS_DYNAMO_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/dqlrs/dynamodb-local")
}

fn install_dir() -> PathBuf {
    home().join("install")
}

fn instances_dir() -> PathBuf {
    home().join("instances")
}

fn record_path(port: u16) -> PathBuf {
    instances_dir().join(format!("{port}.json"))
}

fn log_path(port: u16) -> PathBuf {
    instances_dir().join(format!("{port}.log"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Java,
    Docker,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Java => "java",
            Kind::Docker => "docker",
        }
    }
}

/// What `start` remembers about a server it started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) kind: Kind,
    pub(crate) port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pid: Option<u32>,
    /// Java: the folder passed as `-dbPath`, removed by `clean`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) data_dir: Option<PathBuf>,
    /// Docker: the compose file it was started with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) compose_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) container: Option<String>,
    /// Where the data is, for people: a folder or a Docker volume.
    pub(crate) data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) workspace: Option<PathBuf>,
    /// Seconds since the Unix epoch.
    pub(crate) started_at: u64,
}

fn read_records() -> Vec<Record> {
    let Ok(entries) = std::fs::read_dir(instances_dir()) else {
        return Vec::new();
    };
    let mut records: Vec<Record> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    records.sort_by_key(|record| record.port);
    records
}

fn write_record(record: &Record) -> color_eyre::Result<()> {
    std::fs::create_dir_all(instances_dir())
        .wrap_err_with(|| format!("create {}", instances_dir().display()))?;
    let path = record_path(record.port);
    std::fs::write(&path, serde_json::to_string_pretty(record)? + "\n")
        .wrap_err_with(|| format!("write {}", path.display()))
}

fn remove_record(port: u16) {
    let _ = std::fs::remove_file(record_path(port));
    let _ = std::fs::remove_file(log_path(port));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Running,
    Stopped,
}

/// Who started a server: dqlrs (it has a record) or something else (found by scanning).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Owner {
    Dqlrs,
    Other,
}

/// One DynamoDB Local server, as `list` shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Server {
    pub(crate) port: u16,
    pub(crate) kind: Kind,
    pub(crate) status: Status,
    pub(crate) owner: Owner,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) container: Option<String>,
    pub(crate) data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<u64>,
    #[serde(skip)]
    pub(crate) record: Option<Record>,
}

/// A `java … DynamoDBLocal.jar` process found by scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
struct JavaProcess {
    pid: u32,
    port: u16,
    data: String,
}

/// An `amazon/dynamodb-local` container found by scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Container {
    id: String,
    name: String,
    running: bool,
    port: Option<u16>,
}

/// `ps -ax -o pid=,command=` lines for DynamoDB Local servers. Only the server itself counts
/// (`-jar DynamoDBLocal.jar` or `-jar …/DynamoDBLocal.jar`), not a shell or editor whose command
/// line mentions the jar.
fn parse_java_processes(listing: &str) -> Vec<JavaProcess> {
    let mut found = Vec::new();
    for line in listing.lines() {
        let line = line.trim();
        let Some((pid, command)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        let words: Vec<&str> = command.split_whitespace().collect();
        let is_server = words.windows(2).any(|pair| {
            pair[0] == "-jar"
                && (pair[1] == "DynamoDBLocal.jar" || pair[1].ends_with("/DynamoDBLocal.jar"))
        });
        if !is_server {
            continue;
        }
        let value = |flag: &str| {
            words
                .windows(2)
                .find(|pair| pair[0] == flag)
                .map(|pair| pair[1].to_string())
        };
        let port = value("-port")
            .and_then(|port| port.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let data = if words.contains(&"-inMemory") {
            "in memory".to_string()
        } else {
            value("-dbPath").unwrap_or_else(|| "?".to_string())
        };
        found.push(JavaProcess { pid, port, data });
    }
    found
}

fn scan_java() -> Vec<JavaProcess> {
    if !cfg!(unix) {
        return Vec::new();
    }
    match Command::new("ps")
        .args(["-ax", "-o", "pid=,command="])
        .output()
    {
        Ok(output) if output.status.success() => {
            parse_java_processes(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// `docker ps -a --format '{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Ports}}'` lines for
/// DynamoDB Local containers.
fn parse_containers(listing: &str) -> Vec<Container> {
    listing
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let id = parts.next()?.trim().to_string();
            let name = parts.next()?.trim().to_string();
            let image = parts.next()?.trim();
            let state = parts.next()?.trim();
            let ports = parts.next().unwrap_or("").trim();
            if id.is_empty() || !(image == IMAGE || image.starts_with(&format!("{IMAGE}:"))) {
                return None;
            }
            Some(Container {
                id,
                name,
                running: state == "running",
                port: host_port(ports),
            })
        })
        .collect()
}

/// The host port mapped to 8000 in `0.0.0.0:8001->8000/tcp, :::8001->8000/tcp`.
fn host_port(ports: &str) -> Option<u16> {
    ports.split(',').find_map(|mapping| {
        let (host, inside) = mapping.trim().split_once("->")?;
        if !inside.starts_with("8000") {
            return None;
        }
        host.rsplit(':').next()?.parse().ok()
    })
}

fn scan_containers() -> Vec<Container> {
    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Ports}}",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            parse_containers(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

fn pid_alive(pid: u32) -> bool {
    if cfg!(unix) {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    } else {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
    }
}

/// Every DynamoDB Local server dqlrs knows about or can see, one per port, by port.
pub(crate) fn servers() -> Vec<Server> {
    // A Java server counts as running while its process lives and its port is open (a process
    // that is shutting down has already closed the port).
    merge(read_records(), scan_java(), scan_containers(), &|record| {
        record.kind == Kind::Java
            && record.pid.is_some_and(pid_alive)
            && local::port_open(record.port)
    })
}

/// Records first, then servers found by scanning on ports no record has. `java_alive` says
/// whether a Java record's process still runs (scanning may miss it on Windows).
fn merge(
    records: Vec<Record>,
    java: Vec<JavaProcess>,
    containers: Vec<Container>,
    java_alive: &dyn Fn(&Record) -> bool,
) -> Vec<Server> {
    let mut by_port: BTreeMap<u16, Server> = BTreeMap::new();
    for record in records {
        let (running, container) = match record.kind {
            Kind::Java => {
                let running = java_alive(&record);
                (running, None)
            }
            Kind::Docker => {
                let found = containers.iter().find(|container| {
                    record.container.as_deref() == Some(container.id.as_str())
                        || record.container.as_deref() == Some(container.name.as_str())
                        || (record.container.is_none() && container.port == Some(record.port))
                });
                (
                    found.is_some_and(|container| container.running),
                    found.map(|container| container.name.clone()),
                )
            }
        };
        by_port.insert(
            record.port,
            Server {
                port: record.port,
                kind: record.kind,
                status: if running {
                    Status::Running
                } else {
                    Status::Stopped
                },
                owner: Owner::Dqlrs,
                pid: record.pid.filter(|_| record.kind == Kind::Java),
                container: container.or_else(|| record.container.clone()),
                data: record.data.clone(),
                workspace: record.workspace.clone(),
                started_at: Some(record.started_at),
                record: Some(record),
            },
        );
    }
    for process in java {
        let taken = by_port.get(&process.port).is_some_and(|server| {
            server.status == Status::Running || server.pid == Some(process.pid)
        });
        if taken {
            continue;
        }
        by_port.insert(
            process.port,
            Server {
                port: process.port,
                kind: Kind::Java,
                status: Status::Running,
                owner: Owner::Other,
                pid: Some(process.pid),
                container: None,
                data: process.data,
                workspace: None,
                started_at: None,
                record: None,
            },
        );
    }
    for container in containers {
        let Some(port) = container
            .port
            .or_else(|| (container.name == EXAMPLES_CONTAINER).then_some(DEFAULT_PORT))
        else {
            continue;
        };
        let known = by_port.values().any(|server| {
            server.container.as_deref() == Some(container.name.as_str())
                || server.container.as_deref() == Some(container.id.as_str())
        });
        if known || by_port.contains_key(&port) {
            continue;
        }
        by_port.insert(
            port,
            Server {
                port,
                kind: Kind::Docker,
                status: if container.running {
                    Status::Running
                } else {
                    Status::Stopped
                },
                owner: Owner::Other,
                pid: None,
                container: Some(container.name),
                data: "container volume".to_string(),
                workspace: None,
                started_at: None,
                record: None,
            },
        );
    }
    by_port.into_values().collect()
}

/// The server on `port`, if dqlrs knows about or can see one.
pub(crate) fn server_on(port: u16) -> Option<Server> {
    servers().into_iter().find(|server| server.port == port)
}

/// What `start` should do.
#[derive(Clone, Debug)]
pub(crate) struct StartOptions {
    /// Java or Docker; `None` picks one (a stopped record's, else Java, else Docker).
    pub(crate) kind: Option<Kind>,
    /// `None` is 8000, or the next free port when `auto` was asked for (see `free_port`).
    pub(crate) port: Option<u16>,
    /// The workspace: Java data goes under `<dir>/.dynamodb-local/data`.
    pub(crate) dir: PathBuf,
    /// A data folder instead of the workspace's.
    pub(crate) data: Option<PathBuf>,
    /// Docker: start with `<dir>/docker-compose.yml` when it is the examples' file.
    pub(crate) use_workspace_compose: bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// The first port from 8000 up with nothing listening and no record.
pub(crate) fn free_port() -> u16 {
    let taken: Vec<u16> = servers().iter().map(|server| server.port).collect();
    (DEFAULT_PORT..u16::MAX)
        .find(|port| !taken.contains(port) && !local::port_open(*port))
        .unwrap_or(DEFAULT_PORT)
}

/// Start DynamoDB Local. Returns the record of the running server.
pub(crate) fn start(options: &StartOptions, progress: &Progress) -> color_eyre::Result<Record> {
    let port = options.port.unwrap_or(DEFAULT_PORT);
    let existing = server_on(port);
    if local::answers_like_dynamodb(port) {
        match &existing {
            Some(server) if server.owner == Owner::Dqlrs => {
                progress.say_check(&format!(
                    "DynamoDB Local is already running on {}:{port} ({})",
                    local::HOST,
                    server.kind.name()
                ));
                if let Some(record) = &server.record {
                    return Ok(record.clone());
                }
            }
            _ => {}
        }
        bail!(
            "something already answers on {}:{port}; stop it first, or start on another port with --port auto",
            local::HOST
        );
    }
    if local::port_open(port) {
        bail!("port {port} is in use by something that is not DynamoDB Local; pick another with --port");
    }
    // A stopped server on this port starts again with the same settings.
    let previous = existing.and_then(|server| server.record);
    let kind = match (options.kind, &previous) {
        (Some(kind), _) => kind,
        (None, Some(record)) => record.kind,
        (None, None) => pick_kind()?,
    };
    let record = match kind {
        Kind::Java => {
            let data_dir = match (&options.data, &previous) {
                (Some(data), _) => absolute(data)?,
                (None, Some(record)) if record.kind == Kind::Java && options.kind.is_none() => {
                    record
                        .data_dir
                        .clone()
                        .unwrap_or_else(|| workspace_data(&options.dir))
                }
                _ => workspace_data(&absolute(&options.dir)?),
            };
            start_java(port, &data_dir, Some(absolute(&options.dir)?), progress)?
        }
        Kind::Docker => {
            let compose_file = match &previous {
                Some(record) if record.kind == Kind::Docker && options.kind.is_none() => {
                    record.compose_file.clone()
                }
                _ => None,
            };
            let compose_file = match compose_file {
                Some(file) => file,
                None => compose_file_for(port, &options.dir, options.use_workspace_compose)?,
            };
            start_docker(port, &compose_file, Some(absolute(&options.dir)?), progress)?
        }
    };
    write_record(&record)?;
    progress.say("    Manage it with:");
    progress.say_code(
        4,
        &format!("dqlrs dynamo list | stop {port} | clean {port} | logs {port}"),
    );
    Ok(record)
}

fn pick_kind() -> color_eyre::Result<Kind> {
    let java = local::detect_java();
    if matches!(java, JavaState::Ready(_)) {
        return Ok(Kind::Java);
    }
    let docker = local::detect_docker();
    if matches!(docker, DockerState::Ready { .. }) {
        return Ok(Kind::Docker);
    }
    let java_why = match java {
        JavaState::TooOld(version) => format!("Java {version} is too old (17 or newer)"),
        _ => "Java was not found".to_string(),
    };
    let docker_why = match docker {
        DockerState::NotRunning(_) => "Docker is not running",
        DockerState::NoCompose(_) => "docker compose is not installed",
        _ => "Docker was not found",
    };
    bail!("cannot start DynamoDB Local: {java_why}, and {docker_why}")
}

/// `path` made absolute, without `.` parts, and with symlinks resolved when it exists.
fn absolute(path: &Path) -> color_eyre::Result<PathBuf> {
    if let Ok(real) = std::fs::canonicalize(path) {
        return Ok(real);
    }
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .wrap_err("current directory")?
            .join(path)
    };
    Ok(joined
        .components()
        .filter(|part| !matches!(part, std::path::Component::CurDir))
        .collect())
}

fn workspace_data(dir: &Path) -> PathBuf {
    dir.join(local::INSTALL_DIR).join("data")
}

fn start_java(
    port: u16,
    data_dir: &Path,
    workspace: Option<PathBuf>,
    progress: &Progress,
) -> color_eyre::Result<Record> {
    let install = install_dir();
    local::install_java(&install, progress)?;
    std::fs::create_dir_all(data_dir).wrap_err_with(|| format!("create {}", data_dir.display()))?;
    std::fs::create_dir_all(instances_dir())?;
    let pid = progress.step("Starting DynamoDB Local", |detail| {
        let log = log_path(port);
        let file =
            std::fs::File::create(&log).wrap_err_with(|| format!("create {}", log.display()))?;
        let mut command = Command::new("java");
        command
            .arg(format!(
                "-Djava.library.path={}",
                install.join("DynamoDBLocal_lib").display()
            ))
            .arg("-jar")
            .arg(install.join("DynamoDBLocal.jar"))
            .args(["-sharedDb", "-dbPath"])
            .arg(data_dir)
            .args(["-port", &port.to_string()])
            .current_dir(&install)
            .stdin(Stdio::null())
            .stdout(file.try_clone()?)
            .stderr(file);
        local::detach(&mut command);
        let mut child = command.spawn().wrap_err("start java")?;
        let pid = child.id();
        local::wait_until_up(port, detail, || match child.try_wait() {
            Ok(Some(status)) => Err(eyre!("java exited ({status}); see {}", log.display())),
            _ => Ok(()),
        })?;
        Ok((
            pid,
            format!(
                "DynamoDB Local is running on {}:{port} (Java, pid {pid})",
                local::HOST
            ),
        ))
    })?;
    Ok(Record {
        kind: Kind::Java,
        port,
        pid: Some(pid),
        data_dir: Some(data_dir.to_path_buf()),
        compose_file: None,
        container: None,
        data: data_dir.display().to_string(),
        workspace,
        started_at: now(),
    })
}

/// The examples' compose file when the workspace has it (and the port is its 8000), else a
/// compose file for `port` under the shared folder.
fn compose_file_for(port: u16, dir: &Path, use_workspace: bool) -> color_eyre::Result<PathBuf> {
    let workspace_file = absolute(dir)?.join("docker-compose.yml");
    if use_workspace || port == DEFAULT_PORT {
        if let Ok(text) = std::fs::read_to_string(&workspace_file) {
            if text.contains(IMAGE) && text.contains(EXAMPLES_CONTAINER) && port == DEFAULT_PORT {
                return Ok(workspace_file);
            }
        }
    }
    let folder = home().join("compose").join(port.to_string());
    std::fs::create_dir_all(&folder).wrap_err_with(|| format!("create {}", folder.display()))?;
    let file = folder.join("docker-compose.yml");
    std::fs::write(&file, generated_compose(port))
        .wrap_err_with(|| format!("write {}", file.display()))?;
    Ok(file)
}

fn generated_compose(port: u16) -> String {
    format!(
        "# Written by `dqlrs dynamo start --docker --port {port}`. `dqlrs dynamo clean {port}` removes\n\
         # the container and its volume.\n\
         services:\n\
         \x20 dynamodb-local:\n\
         \x20   image: \"{IMAGE}:latest\"\n\
         \x20   container_name: {EXAMPLES_CONTAINER}-{port}\n\
         \x20   command: \"-jar DynamoDBLocal.jar -sharedDb -dbPath ./data\"\n\
         \x20   working_dir: /home/dynamodblocal\n\
         \x20   user: root\n\
         \x20   ports:\n\
         \x20     - \"{port}:8000\"\n\
         \x20   volumes:\n\
         \x20     - \"data:/home/dynamodblocal/data\"\n\
         \n\
         volumes:\n\
         \x20 data:\n"
    )
}

fn compose_command() -> Command {
    match local::detect_docker() {
        DockerState::Ready {
            compose: Compose::Standalone,
            ..
        } => Command::new("docker-compose"),
        _ => {
            let mut command = Command::new("docker");
            command.arg("compose");
            command
        }
    }
}

/// Run docker compose with `file` (from its folder) and return stdout, or the last error line.
fn compose(file: &Path, args: &[&str]) -> color_eyre::Result<String> {
    let folder = file.parent().unwrap_or(Path::new("."));
    let output = compose_command()
        .arg("-f")
        .arg(file)
        .args(args)
        .current_dir(folder)
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
        bail!("docker compose {} failed: {last}", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn start_docker(
    port: u16,
    compose_file: &Path,
    workspace: Option<PathBuf>,
    progress: &Progress,
) -> color_eyre::Result<Record> {
    if !matches!(local::detect_docker(), DockerState::Ready { .. }) {
        bail!("Docker is not running, or docker compose is not installed");
    }
    progress.step("Starting DynamoDB Local with docker compose", |detail| {
        detail.set(format!("pulling {IMAGE}"));
        compose(compose_file, &["up", "-d"])?;
        detail.set("");
        local::wait_until_up(port, detail, || Ok(()))?;
        Ok((
            (),
            format!(
                "DynamoDB Local is running on {}:{port} (docker compose)",
                local::HOST
            ),
        ))
    })?;
    let container = compose(compose_file, &["ps", "-q"])
        .ok()
        .and_then(|out| out.lines().next().map(|id| id.trim().to_string()))
        .filter(|id| !id.is_empty());
    Ok(Record {
        kind: Kind::Docker,
        port,
        pid: None,
        data_dir: None,
        compose_file: Some(compose_file.to_path_buf()),
        container,
        data: format!("Docker volume ({})", compose_file.display()),
        workspace,
        started_at: now(),
    })
}

/// Stop a server and wait for its port to close. Data stays.
pub(crate) fn stop(server: &Server) -> color_eyre::Result<()> {
    if server.status == Status::Stopped {
        return Ok(());
    }
    match server.kind {
        Kind::Java => {
            let pid = server
                .pid
                .ok_or_else(|| eyre!("no pid for port {}", server.port))?;
            let status = if cfg!(unix) {
                Command::new("kill").arg(pid.to_string()).status()
            } else {
                Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .status()
            }
            .wrap_err("stop java")?;
            if !status.success() {
                bail!("could not stop pid {pid}");
            }
        }
        Kind::Docker => {
            match server
                .record
                .as_ref()
                .and_then(|record| record.compose_file.as_ref())
            {
                Some(file) => {
                    compose(file, &["stop"])?;
                }
                None => {
                    let container = server
                        .container
                        .as_deref()
                        .ok_or_else(|| eyre!("no container for port {}", server.port))?;
                    docker(&["stop", container])?;
                }
            }
        }
    }
    let started = Instant::now();
    while local::port_open(server.port) && started.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(200));
    }
    if local::port_open(server.port) {
        bail!("port {} is still open after 15 seconds", server.port);
    }
    Ok(())
}

fn docker(args: &[&str]) -> color_eyre::Result<String> {
    let output = Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .wrap_err("run docker")?;
    if !output.status.success() {
        bail!(
            "docker {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Stop a server dqlrs started and delete its data and record. `purge` also removes the
/// shared Java download (when no other Java server uses it) or the Docker image.
pub(crate) fn clean(server: &Server, purge: bool) -> color_eyre::Result<Vec<String>> {
    let record = server
        .record
        .as_ref()
        .ok_or_else(|| eyre!("port {} was not started by dqlrs; `dqlrs dynamo stop {}` stops it, but its data is left alone", server.port, server.port))?;
    let mut done = Vec::new();
    stop(server)?;
    match record.kind {
        Kind::Java => {
            if let Some(dir) = &record.data_dir {
                if dir.exists() {
                    std::fs::remove_dir_all(dir)
                        .wrap_err_with(|| format!("remove {}", dir.display()))?;
                    done.push(format!("Removed {}", dir.display()));
                }
            }
        }
        Kind::Docker => {
            if let Some(file) = &record.compose_file {
                compose(file, &["down", "-v"])?;
                done.push("Removed the container and its volume".to_string());
            }
        }
    }
    remove_record(record.port);
    if purge {
        match record.kind {
            Kind::Java => {
                let others = servers()
                    .iter()
                    .any(|other| other.kind == Kind::Java && other.owner == Owner::Dqlrs);
                let install = install_dir();
                if others {
                    done.push(format!(
                        "Kept {} (another Java server uses it)",
                        install.display()
                    ));
                } else if install.exists() {
                    std::fs::remove_dir_all(&install)
                        .wrap_err_with(|| format!("remove {}", install.display()))?;
                    done.push(format!("Removed {}", install.display()));
                }
            }
            Kind::Docker => match docker(&["image", "rm", &format!("{IMAGE}:latest")]) {
                Ok(_) => done.push(format!("Removed the {IMAGE} image")),
                Err(err) => done.push(format!("Kept the {IMAGE} image: {err}")),
            },
        }
    }
    Ok(done)
}

/// The server's log: the Java log file, or `docker logs`.
pub(crate) fn logs(server: &Server) -> color_eyre::Result<String> {
    match server.kind {
        Kind::Java => {
            let path = log_path(server.port);
            if server.owner == Owner::Other || !path.exists() {
                bail!(
                    "no log for port {}: it was not started by dqlrs",
                    server.port
                );
            }
            std::fs::read_to_string(&path).wrap_err_with(|| format!("read {}", path.display()))
        }
        Kind::Docker => {
            let container = server
                .container
                .as_deref()
                .ok_or_else(|| eyre!("no container for port {}", server.port))?;
            let output = Command::new("docker")
                .args(["logs", "--tail", "200", container])
                .output()
                .wrap_err("run docker logs")?;
            Ok(format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

/// `5 min ago`, `2 hr ago`, `3 days ago`.
pub(crate) fn ago(started_at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(started_at);
    match seconds {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} hr ago", seconds / 3600),
        _ => {
            let days = seconds / 86_400;
            format!("{days} day{} ago", if days == 1 { "" } else { "s" })
        }
    }
}

/// The JSON `list --json` and `/api/dynamo` give for a server.
pub(crate) fn server_json(server: &Server) -> Value {
    let mut value = serde_json::to_value(server).unwrap_or_else(|_| json!({}));
    if let Some(started) = server.started_at {
        value["started"] = json!(ago(started, now()));
    }
    value
}

// --- `dqlrs dynamo …` -------------------------------------------------------------------------

pub(crate) fn help_text() -> String {
    // One line per entry, laid out like `dqlrs --help` so Style::help colors it the same way.
    [
        "List, start, stop, and clean up DynamoDB Local servers.",
        "",
        "Usage:",
        "  dqlrs dynamo list [--json]          Every server dqlrs started or can see",
        "  dqlrs dynamo start [options]        Start a server (port 8000 unless --port)",
        "  dqlrs dynamo stop [PORT | --all]    Stop a server; its data stays",
        "  dqlrs dynamo clean [PORT | --all]   Stop a server dqlrs started and delete its data",
        "  dqlrs dynamo logs [PORT]            Print a server's log",
        "",
        "Start options:",
        "  --java, --docker      How to run it (default: Java when installed, else Docker)",
        "  --port <N|auto>       Port (default 8000; auto takes the next free one)",
        "  --dir <DIR>           Workspace: Java data goes in DIR/.dynamodb-local/data",
        "  --data <DIR>          A data folder instead of the workspace's",
        "",
        "Clean options:",
        "  --yes                 Do not ask before deleting data",
        "  --purge               Also remove the shared Java download or the Docker image",
        "",
        "Notes:",
        "  PORT can be left out when only one server is known. --all and clean never touch",
        "  servers that dqlrs did not start. The Java download is shared, in",
        "  ~/.local/share/dqlrs/dynamodb-local (DQLRS_DYNAMO_HOME overrides it).",
        "",
    ]
    .join("\n")
}

#[derive(Debug, Default, PartialEq)]
struct Parsed {
    verb: String,
    target: Option<u16>,
    all: bool,
    json: bool,
    purge: bool,
    yes: bool,
    kind: Option<Kind>,
    port: Option<String>,
    dir: Option<String>,
    data: Option<String>,
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut words = args.iter();
    parsed.verb = match words.next() {
        Some(verb) => verb.clone(),
        None => "help".to_string(),
    };
    let allowed: &[&str] = match parsed.verb.as_str() {
        "list" | "ls" => &["--json"],
        "start" => &["--java", "--docker", "--port", "--dir", "--data"],
        "stop" => &["--all"],
        "clean" => &["--all", "--purge", "--yes", "-y"],
        "logs" => &[],
        "help" | "-h" | "--help" => &[],
        other => return Err(format!("unknown dynamo command '{other}'")),
    };
    while let Some(word) = words.next() {
        let word = word.as_str();
        if word.starts_with('-') && !allowed.contains(&word) {
            return Err(format!(
                "'{word}' does not go with `dqlrs dynamo {}`",
                parsed.verb
            ));
        }
        let mut value = |name: &str| {
            words
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match word {
            "--json" => parsed.json = true,
            "--all" => parsed.all = true,
            "--purge" => parsed.purge = true,
            "--yes" | "-y" => parsed.yes = true,
            "--java" => parsed.kind = Some(Kind::Java),
            "--docker" => parsed.kind = Some(Kind::Docker),
            "--port" => parsed.port = Some(value("--port")?),
            "--dir" => parsed.dir = Some(value("--dir")?),
            "--data" => parsed.data = Some(value("--data")?),
            port if matches!(parsed.verb.as_str(), "stop" | "clean" | "logs") => {
                if parsed.target.is_some() {
                    return Err("give one PORT".to_string());
                }
                parsed.target = Some(
                    port.parse()
                        .map_err(|_| format!("'{port}' is not a port"))?,
                );
            }
            other => return Err(format!("unexpected '{other}'")),
        }
    }
    if parsed.all && parsed.target.is_some() {
        return Err("give a PORT or --all, not both".to_string());
    }
    Ok(parsed)
}

/// The servers a `stop`, `clean`, or `logs` command means.
fn targets(parsed: &Parsed, servers: &[Server]) -> Result<Vec<Server>, String> {
    if parsed.all {
        return Ok(servers
            .iter()
            .filter(|server| server.owner == Owner::Dqlrs)
            .cloned()
            .collect());
    }
    if let Some(port) = parsed.target {
        return servers
            .iter()
            .find(|server| server.port == port)
            .cloned()
            .map(|server| vec![server])
            .ok_or_else(|| format!("no DynamoDB Local server on port {port}"));
    }
    match servers {
        [] => Err("no DynamoDB Local servers".to_string()),
        [only] => Ok(vec![only.clone()]),
        _ => Err(format!(
            "{} servers; give a PORT ({})",
            servers.len(),
            servers
                .iter()
                .map(|server| server.port.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn table(servers: &[Server], style: Style) -> String {
    if servers.is_empty() {
        return style.rich("No DynamoDB Local servers. Start one with `dqlrs dynamo start`.")
            + "\n";
    }
    let now = now();
    let rows: Vec<[String; 6]> = servers
        .iter()
        .map(|server| {
            [
                server.port.to_string(),
                server.kind.name().to_string(),
                match server.status {
                    Status::Running => "running".to_string(),
                    Status::Stopped => "stopped".to_string(),
                },
                server.data.clone(),
                server
                    .started_at
                    .map(|started| ago(started, now))
                    .unwrap_or_else(|| "?".to_string()),
                match server.owner {
                    Owner::Dqlrs => "dqlrs".to_string(),
                    Owner::Other => "other".to_string(),
                },
            ]
        })
        .collect();
    let head = ["PORT", "RUNTIME", "STATUS", "DATA", "STARTED", "BY"];
    let widths: Vec<usize> = (0..6)
        .map(|col| {
            rows.iter()
                .map(|row| row[col].chars().count())
                .chain([head[col].len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    // Pad first, then color, so the escape codes do not count toward the widths.
    let line = |cells: &[String], paint: &dyn Fn(usize, &str) -> String| {
        cells
            .iter()
            .enumerate()
            .map(|(col, cell)| {
                let padded = if col == cells.len() - 1 {
                    cell.clone()
                } else {
                    format!("{cell:<width$}", width = widths[col])
                };
                paint(col, &padded)
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let mut out = line(&head.map(String::from), &|_, cell| style.heading(cell)) + "\n";
    for row in rows {
        let paint = |col: usize, cell: &str| match col {
            0 => style.prompt(cell),
            1 => style.code(cell),
            2 if row[2] == "running" => style.notice(cell),
            2 => style.warn(cell),
            3 => style.path(cell),
            4 => style.dim(cell),
            _ if row[5] == "other" => style.warn(cell),
            _ => style.dim(cell),
        };
        out.push_str(&line(&row, &paint));
        out.push('\n');
    }
    out
}

fn confirm(question: &str, style: Style) -> io::Result<bool> {
    let mut out = io::stdout();
    write!(out, "{} ", style.prompt(&format!("{question} [y/N]")))?;
    out.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// `dqlrs dynamo …`. `args` are the words after `dynamo`.
pub fn run(args: &[String]) -> color_eyre::Result<()> {
    let style = Style::stdout();
    let progress = Progress::stdout();
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            let style = Style::stderr();
            eprintln!(
                "{}\n\n{}",
                style.error(&err),
                style.help(help_text().trim_end())
            );
            std::process::exit(2);
        }
    };
    match parsed.verb.as_str() {
        "help" | "-h" | "--help" => {
            println!("{}", style.help(help_text().trim_end()));
        }
        "list" | "ls" => {
            let servers = servers();
            if parsed.json {
                let values: Vec<Value> = servers.iter().map(server_json).collect();
                println!("{}", style.json(&serde_json::to_string_pretty(&values)?));
            } else {
                print!("{}", table(&servers, style));
            }
        }
        "start" => {
            let port = match parsed.port.as_deref() {
                None => None,
                Some("auto") => Some(free_port()),
                Some(port) => Some(
                    port.parse()
                        .map_err(|_| eyre!("--port takes a number or auto, not '{port}'"))?,
                ),
            };
            let options = StartOptions {
                kind: parsed.kind,
                port,
                dir: PathBuf::from(parsed.dir.as_deref().unwrap_or(".")),
                data: parsed.data.as_ref().map(PathBuf::from),
                use_workspace_compose: true,
            };
            start(&options, &progress)?;
        }
        "stop" => {
            let servers = servers();
            for server in targets(&parsed, &servers).map_err(|err| eyre!(err))? {
                if server.status == Status::Stopped {
                    progress.note(&format!("Port {} is already stopped", server.port));
                    continue;
                }
                stop(&server)?;
                progress.say_check(&format!(
                    "Stopped DynamoDB Local on port {} ({}); its data stays",
                    server.port,
                    server.kind.name()
                ));
            }
        }
        "clean" => {
            let servers = servers();
            let targets = targets(&parsed, &servers).map_err(|err| eyre!(err))?;
            if targets.is_empty() {
                progress.note("No servers started by dqlrs to clean");
            }
            for server in targets {
                if server.owner == Owner::Other {
                    bail!(
                        "port {} was not started by dqlrs, so dqlrs will not delete its data; `dqlrs dynamo stop {}` stops it",
                        server.port,
                        server.port
                    );
                }
                if !parsed.yes {
                    if !io::stdin().is_terminal() {
                        bail!("clean deletes data; pass --yes to run it without a terminal");
                    }
                    let what = if parsed.purge {
                        format!("{}, and the shared download or image", server.data)
                    } else {
                        server.data.clone()
                    };
                    if !confirm(
                        &format!("Stop port {} and delete {what}?", server.port),
                        style,
                    )? {
                        progress.note(&format!("Left port {} alone", server.port));
                        continue;
                    }
                }
                for line in clean(&server, parsed.purge)? {
                    progress.say_check(&line);
                }
                progress.say_check(&format!("Cleaned port {}", server.port));
            }
        }
        "logs" => {
            let servers = servers();
            for server in targets(&parsed, &servers).map_err(|err| eyre!(err))? {
                print!("{}", logs(&server)?);
            }
        }
        _ => unreachable!("parse refuses other verbs"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    fn record(kind: Kind, port: u16) -> Record {
        Record {
            kind,
            port,
            pid: (kind == Kind::Java).then_some(100 + u32::from(port)),
            data_dir: None,
            compose_file: None,
            container: (kind == Kind::Docker).then(|| format!("c{port}")),
            data: format!("data{port}"),
            workspace: None,
            started_at: 1_000,
        }
    }

    #[test]
    fn finds_only_the_java_server_itself() {
        let listing = "\
  101 java -Djava.library.path=./DynamoDBLocal_lib -jar DynamoDBLocal.jar -sharedDb -inMemory
  102 java -jar /home/a/.local/share/dqlrs/dynamodb-local/install/DynamoDBLocal.jar -sharedDb -dbPath /w/.dynamodb-local/data -port 8001
  103 bash -c echo DynamoDBLocal.jar
  104 vim DynamoDBLocal.jar
";
        assert_eq!(
            parse_java_processes(listing),
            vec![
                JavaProcess {
                    pid: 101,
                    port: 8000,
                    data: "in memory".to_string()
                },
                JavaProcess {
                    pid: 102,
                    port: 8001,
                    data: "/w/.dynamodb-local/data".to_string()
                },
            ]
        );
    }

    #[test]
    fn reads_dynamodb_local_containers_and_their_host_port() {
        let listing = "abc\tdqlrs-dynamodb-local\tamazon/dynamodb-local:latest\trunning\t0.0.0.0:8000->8000/tcp, :::8000->8000/tcp\n\
                       def\tother\tpostgres:16\trunning\t0.0.0.0:5432->5432/tcp\n\
                       ghi\tdqlrs-dynamodb-local-8002\tamazon/dynamodb-local\texited\t\n";
        assert_eq!(
            parse_containers(listing),
            vec![
                Container {
                    id: "abc".to_string(),
                    name: "dqlrs-dynamodb-local".to_string(),
                    running: true,
                    port: Some(8000)
                },
                Container {
                    id: "ghi".to_string(),
                    name: "dqlrs-dynamodb-local-8002".to_string(),
                    running: false,
                    port: None
                },
            ]
        );
        assert_eq!(host_port("127.0.0.1:9000->8000/tcp"), Some(9000));
        assert_eq!(host_port("0.0.0.0:9000->9999/tcp"), None);
    }

    #[test]
    fn merges_records_with_scans_and_marks_the_rest_other() {
        let java = vec![
            JavaProcess {
                pid: 8100,
                port: 8000,
                data: "x".to_string(),
            },
            JavaProcess {
                pid: 9,
                port: 8005,
                data: "in memory".to_string(),
            },
        ];
        let containers = vec![
            Container {
                id: "c8001".to_string(),
                name: "n1".to_string(),
                running: true,
                port: Some(8001),
            },
            Container {
                id: "z".to_string(),
                name: "stray".to_string(),
                running: false,
                port: Some(8009),
            },
        ];
        let records = vec![
            record(Kind::Java, 8000),
            record(Kind::Docker, 8001),
            record(Kind::Java, 8002),
        ];
        let servers = merge(records, java, containers, &|record| record.port == 8000);
        let summary: Vec<(u16, Kind, Status, Owner)> = servers
            .iter()
            .map(|server| (server.port, server.kind, server.status, server.owner))
            .collect();
        assert_eq!(
            summary,
            vec![
                (8000, Kind::Java, Status::Running, Owner::Dqlrs),
                (8001, Kind::Docker, Status::Running, Owner::Dqlrs),
                (8002, Kind::Java, Status::Stopped, Owner::Dqlrs),
                (8005, Kind::Java, Status::Running, Owner::Other),
                (8009, Kind::Docker, Status::Stopped, Owner::Other),
            ]
        );
    }

    #[test]
    fn a_stopped_record_gives_way_to_another_process_on_its_port() {
        let java = vec![JavaProcess {
            pid: 1,
            port: 8002,
            data: "y".to_string(),
        }];
        let servers = merge(vec![record(Kind::Java, 8002)], java, Vec::new(), &|_| false);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].owner, Owner::Other);
        assert_eq!(servers[0].pid, Some(1));
    }

    #[test]
    fn parses_each_command() {
        assert!(parse(&words("list --json")).unwrap().json);
        let start = parse(&words("start --docker --port auto --dir ws --data d")).unwrap();
        assert_eq!(start.kind, Some(Kind::Docker));
        assert_eq!(start.port.as_deref(), Some("auto"));
        assert_eq!(start.dir.as_deref(), Some("ws"));
        assert_eq!(start.data.as_deref(), Some("d"));
        assert_eq!(parse(&words("stop 8001")).unwrap().target, Some(8001));
        let clean = parse(&words("clean --all --purge --yes")).unwrap();
        assert!(clean.all && clean.purge && clean.yes);
        assert_eq!(parse(&[]).unwrap().verb, "help");
        assert!(parse(&words("stop --purge")).is_err());
        assert!(parse(&words("stop 8001 --all")).is_err());
        assert!(parse(&words("start 8001")).is_err());
        assert!(parse(&words("frobnicate")).is_err());
        assert!(parse(&words("stop eighty")).is_err());
    }

    #[test]
    fn targets_need_a_port_unless_one_server_is_known() {
        let mut other = merge(
            Vec::new(),
            vec![JavaProcess {
                pid: 1,
                port: 8005,
                data: String::new(),
            }],
            Vec::new(),
            &|_| false,
        );
        let mine = merge(
            vec![record(Kind::Java, 8000)],
            Vec::new(),
            Vec::new(),
            &|_| true,
        );
        let both: Vec<Server> = mine.iter().cloned().chain(other.drain(..)).collect();
        let stop = |text: &str| parse(&words(text)).unwrap();
        assert!(targets(&stop("stop"), &both)
            .unwrap_err()
            .contains("8000, 8005"));
        assert_eq!(targets(&stop("stop 8005"), &both).unwrap()[0].port, 8005);
        // --all leaves servers dqlrs did not start alone.
        let all = targets(&stop("stop --all"), &both).unwrap();
        assert_eq!(
            all.iter().map(|server| server.port).collect::<Vec<_>>(),
            vec![8000]
        );
        assert_eq!(targets(&stop("logs"), &mine).unwrap()[0].port, 8000);
        assert!(targets(&stop("stop 9999"), &both).is_err());
    }

    #[test]
    fn records_round_trip_through_json() {
        let mut java = record(Kind::Java, 8000);
        java.data_dir = Some(PathBuf::from("/w/.dynamodb-local/data"));
        let text = serde_json::to_string(&java).unwrap();
        assert!(text.contains("\"kind\":\"java\""));
        assert_eq!(serde_json::from_str::<Record>(&text).unwrap(), java);
    }

    #[test]
    fn says_how_long_ago() {
        assert_eq!(ago(1_000, 1_030), "just now");
        assert_eq!(ago(1_000, 1_000 + 5 * 60), "5 min ago");
        assert_eq!(ago(1_000, 1_000 + 2 * 3600), "2 hr ago");
        assert_eq!(ago(1_000, 1_000 + 86_400), "1 day ago");
        assert_eq!(ago(1_000, 1_000 + 3 * 86_400), "3 days ago");
    }

    #[test]
    fn generated_compose_maps_the_port_and_names_the_container() {
        let text = generated_compose(8003);
        assert!(text.contains("\"8003:8000\""));
        assert!(text.contains("container_name: dqlrs-dynamodb-local-8003"));
        assert!(text.contains("image: \"amazon/dynamodb-local:latest\""));
    }

    #[test]
    fn table_lists_every_column() {
        let servers = merge(
            vec![record(Kind::Java, 8000)],
            Vec::new(),
            Vec::new(),
            &|_| true,
        );
        let text = table(&servers, Style::new(false));
        assert!(text.starts_with("PORT  RUNTIME  STATUS   DATA"), "{text}");
        assert!(text.contains("8000  java     running  data8000"), "{text}");
        assert!(table(&[], Style::new(false)).contains("dqlrs dynamo start"));
    }
}
