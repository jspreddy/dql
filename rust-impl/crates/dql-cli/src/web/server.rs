//! HTTP API and static files for the web UI.
//!
//! Each connection answers one request completely before reading the next.
//! The editor loads many scripts at once; overlapping keep-alive writes were
//! leaving some of those responses unsent.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use dql_parser::{parse_statement, Statement};
use include_dir::{include_dir, Dir};
use serde_json::{json, Value};

use super::files::{self, PathError};
use super::presence::{self, Kind};
use super::progress::{Detail, Progress};
use super::query_cache;
use super::shape::{
    changed_tables, column_order, explain_index, parse_table_shape, read_query, safe_table_name,
    split_statements, Column, ExecText, ListedTable, TableShape, PAGE_SIZE,
};
use super::structure;
use crate::serve::protocol::{envelope_from_error, ServeEnvelope};
use crate::session::Session;
use crate::table_cache::LoadEvent;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../../dqlrs-web/static");

const HEADER_LIMIT: usize = 64 * 1024;
const BODY_LIMIT: usize = 2_000_000;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

struct FetchedPage {
    items: Vec<Value>,
    columns: Vec<Column>,
    shape: Option<TableShape>,
}

struct HeldResult {
    statement: String,
    items: Vec<Value>,
    columns: Vec<Column>,
}

struct WebState {
    session: Session,
    shapes: HashMap<String, TableShape>,
    table_catalog: Option<Vec<ListedTable>>,
    mcp_initialized: bool,
    /// The table page most recently returned by `/api/rows`.
    fetched_pages: HashMap<String, FetchedPage>,
    /// Query results from this process that have not been read back from the cache.
    held_results: HashMap<String, HeldResult>,
    next_result: u64,
}

pub(crate) struct App {
    pub(crate) root: PathBuf,
    endpoint: String,
    state: Mutex<WebState>,
    cancel: dql_engine::CancelFlag,
    identity: Mutex<Option<dql_engine::CallerIdentity>>,
    presence: presence::Book,
    pub(crate) hub: super::events::Hub,
    access: super::access::Store,
    mcp_hub: super::mcp_stream::Hub,
    /// Whether `/mcp` answers. `--mcp` sets it at start; the header's MCP pill turns it on and off.
    mcp: AtomicBool,
    /// `http://<bound address>/mcp`, set once the listener is bound.
    mcp_url: std::sync::OnceLock<String>,
    token: String,
}

struct Incoming {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
    origin: Option<String>,
    mcp_session: Option<String>,
    accept: Option<String>,
    authorization: Option<String>,
    keep_alive: bool,
    overflow: bool,
}

enum Reply {
    Json(u16, Value),
    /// JSON with an optional `Mcp-Session-Id` response header.
    McpJson(u16, Value, Option<String>),
    Bytes(u16, &'static str, Vec<u8>),
    Ndjson(Arc<App>, Vec<String>, dql_engine::FileBase),
    Events(Arc<App>, String),
    McpEvents(Arc<App>, String),
}

pub fn serve(
    session: Session,
    root: PathBuf,
    listen: SocketAddr,
    mcp: bool,
    load_tables: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let token = crate::api_token::load()?;
    let app = Arc::new(App::new(session, root, mcp, token));
    super::events::start(&app);
    // Bind first so a busy port fails before a long table load. Connections wait in the
    // backlog until `accept` runs.
    let listener = TcpListener::bind(listen)?;
    let bound = listener.local_addr().unwrap_or(listen);
    let _ = app.mcp_url.set(format!("http://{bound}/mcp"));
    let loaded = if load_tables {
        Some(load_tables_with_progress(&app, &Progress::stdout()))
    } else {
        None
    };
    // Colors only in a terminal; piped, this is the same plain text as before.
    let style = crate::style::Style::stdout();
    const WIDTH: usize = 11;
    println!(
        "{}  {}",
        style.title("DQLRS Web"),
        style.url(&format!("http://{bound}"))
    );
    if let Some(count) = loaded {
        println!(
            "{}",
            style.row("tables", WIDTH, &format!("{count} definitions loaded"))
        );
    }
    if mcp {
        let url = format!("http://{bound}/mcp");
        println!("{}", style.row("MCP", WIDTH, &style.url(&url)));
        println!(
            "{}",
            style.row(
                "workspace",
                WIDTH,
                &style.path(&app.root.display().to_string())
            )
        );
        println!("{}", style.row("endpoint", WIDTH, &app.endpoint));
        println!();
        println!(
            "{}",
            style.config(&crate::mcp::http_client_config(&url, &app.token))
        );
    } else {
        println!(
            "{}{}{}{}{}",
            style.label(&format!("{:<WIDTH$}", "MCP")),
            style.dim("off. Pass "),
            style.code("--mcp"),
            style.dim(" to serve "),
            style.url(&format!("http://{bound}/mcp"))
        );
        println!(
            "{}",
            style.row(
                "workspace",
                WIDTH,
                &style.path(&app.root.display().to_string())
            )
        );
        println!("{}", style.row("endpoint", WIDTH, &app.endpoint));
    }
    let _ = io::stdout().flush();
    let err_style = crate::style::Style::stderr();
    eprintln!(
        "{}{}",
        err_style.label("dqlrs token "),
        err_style.dim(&app.token)
    );
    let _ = io::stderr().flush();
    accept(app, listener, Arc::new(AtomicBool::new(false)));
    Ok(())
}

/// Load every table definition before the server starts, with a progress bar, and return
/// how many loaded. Only failures get their own line. A failure is reported and the server
/// starts anyway, as with a background load.
fn load_tables_with_progress(app: &Arc<App>, progress: &Progress) -> usize {
    progress.say(&format!("Connected to {}", connection_line(app)));
    let started = std::time::Instant::now();
    let mut loaded = 0;
    let result = progress.step("Loading table definitions", |detail| {
        detail.set("listing tables");
        let mut failed = 0;
        let outcome = run_table_load(app, &mut |event| {
            report_load_event(detail, event, &mut loaded, &mut failed)
        });
        let seconds = started.elapsed().as_secs_f64();
        outcome.map_err(|err| color_eyre::eyre::eyre!(err))?;
        let noun = if loaded == 1 {
            "definition"
        } else {
            "definitions"
        };
        let summary = if failed == 0 {
            format!("Loaded {loaded} table {noun} in {seconds:.1}s")
        } else {
            format!("Loaded {loaded} table {noun} in {seconds:.1}s, {failed} failed")
        };
        Ok(((), summary))
    });
    if let Err(err) = result {
        println!("    {err}");
    }
    progress.say("");
    loaded
}

/// Line 1 of the load progress: the connection, and on AWS the account and the role or user
/// it runs as. The identity is cached for the UI's identity panel.
fn connection_line(app: &App) -> String {
    let (local, memory) = {
        let state = lock_state(app);
        (
            state.session.local_endpoint.is_some(),
            state.session.engine.is_memory(),
        )
    };
    if memory {
        return "the in-memory backend".to_string();
    }
    if local {
        return format!("{} · DynamoDB Local", app.endpoint);
    }
    let fetched = lock_state(app).session.engine.caller_identity();
    match fetched {
        Ok(identity) => {
            let line = format!(
                "{} · account {} · {}",
                app.endpoint,
                identity.account,
                principal_label(&identity.arn)
            );
            *app.identity.lock().expect("caller identity") = Some(identity);
            line
        }
        Err(err) => format!("{} · identity unknown ({err})", app.endpoint),
    }
}

/// `role Admin (jonsmith)` for an assumed role, `user alice` for an IAM user, else the
/// principal as the ARN names it.
fn principal_label(arn: &str) -> String {
    let principal = super::environment::principal_from_arn(arn);
    if arn.contains(":assumed-role/") || arn.contains(":role/") {
        format!("role {principal}")
    } else if arn.contains(":user/") {
        format!("user {principal}")
    } else if arn.ends_with(":root") {
        "root".to_string()
    } else {
        principal
    }
}

/// Advance the bar. A table that fails to load gets a line of its own with the error.
fn report_load_event(detail: &Detail, event: LoadEvent, loaded: &mut usize, failed: &mut usize) {
    match event {
        LoadEvent::Listed(total) => {
            detail.set_bar(0, total);
            detail.set("");
        }
        LoadEvent::Described {
            index,
            total,
            name,
            result,
        } => {
            match result {
                Ok(Some(_)) => *loaded += 1,
                // Dropped between the listing and the describe: nothing to load.
                Ok(None) => {}
                Err(err) => {
                    *failed += 1;
                    detail.log(false, name, &err);
                }
            }
            detail.set_bar(index, total);
        }
    }
}

/// The table list and every definition, through the workspace's disk cache when there is
/// one, else through the session.
fn run_table_load(app: &Arc<App>, report: &mut dyn FnMut(LoadEvent)) -> Result<(), String> {
    let disk = {
        let state = lock_state(app);
        state.session.table_disk.clone()
    };
    match disk {
        Some(cache) => cache.load_with(report),
        None => load_session_tables(app, report),
    }
}

fn load_session_tables(app: &Arc<App>, report: &mut dyn FnMut(LoadEvent)) -> Result<(), String> {
    let names = {
        let mut state = lock_state(app);
        state
            .session
            .engine
            .cached_table_names(true)
            .map_err(|err| err.to_string())?
    };
    report(LoadEvent::Listed(names.len()));
    for (index, name) in names.iter().enumerate() {
        let mut state = lock_state(app);
        let described = state.session.engine.describe(name, false);
        let result = match &described {
            Ok(meta) => Ok(meta.as_ref()),
            Err(err) => Err(err.to_string()),
        };
        report(LoadEvent::Described {
            index: index + 1,
            total: names.len(),
            name,
            result,
        });
    }
    Ok(())
}

fn accept(app: Arc<App>, listener: TcpListener, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(_) => break,
        };
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let app = Arc::clone(&app);
        thread::spawn(move || {
            if let Err(err) = handle_connection(app, stream) {
                if !is_disconnect(&err) {
                    eprintln!(
                        "{}",
                        crate::style::Style::stderr().warn(&format!("dqlrs web: {err}"))
                    );
                }
            }
        });
    }
}

impl App {
    /// The current connection's key. Query results are saved and read under it.
    fn connection_key(&self) -> String {
        lock_state(self).session.connection_key.clone()
    }

    /// Query results for `connection_key` in this workspace.
    fn results<'a>(&'a self, connection_key: &'a str) -> query_cache::Store<'a> {
        query_cache::Store {
            workspace: &self.root,
            connection_key,
        }
    }

    fn new(mut session: Session, root: PathBuf, mcp: bool, token: String) -> Self {
        // The served folder is the workspace. lib.rs passes session.workspace, so this only
        // changes a session built some other way, such as in tests.
        session.serves_mcp = mcp;
        if session.workspace != root {
            session.workspace = root.clone();
            if !session.engine.is_memory() {
                session.attach_connection_storage();
            }
        }
        let endpoint = match &session.local_endpoint {
            Some((host, port)) => format!("{host}:{port}"),
            None => session.region.clone(),
        };
        let cancel = session.cancel().clone();
        Self {
            root,
            endpoint,
            state: Mutex::new(WebState {
                session,
                shapes: HashMap::new(),
                table_catalog: None,
                mcp_initialized: false,
                fetched_pages: HashMap::new(),
                held_results: HashMap::new(),
                next_result: 0,
            }),
            cancel,
            identity: Mutex::new(None),
            presence: presence::Book::new(),
            hub: super::events::Hub::new(),
            access: super::access::Store::new(),
            mcp_hub: super::mcp_stream::Hub::new(),
            mcp: AtomicBool::new(mcp),
            mcp_url: std::sync::OnceLock::new(),
            token,
        }
    }
}

fn handle_connection(app: Arc<App>, stream: TcpStream) -> io::Result<()> {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(IDLE_TIMEOUT));
    let mut reader = BufReader::new(stream);
    loop {
        reader.get_ref().set_read_timeout(Some(IDLE_TIMEOUT))?;
        let request = match read_request(&mut reader) {
            Ok(Some(request)) => request,
            Ok(None) => return Ok(()),
            Err(err) if is_disconnect(&err) => return Ok(()),
            Err(err) => return Err(err),
        };
        reader.get_ref().set_read_timeout(None)?;
        if request.overflow {
            write_bytes(
                reader.get_mut(),
                400,
                "application/json; charset=utf-8",
                br#"{"ok":false,"error":"body too large"}"#,
                false,
                None,
            )?;
            return Ok(());
        }
        let keep_alive = request.keep_alive;
        let reply = dispatch(&app, &request);
        match write_reply(reader.get_mut(), reply, keep_alive) {
            Ok(true) => {}
            Ok(false) => {
                let _ = reader.get_mut().shutdown(Shutdown::Write);
                return Ok(());
            }
            Err(err) if is_disconnect(&err) => return Ok(()),
            Err(err) => return Err(err),
        }
    }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Option<Incoming>> {
    let mut raw = Vec::new();
    loop {
        if raw.len() > HEADER_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
        let n = reader.read_until(b'\n', &mut raw)?;
        if n == 0 {
            return if raw.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated headers",
                ))
            };
        }
        if raw.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let version = parts.next().unwrap_or("HTTP/1.0");
    let (path, query) = split_url(target);
    let mut content_length = None;
    let mut connection = String::new();
    let mut origin = None;
    let mut mcp_session = None;
    let mut accept = None;
    let mut authorization = None;
    let mut length_invalid = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            match value.trim().parse::<usize>() {
                Ok(len) => content_length = Some(len),
                Err(_) => length_invalid = true,
            }
        } else if name.eq_ignore_ascii_case("connection") {
            connection = value.trim().to_ascii_lowercase();
        } else if name.eq_ignore_ascii_case("origin") {
            origin = Some(value.trim().to_string());
        } else if name.eq_ignore_ascii_case("mcp-session-id") {
            mcp_session = Some(value.trim().to_string());
        } else if name.eq_ignore_ascii_case("accept") {
            accept = Some(value.trim().to_string());
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.trim().to_string());
        }
    }
    let keep_alive = if connection.contains("close") {
        false
    } else if connection.contains("keep-alive") {
        true
    } else {
        version == "HTTP/1.1"
    };
    if length_invalid {
        return Ok(Some(Incoming {
            method,
            path: path.to_string(),
            query: query.to_string(),
            body: Vec::new(),
            origin,
            mcp_session,
            accept,
            authorization: authorization.clone(),
            keep_alive: false,
            overflow: true,
        }));
    }
    let len = content_length.unwrap_or(0);
    if len > BODY_LIMIT {
        return Ok(Some(Incoming {
            method,
            path: path.to_string(),
            query: query.to_string(),
            body: Vec::new(),
            origin,
            mcp_session,
            accept,
            authorization: authorization.clone(),
            keep_alive: false,
            overflow: true,
        }));
    }
    let mut body = vec![0; len];
    if len > 0 {
        reader.read_exact(&mut body)?;
    }
    Ok(Some(Incoming {
        method,
        path: path.to_string(),
        query: query.to_string(),
        body,
        origin,
        mcp_session,
        accept,
        authorization,
        keep_alive,
        overflow: false,
    }))
}

fn config_payload(app: &App) -> Value {
    let (local, region, memory) = {
        let state = app.state.lock().expect("web session");
        (
            state.session.local_endpoint.clone(),
            state.session.region.clone(),
            state.session.engine.is_memory(),
        )
    };
    let environment = super::environment::environment_view(
        local.as_ref().map(|(host, port)| (host.as_str(), *port)),
        &region,
        memory,
        &super::environment::EnvFacts::from_env(),
    );
    json!({
        "endpoint": app.endpoint,
        "root": app.root.display().to_string(),
        "page_size": PAGE_SIZE,
        "environment": environment,
        "mcp": { "enabled": app.mcp_enabled(), "path": "/mcp" },
    })
}

impl App {
    fn mcp_enabled(&self) -> bool {
        self.mcp.load(Ordering::SeqCst)
    }

    /// Turn `/mcp` on or off. Off closes the agents' event streams; their next call gets 404.
    /// Grants and pending requests stay, so turning it back on picks up where it was.
    fn set_mcp(&self, enabled: bool) {
        let was = self.mcp.swap(enabled, Ordering::SeqCst);
        lock_state(self).session.serves_mcp = enabled;
        if was == enabled {
            return;
        }
        if !enabled {
            self.mcp_hub.close_all();
        }
        self.hub.mcp(enabled);
        let style = crate::style::Style::stdout();
        let state = if enabled { "on" } else { "off" };
        println!(
            "{}",
            style.row("MCP", 11, &format!("turned {state} in the web app"))
        );
        let _ = io::stdout().flush();
    }
}

/// The MCP menu: whether /mcp answers, its address, and the Cursor and Claude config for it.
fn mcp_info(app: &App) -> Value {
    let url = app
        .mcp_url
        .get()
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1/mcp".to_string());
    let (cursor, claude) = crate::mcp::http_client_snippets(&url, &app.token);
    json!({
        "ok": true,
        "enabled": app.mcp_enabled(),
        "url": url,
        "config": { "cursor": cursor, "claude": claude },
    })
}

fn mcp_reply(app: &App, body: &Value) -> Reply {
    let Some(enabled) = body.get("enabled").and_then(Value::as_bool) else {
        return Reply::Json(
            400,
            json!({"ok": false, "error": "enabled must be true or false"}),
        );
    };
    app.set_mcp(enabled);
    Reply::Json(200, json!({"ok": true, "enabled": app.mcp_enabled()}))
}

/// The DynamoDB Local server behind the endpoint, for the endpoint chip: whether the endpoint is
/// local, whether its port answers like DynamoDB, and what `dqlrs dynamo` knows about it.
fn dynamo_reply(app: &App) -> Value {
    let local = {
        let state = app.state.lock().expect("web session");
        state.session.local_endpoint.clone()
    };
    let Some((host, port)) = local else {
        return json!({"local": false});
    };
    let loopback = matches!(
        host.trim_start_matches("http://")
            .trim_start_matches("https://"),
        "localhost" | "127.0.0.1" | "::1" | "[::1]"
    );
    if !loopback {
        return json!({"local": false, "host": host, "port": port});
    }
    json!({
        "local": true,
        "host": host,
        "port": port,
        "answering": super::dynamodb_local::answers_like_dynamodb(port),
        "server": crate::dynamo::server_on(port).map(|server| crate::dynamo::server_json(&server)),
    })
}

fn identity_reply(app: &App) -> Value {
    let (local, memory) = {
        let state = app.state.lock().expect("web session");
        (
            state.session.local_endpoint.is_some(),
            state.session.engine.is_memory(),
        )
    };
    if local {
        return json!({"ok": false, "error": "localhost has no caller identity"});
    }
    if memory {
        return json!({"ok": false, "error": "memory backend has no caller identity"});
    }
    if let Some(identity) = app.identity.lock().expect("caller identity").clone() {
        return super::environment::identity_payload(&identity);
    }
    let fetched = {
        let state = app.state.lock().expect("web session");
        state.session.engine.caller_identity()
    };
    match fetched {
        Ok(identity) => {
            let payload = super::environment::identity_payload(&identity);
            *app.identity.lock().expect("caller identity") = Some(identity);
            payload
        }
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn dispatch(app: &Arc<App>, request: &Incoming) -> Reply {
    if request.path == "/mcp" || request.path == "/mcp/" {
        if !app.mcp_enabled() {
            return Reply::Json(404, json!({"ok": false, "error": "not found"}));
        }
        if !authorized(app, request) {
            return unauthorized_reply(true);
        }
        return dispatch_mcp(app, request);
    }
    let path = request.path.as_str();
    if (path == "/api" || path.starts_with("/api/")) && !authorized(app, request) {
        return unauthorized_reply(false);
    }
    let query = request.query.as_str();
    match request.method.as_str() {
        "GET" if path == "/api/events" => events_reply(app, query),
        "GET" => dispatch_get(app, path, query),
        "POST" => dispatch_post(app, path, &request.body),
        "PUT" => dispatch_put(app, path, &request.body),
        _ => Reply::Json(404, json!({"ok": false, "error": "not found"})),
    }
}

fn events_reply(app: &Arc<App>, query: &str) -> Reply {
    let id = query_value(query, "id");
    if presence::valid_web_id(&id).is_none() {
        return Reply::Json(400, json!({"ok": false, "error": "client id is invalid"}));
    }
    Reply::Events(Arc::clone(app), id)
}

fn authorized(app: &App, request: &Incoming) -> bool {
    let Some(header) = request.authorization.as_deref() else {
        return false;
    };
    let Some(presented) = bearer_token(header) else {
        return false;
    };
    crate::api_token::matches(&app.token, presented)
}

fn bearer_token(header: &str) -> Option<&str> {
    let mut parts = header.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    Some(token)
}

fn unauthorized_reply(mcp: bool) -> Reply {
    if mcp {
        return Reply::Json(
            401,
            json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32001, "message": "unauthorized" }
            }),
        );
    }
    Reply::Json(401, json!({"ok": false, "error": "unauthorized"}))
}

fn dispatch_get(app: &App, path: &str, query: &str) -> Reply {
    match path {
        "/api/config" => Reply::Json(200, config_payload(app)),
        "/api/mcp-access" => Reply::Json(200, mcp_access_payload(app)),
        "/api/mcp" => Reply::Json(200, mcp_info(app)),
        "/api/identity" => Reply::Json(200, identity_reply(app)),
        "/api/dynamo" => Reply::Json(200, dynamo_reply(app)),
        "/api/tree" => match files::tree(&app.root) {
            Ok(node) => Reply::Json(200, serde_json::to_value(node).unwrap_or(Value::Null)),
            Err(err) => json_error(400, &err),
        },
        "/api/structure" => structure_reply(app, query),
        "/api/file-rows" => {
            let relative = query_value(query, "path");
            let page = query_value(query, "page").parse::<usize>().unwrap_or(0);
            match files::tabular_page(&app.root, &relative, page, PAGE_SIZE) {
                Ok(value) => Reply::Json(200, value),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/file" => {
            let relative = query_value(query, "path");
            match files::read_file(&app.root, &relative) {
                Ok(file) => Reply::Json(
                    200,
                    json!({
                        "path": file.path,
                        "text": file.text,
                        "revision": file.revision,
                    }),
                ),
                Err(err) if files::is_missing(&err) => Reply::Json(
                    404,
                    json!({"ok": false, "error": err.to_string(), "missing": true}),
                ),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/presence" => Reply::Json(200, app.presence.snapshot()),
        "/api/query-results" => {
            let relative = query_value(query, "path");
            let key = app.connection_key();
            match query_cache::list(app.results(&key), &relative) {
                Ok(results) => Reply::Json(200, json!({ "results": results })),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/search" => {
            let q = query_value(query, "q");
            let mut state = lock_state(app);
            let results = super::search::search(&mut state.session, &app.root, &q);
            Reply::Json(200, serde_json::to_value(results).unwrap_or(Value::Null))
        }
        "/api/table-cache" => {
            let state = lock_state(app);
            let hint = crate::meta::ls::web_table_cache_status(&state.session);
            Reply::Json(200, serde_json::to_value(hint).unwrap_or(Value::Null))
        }
        "/api/tables" => {
            let mut state = lock_state(app);
            let name = query_value(query, "name");
            let listed = if !name.is_empty() {
                crate::meta::ls::web_table_by_name(&mut state.session, &name)
            } else {
                let pattern = query_value(query, "pattern");
                let refresh = query_flag(query, "refresh");
                let background = query_flag(query, "async");
                crate::meta::ls::web_table_list(&mut state.session, &pattern, refresh, background)
            };
            match listed {
                Ok(listed) => Reply::Json(200, serde_json::to_value(listed).unwrap_or(Value::Null)),
                Err(message) => Reply::Json(502, json!({"ok": false, "error": message})),
            }
        }
        "/api/explain" => {
            let name = query_value(query, "table");
            let dql = query_value(query, "dql");
            let mut state = lock_state(app);
            match table_explain(&mut state, &name, &dql) {
                Ok(payload) => Reply::Json(200, payload),
                Err(message) => Reply::Json(502, json!({"ok": false, "error": message})),
            }
        }
        "/api/rows" => {
            let name = query_value(query, "table");
            let page = query_value(query, "page")
                .parse::<i64>()
                .unwrap_or(0)
                .max(0) as usize;
            let dql = query_value(query, "dql");
            // A fresh read starts uncancelled; /api/stop cancels it like an editor run.
            app.cancel.clear();
            let mut state = lock_state(app);
            match table_rows(&mut state, &name, page, &dql) {
                Ok(payload) => Reply::Json(200, payload),
                Err(message) => Reply::Json(502, json!({"ok": false, "error": message})),
            }
        }
        _ => serve_static(app, path),
    }
}

fn dispatch_mcp(app: &Arc<App>, request: &Incoming) -> Reply {
    if let Some(origin) = &request.origin {
        if !loopback_origin(origin) {
            return Reply::Json(
                403,
                json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32000, "message": "origin is not allowed" }
                }),
            );
        }
    }
    let wants_stream = request.method == "GET"
        && request
            .accept
            .as_deref()
            .is_some_and(|accept| accept.to_ascii_lowercase().contains("text/event-stream"));
    if wants_stream {
        let mcp_id = mcp_client_id(request);
        return Reply::McpEvents(Arc::clone(app), mcp_id);
    }
    if request.method != "POST" {
        return Reply::Json(
            405,
            json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32601, "message": "POST a JSON-RPC message to /mcp" }
            }),
        );
    }
    let text = String::from_utf8_lossy(&request.body);
    let text = text.trim();
    let assigning = request
        .mcp_session
        .as_deref()
        .and_then(presence::valid_mcp_id)
        .is_none()
        && mcp_method(text).as_deref() == Some("initialize");
    let mcp_id = mcp_client_id(request);
    if !assigning && app.presence.touch(&mcp_id, Kind::Mcp) {
        app.hub.presence(&app.presence.snapshot());
    }
    if mcp_method(text).as_deref() == Some("tools/call")
        && mcp_tool_name(text).as_deref() == Some("request_access")
    {
        return request_access(app, &mcp_id, text);
    }
    let read = mcp_read_request(text);
    let response = {
        let mut state = lock_state(app);
        let WebState {
            session,
            mcp_initialized,
            ..
        } = &mut *state;
        crate::mcp::exchange(mcp_initialized, session, &app.root, text)
    };
    match response {
        Some(response) => {
            let value = serde_json::from_str(&response).unwrap_or_else(|_| {
                json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32603, "message": "bad response" }
                })
            });
            if let Some((table, offset)) = read {
                if value["result"]["isError"] != true {
                    let rows = value["result"]["structuredContent"]["rows"]
                        .as_array()
                        .map(|rows| rows.len())
                        .unwrap_or(0);
                    app.hub.mcp_read(&table, rows, offset);
                }
            }
            let status = if value["error"]["code"] == -32700 {
                400
            } else {
                200
            };
            if assigning && value.get("result").is_some() {
                let session_id = new_mcp_session();
                if app.presence.touch(&session_id, Kind::Mcp) {
                    app.hub.presence(&app.presence.snapshot());
                }
                Reply::McpJson(status, value, Some(session_id))
            } else {
                Reply::Json(status, value)
            }
        }
        None => Reply::Bytes(202, "application/json", Vec::new()),
    }
}

fn mcp_client_id(request: &Incoming) -> String {
    request
        .mcp_session
        .as_deref()
        .and_then(presence::valid_mcp_id)
        .unwrap_or("mcp")
        .to_string()
}

fn mcp_method(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn mcp_tool_name(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .pointer("/params/name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn new_mcp_session() -> String {
    let mut bytes = [0u8; 8];
    let _ = getrandom::getrandom(&mut bytes);
    let mut id = String::from("mcp");
    for byte in bytes {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

/// `tools/call` of `read_granted_table`: the table name and the requested offset.
fn mcp_read_request(text: &str) -> Option<(String, usize)> {
    let value: Value = serde_json::from_str(text).ok()?;
    if value.get("method").and_then(Value::as_str) != Some("tools/call") {
        return None;
    }
    let params = value.get("params")?;
    if params.get("name").and_then(Value::as_str) != Some("read_granted_table") {
        return None;
    }
    let arguments = params.get("arguments")?;
    let table = arguments.get("table").and_then(Value::as_str)?.to_string();
    let offset = arguments.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    Some((table, offset))
}

fn request_access(app: &App, agent: &str, text: &str) -> Reply {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => {
            return Reply::Json(
                400,
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}),
            );
        }
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    {
        let state = lock_state(app);
        if !state.mcp_initialized {
            return Reply::Json(
                200,
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32002, "message": "server not initialized"}}),
            );
        }
    }
    let arguments = value
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let request_id = arguments
        .get("request")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let result = if request_id.is_empty() {
        let query = arguments.get("query").and_then(Value::as_str).unwrap_or("");
        match app.access.open(query, agent) {
            Ok(new_id) => {
                publish_requests(app);
                app.access.wait(&new_id, super::access::WAIT)
            }
            Err(message) => Err(message),
        }
    } else {
        app.access.wait(request_id, super::access::WAIT)
    };
    let body = match result {
        Ok(answer) => crate::mcp::tool_ok(live_answer(app, answer)),
        Err(message) => crate::mcp::tool_err(message),
    };
    Reply::Json(200, json!({"jsonrpc": "2.0", "id": id, "result": body}))
}

/// A grant that a later revoke or drop ended is no longer readable.
fn live_answer(app: &App, mut answer: Value) -> Value {
    if answer.get("status").and_then(Value::as_str) != Some("granted") {
        return answer;
    }
    let table = answer
        .get("table")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let live = {
        let state = lock_state(app);
        state.session.engine.temp_table_granted(&table)
    };
    if !live {
        answer["status"] = json!("revoked");
    }
    answer
}

fn publish_requests(app: &App) {
    let requests = app
        .access
        .list(|id| agent_connected(app, id), |id| agent_label(app, id));
    app.hub.grant_request(&requests);
}

fn agent_connected(app: &App, id: &str) -> bool {
    app.presence.snapshot()["clients"]
        .as_array()
        .is_some_and(|clients| clients.iter().any(|client| client["id"] == id))
}

fn agent_label(app: &App, id: &str) -> String {
    app.presence.snapshot()["clients"]
        .as_array()
        .and_then(|clients| {
            clients
                .iter()
                .find(|client| client["id"] == id)
                .and_then(|client| client["label"].as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "M".to_string())
}

fn execute_watched(app: &App, state: &mut WebState, statement: &str) -> ServeEnvelope {
    let before = state.session.engine.granted_tables();
    let envelope = state.session.execute_for_serve(statement);
    if envelope.ok {
        let after = state.session.engine.granted_tables();
        if before
            .iter()
            .map(|info| info.name.as_str())
            .collect::<Vec<_>>()
            != after
                .iter()
                .map(|info| info.name.as_str())
                .collect::<Vec<_>>()
        {
            state.table_catalog = None;
        }
        publish_grant_changes(app, &before, &after);
        let mut requests_changed = false;
        for info in &after {
            if !before.iter().any(|prior| prior.name == info.name) {
                requests_changed |= app.access.note_granted(&info.name);
            }
        }
        requests_changed |= app
            .access
            .note_missing(|name| state.session.engine.is_temp_table(name));
        if requests_changed {
            publish_requests(app);
        }
    }
    envelope
}

fn publish_grant_changes(
    app: &App,
    before: &[dql_engine::GrantInfo],
    after: &[dql_engine::GrantInfo],
) {
    let now = dql_engine::rfc3339_now();
    let mut changed = false;
    for info in after {
        if before.iter().any(|prior| prior.name == info.name) {
            continue;
        }
        changed = true;
        let request = app.access.request_for_table(&info.name);
        app.mcp_hub.publish(&super::mcp_stream::granted(
            &info.name,
            request.as_deref(),
            &info.granted_at,
        ));
    }
    for info in before {
        if after.iter().any(|next| next.name == info.name) {
            continue;
        }
        changed = true;
        let request = app.access.request_for_table(&info.name);
        app.mcp_hub.publish(&super::mcp_stream::revoked(
            &info.name,
            request.as_deref(),
            &now,
        ));
    }
    if changed {
        app.hub.grants();
    }
}

fn loopback_origin(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once(':').map(|(host, _)| host).unwrap_or(host);
    let host = host.trim_matches(['[', ']']);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn dispatch_post(app: &Arc<App>, path: &str, body: &[u8]) -> Reply {
    if path == "/api/stop" {
        app.cancel.request();
        return Reply::Json(200, json!({"ok": true}));
    }
    if path == "/api/mcp" {
        return mcp_reply(app, &parse_json(body));
    }
    if path == "/api/mcp-access/run" {
        return run_access(app, &parse_json(body));
    }
    if path == "/api/mcp-access/grant" {
        return grant_access(app, &parse_json(body));
    }
    if path == "/api/mcp-access/deny" {
        return deny_access(app, &parse_json(body));
    }
    if path == "/api/run" {
        let parsed = parse_json(body);
        let dql = parsed
            .get("dql")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let script = parsed.get("path").and_then(Value::as_str).unwrap_or("");
        return handle_run(Arc::clone(app), dql, script);
    }
    let body = parse_json(body);
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    match path {
        "/api/presence" => {
            let id = body.get("id").and_then(Value::as_str).unwrap_or("");
            let Some(id) = presence::valid_web_id(id) else {
                return Reply::Json(400, json!({"ok": false, "error": "client id is invalid"}));
            };
            if app.presence.touch(id, Kind::Web) {
                app.hub.presence(&app.presence.snapshot());
            }
            Reply::Json(200, app.presence.snapshot())
        }
        "/api/file" => match files::create_file(&app.root, relative) {
            Ok(path) => {
                app.hub.file(&path, &files::content_revision(""));
                app.hub.tree();
                Reply::Json(200, json!({"path": path, "text": ""}))
            }
            Err(err) => json_error(400, &err),
        },
        "/api/mkdir" => match files::make_dir(&app.root, relative) {
            Ok(path) => {
                app.hub.tree();
                Reply::Json(200, json!({"path": path, "kind": "dir"}))
            }
            Err(err) => json_error(400, &err),
        },
        "/api/duplicate" => match files::duplicate_path(&app.root, relative) {
            Ok((path, kind)) => {
                app.hub.tree();
                Reply::Json(200, json!({"path": path, "kind": kind}))
            }
            Err(err) => json_error(400, &err),
        },
        "/api/move" => {
            let from = body.get("from").and_then(Value::as_str).unwrap_or("");
            let to = body.get("to").and_then(Value::as_str).unwrap_or("");
            match files::move_path(&app.root, from, to) {
                Ok((path, kind)) => {
                    app.hub.tree();
                    Reply::Json(200, json!({"path": path, "kind": kind}))
                }
                Err(err) => json_error(400, &err),
            }
        }
        "/api/delete" => match files::delete_path(&app.root, relative) {
            Ok(()) => {
                app.hub.file(relative, "missing");
                app.hub.tree();
                Reply::Json(200, json!({"ok": true}))
            }
            Err(err) => json_error(400, &err),
        },
        "/api/query-results" => save_query_result(app.results(&app.connection_key()), &body),
        "/api/query-results/clear" => {
            match query_cache::clear_at(app.results(&app.connection_key()), relative) {
                Ok(paths) => Reply::Json(200, json!({"ok": true, "paths": paths})),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/lint" => {
            let text = body.get("text").and_then(Value::as_str).unwrap_or("");
            let mut state = lock_state(app);
            let report = super::lint::lint_script(&mut state.session, text);
            Reply::Json(200, serde_json::to_value(report).unwrap_or(Value::Null))
        }
        "/api/local-tables/clear" => {
            let mut state = lock_state(app);
            match state.session.engine.clear_local_tables() {
                Ok(names) => {
                    state.table_catalog = None;
                    for name in &names {
                        state.shapes.remove(name);
                    }
                    drop(state);
                    app.hub.tables(&names);
                    Reply::Json(200, json!({"ok": true, "tables": names}))
                }
                Err(err) => Reply::Json(500, json!({"ok": false, "error": err.to_string()})),
            }
        }
        "/api/result-cache/clear" => {
            let key = app.connection_key();
            match query_cache::clear_all(app.results(&key)) {
                Ok(files) => Reply::Json(200, json!({"ok": true, "files": files})),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/table-cache/clear" => {
            let mut state = lock_state(app);
            state.shapes.clear();
            state.table_catalog = None;
            state.session.clear_table_cache();
            Reply::Json(200, json!({"ok": true}))
        }
        _ => Reply::Json(404, json!({"ok": false, "error": "not found"})),
    }
}

fn dispatch_put(app: &App, path: &str, raw: &[u8]) -> Reply {
    if path != "/api/file" {
        return Reply::Json(404, json!({"ok": false, "error": "not found"}));
    }
    let body = parse_json(raw);
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    let text = body.get("text").and_then(Value::as_str).unwrap_or("");
    let base = body.get("base").and_then(Value::as_str);
    match files::save_file(&app.root, relative, text, base) {
        Ok(files::SaveFile::Stored(file)) => {
            app.hub.file(&file.path, &file.revision);
            Reply::Json(
                200,
                json!({"ok": true, "path": file.path, "revision": file.revision}),
            )
        }
        Ok(files::SaveFile::Conflict(file)) => Reply::Json(
            409,
            json!({
                "ok": false,
                "error": "file changed on disk",
                "conflict": true,
                "path": relative,
                "text": file.text,
                "revision": file.revision,
                "missing": file.missing,
            }),
        ),
        Err(err) => json_error(400, &err),
    }
}

fn save_query_result(store: query_cache::Store<'_>, body: &Value) -> Reply {
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    let query = body.get("query").and_then(Value::as_str).unwrap_or("");
    let Some(success) = body.get("success").and_then(Value::as_bool) else {
        return Reply::Json(
            400,
            json!({"ok": false, "error": "success must be a boolean"}),
        );
    };
    let result = body.get("result").cloned().unwrap_or(Value::Null);
    match query_cache::save(store, relative, query, result, success) {
        Ok(stored) => Reply::Json(200, serde_json::to_value(stored).unwrap_or(Value::Null)),
        Err(err) => json_error(400, &err),
    }
}

fn mcp_access_payload(app: &App) -> Value {
    let requests = app
        .access
        .list(|id| agent_connected(app, id), |id| agent_label(app, id));
    let grants = {
        let state = lock_state(app);
        state
            .session
            .engine
            .granted_tables()
            .into_iter()
            .map(|info| {
                json!({
                    "table": info.name,
                    "access": "MCP read",
                    "granted": info.granted_at,
                })
            })
            .collect::<Vec<_>>()
    };
    json!({ "requests": requests, "grants": grants })
}

fn envelope_error(envelope: &ServeEnvelope) -> String {
    envelope
        .error
        .as_ref()
        .map(|error| error.message.clone())
        .or_else(|| envelope.message.clone())
        .unwrap_or_else(|| "run failed".to_string())
}

fn run_access(app: &Arc<App>, body: &Value) -> Reply {
    let id = body
        .get("request")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return Reply::Json(400, json!({"ok": false, "error": "missing request"}));
    }
    app.cancel.clear();
    let drop_first = match app.access.can_run(id) {
        Ok(drop_first) => drop_first,
        Err(message) => return Reply::Json(409, json!({"ok": false, "error": message})),
    };
    let query = match app.access.query(id) {
        Ok(query) => query,
        Err(message) => return Reply::Json(404, json!({"ok": false, "error": message})),
    };
    let table = super::access::Store::table_name(id);
    let mut state = lock_state(app);
    if !drop_first && state.session.engine.is_temp_table(&table) {
        return Reply::Json(
            409,
            json!({"ok": false, "error": format!("Table '{table}' already exists")}),
        );
    }
    if drop_first {
        let drop = format!("DROP TEMPTABLE IF EXISTS {table}");
        let envelope = execute_watched(app, &mut state, &drop);
        if !envelope.ok {
            return Reply::Json(
                502,
                json!({"ok": false, "error": envelope_error(&envelope)}),
            );
        }
    }
    let create = format!("CREATE TEMPTABLE {table} AS ({query})");
    let envelope = execute_watched(app, &mut state, &create);
    if !envelope.ok {
        return Reply::Json(
            502,
            json!({"ok": false, "error": envelope_error(&envelope)}),
        );
    }
    let (page, total) = state
        .session
        .engine
        .temp_table_page(&table, 0, 1000)
        .unwrap_or_else(|| (Vec::new(), 0));
    let found = super::classify::classify(&page, total);
    let rows = page
        .iter()
        .take(20)
        .map(dql_engine::json_util::item_to_json_value)
        .collect::<Vec<_>>();
    let classification = super::classify::to_json(&found);
    if let Err(message) = app.access.ran(id, classification, json!(rows), total) {
        return Reply::Json(409, json!({"ok": false, "error": message}));
    }
    drop(state);
    publish_requests(app);
    Reply::Json(200, mcp_access_payload(app))
}

fn grant_access(app: &Arc<App>, body: &Value) -> Reply {
    let id = body
        .get("request")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let table = match app.access.grant(id) {
        Ok(table) => table,
        Err(message) => return Reply::Json(409, json!({"ok": false, "error": message})),
    };
    let statement = format!("GRANT MCP READ ON TEMPTABLE {table}");
    let mut state = lock_state(app);
    let envelope = execute_watched(app, &mut state, &statement);
    if !envelope.ok {
        return Reply::Json(
            502,
            json!({"ok": false, "error": envelope_error(&envelope)}),
        );
    }
    app.access.finish_grant(id);
    drop(state);
    publish_requests(app);
    Reply::Json(200, mcp_access_payload(app))
}

fn deny_access(app: &Arc<App>, body: &Value) -> Reply {
    let id = body
        .get("request")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let table = match app.access.deny(id) {
        Ok(table) => table,
        Err(message) => return Reply::Json(409, json!({"ok": false, "error": message})),
    };
    if let Some(table) = table {
        let statement = format!("DROP TEMPTABLE IF EXISTS {table}");
        let mut state = lock_state(app);
        let _ = execute_watched(app, &mut state, &statement);
    }
    app.mcp_hub
        .publish(&super::mcp_stream::denied(id, &dql_engine::rfc3339_now()));
    publish_requests(app);
    Reply::Json(200, mcp_access_payload(app))
}

fn handle_run(app: Arc<App>, dql: String, script: &str) -> Reply {
    let statements = split_statements(&dql);
    if statements.is_empty() {
        return Reply::Json(502, json!({"ok": false, "error": "nothing to run"}));
    }
    let file_base = match run_file_base(&app.root, script) {
        Ok(file_base) => file_base,
        Err(err) => return Reply::Json(400, json!({"ok": false, "error": err.to_string()})),
    };
    Reply::Ndjson(app, statements, file_base)
}

/// SAVE, LOAD, and local-table paths in a run: `./` is relative to the folder of the
/// `.dql` file (`script`, workspace-relative), and other relative paths to the workspace.
/// A run without a file (an empty `script`) resolves `./` against the workspace.
fn run_file_base(root: &Path, script: &str) -> Result<dql_engine::FileBase, PathError> {
    let workspace = files::resolve_inside(root, "")?;
    let script = script.trim();
    let script_dir = if script.is_empty() {
        None
    } else {
        let file = files::resolve_inside(root, script)?;
        file.parent().map(Path::to_path_buf)
    };
    Ok(dql_engine::FileBase::new(Some(workspace), script_dir))
}

fn run_script(
    app: Arc<App>,
    statements: Vec<String>,
    file_base: dql_engine::FileBase,
    writer: impl Write + Send + 'static,
) {
    app.cancel.clear();
    let writer = Arc::new(Mutex::new(writer));
    let mut state = lock_state(&app);
    state.session.engine.set_file_base(file_base);
    for (index, statement) in statements.iter().enumerate() {
        if write_json(
            &mut *lock_writer(&writer),
            &json!({"event": "statement", "index": index}),
        )
        .is_err()
        {
            return;
        }
        if app.cancel.is_requested() {
            let envelope = envelope_from_error(dql_engine::EngineError::Cancelled);
            let result = public_result(statement, &envelope, None, 0, 0.0, 0.0);
            let _ = write_json(
                &mut *lock_writer(&writer),
                &json!({"event": "result", "index": index, "result": result}),
            );
            return;
        }
        let progress = Arc::clone(&writer);
        state.session.progress().set(Some(Box::new(move |event| {
            let mut writer = lock_writer(&progress);
            let _ = write_json(
                &mut *writer,
                &json!({
                    "event": "progress",
                    "index": index,
                    "done": event.done,
                    "total": event.total,
                    "phase": event.phase,
                }),
            );
        })));
        let started = std::time::Instant::now();
        let envelope = execute_watched(app.as_ref(), &mut state, statement);
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (read_units, write_units) = state.session.engine.statement_capacity();
        state.session.progress().clear();
        let columns = if envelope.ok && envelope.kind == "items" {
            Some(columns_for(
                &mut state,
                statement,
                envelope.items.as_deref().unwrap_or(&[]),
            ))
        } else {
            None
        };
        if envelope.ok {
            let names = changed_tables(statement);
            if !names.is_empty() {
                for name in &names {
                    state.shapes.remove(name);
                }
                state.table_catalog = None;
                app.hub.tables(&names);
            }
        }
        let mut result = public_result(
            statement,
            &envelope,
            columns.clone(),
            elapsed_ms,
            read_units,
            write_units,
        );
        if envelope.ok && envelope.kind == "items" {
            let id = state.remember_result(
                statement,
                envelope.items.clone().unwrap_or_default(),
                columns.unwrap_or_default(),
            );
            result["id"] = json!(id);
        }
        let ok = envelope.ok;
        if write_json(
            &mut *lock_writer(&writer),
            &json!({"event": "result", "index": index, "result": result}),
        )
        .is_err()
            || !ok
        {
            return;
        }
    }
}

fn columns_for(state: &mut WebState, statement: &str, items: &[Value]) -> Vec<Column> {
    let (table, selection) = read_query(statement);
    let Some(table) = table else {
        return column_order(items, None, None, None);
    };
    let explained = state
        .session
        .execute_for_serve(&format!("EXPLAIN {statement}"));
    let index = if explained.ok {
        explain_index(explained.message.as_deref().unwrap_or(""))
    } else {
        None
    };
    let mut shape = load_shape(state, &table);
    if let Some(name) = index.as_deref() {
        let missing = shape
            .as_ref()
            .is_some_and(|shape| !shape.indexes.contains_key(name));
        if missing {
            state.shapes.remove(&table);
            shape = load_shape(state, &table);
        }
    }
    column_order(
        items,
        shape.as_ref(),
        index.as_deref(),
        selection.as_deref(),
    )
}

fn load_shape(state: &mut WebState, name: &str) -> Option<TableShape> {
    if let Some(shape) = state.shapes.get(name) {
        return Some(shape.clone());
    }
    if !safe_table_name(name) {
        return None;
    }
    let detail = ExecText::from_envelope(&state.session.execute_for_serve(&format!("ls {name}")));
    if !detail.ok {
        return None;
    }
    let shape = parse_table_shape(&detail.message)?;
    state.shapes.insert(name.to_string(), shape.clone());
    Some(shape)
}

fn table_rows(state: &mut WebState, name: &str, page: usize, dql: &str) -> Result<Value, String> {
    if !safe_table_name(name) {
        return Err(format!("unsupported table name {name:?}"));
    }
    let start = PAGE_SIZE.saturating_mul(page);
    let fetch_limit = start.saturating_add(PAGE_SIZE).saturating_add(1);
    let (statements, _) = browser_read_dql(name, dql, fetch_limit)?;
    let mut items = Vec::new();
    let started = std::time::Instant::now();
    let (mut read_units, mut write_units) = (0.0, 0.0);
    for statement in &statements {
        let envelope = state.session.execute_for_serve(statement);
        if !envelope.ok {
            return Err(ExecText::from_envelope(&envelope).failure());
        }
        let (read, write) = state.session.engine.statement_capacity();
        read_units += read;
        write_units += write;
        items.extend(envelope.items.unwrap_or_default());
    }
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let statement = statements.join("; ");
    let end = (start + PAGE_SIZE).min(items.len());
    let window = items.get(start..end).unwrap_or(&[]);
    let shape = load_shape(state, name);
    let columns = column_order(window, shape.as_ref(), None, None);
    state.fetched_pages.insert(
        name.to_string(),
        FetchedPage {
            items: window.to_vec(),
            columns: columns.clone(),
            shape,
        },
    );
    Ok(json!({
        "name": name,
        "page": page,
        "page_size": PAGE_SIZE,
        "items": window,
        "columns": columns,
        "has_more": items.len() > end,
        "dql": statement,
        "elapsed_ms": elapsed_ms,
        "read_units": read_units,
        "write_units": write_units,
    }))
}

/// EXPLAIN for the table browser: the plan of each statement its first page would run, with the
/// same LIMIT as `/api/rows`, so the plan matches what Run reads.
fn table_explain(state: &mut WebState, name: &str, dql: &str) -> Result<Value, String> {
    if !safe_table_name(name) {
        return Err(format!("unsupported table name {name:?}"));
    }
    let (statements, _) = browser_read_dql(name, dql, PAGE_SIZE.saturating_add(1))?;
    let mut plans = Vec::new();
    for statement in &statements {
        let envelope = state
            .session
            .execute_for_serve(&format!("EXPLAIN {statement}"));
        if !envelope.ok {
            return Err(ExecText::from_envelope(&envelope).failure());
        }
        plans.push(json!({
            "statement": statement,
            "plan": envelope.message.unwrap_or_default(),
        }));
    }
    Ok(json!({"name": name, "plans": plans}))
}

/// SCAN/SELECT of `name` only. Paged reads append LIMIT; KEYS IN and multi-query keys cannot.
fn browser_read_dql(name: &str, dql: &str, limit: usize) -> Result<(Vec<String>, bool), String> {
    let text = dql.trim().trim_end_matches(';').trim();
    if text.is_empty() {
        return Ok((vec![format!("SCAN * FROM {name} LIMIT {limit}")], true));
    }
    let parts = split_statements(text);
    if parts.is_empty() {
        return Err("only SELECT or SCAN in the table browser".to_string());
    }
    let multi = parts.len() > 1;
    let mut statements = Vec::new();
    let mut keys_in = false;
    for part in parts {
        let part = part.trim().trim_end_matches(';').trim().to_string();
        let statement = parse_statement(&part).map_err(|err| err.to_string())?;
        match &statement {
            Statement::Select { table, options, .. } | Statement::Scan { table, options, .. } => {
                if table != name {
                    return Err(format!("read must use table {name}"));
                }
                if options.save_file.is_some() {
                    return Err("SAVE is not allowed in the table browser".to_string());
                }
                if options.limit.is_some() || options.scan_limit.is_some() {
                    return Err("LIMIT is set by the table browser".to_string());
                }
                if options.keys_in.is_some() {
                    keys_in = true;
                    statements.push(part);
                } else if multi {
                    statements.push(part);
                } else {
                    statements.push(format!("{part} LIMIT {limit}"));
                }
            }
            _ => return Err("only SELECT or SCAN in the table browser".to_string()),
        }
    }
    Ok((statements, !(keys_in || multi)))
}

fn structure_reply(app: &App, query: &str) -> Reply {
    let result = query_value(query, "result");
    let table = query_value(query, "table");
    let path = query_value(query, "path");
    let chosen = [&result, &table, &path]
        .iter()
        .filter(|value| !value.is_empty())
        .count();
    if chosen != 1 {
        return json_error(
            400,
            &PathError("pass one query result, table, or data file".to_string()),
        );
    }
    if !path.is_empty() {
        return match file_structure(&app.root, &path) {
            Ok(value) => Reply::Json(200, value),
            Err(err) => json_error(400, &err),
        };
    }
    let mut state = lock_state(app);
    if !table.is_empty() {
        return match table_structure(&mut state, &table) {
            Ok(value) => Reply::Json(200, value),
            Err(message) => Reply::Json(400, json!({"ok": false, "error": message})),
        };
    }
    let key = state.session.connection_key.clone();
    match result_structure(app.results(&key), &mut state, &result) {
        Ok(value) => Reply::Json(200, value),
        Err(err) => json_error(400, &err),
    }
}

fn file_structure(root: &Path, relative: &str) -> Result<Value, PathError> {
    let (name, columns, items) = files::tabular_all(root, relative)?;
    let described = structure::describe(&columns, &items);
    Ok(structure::payload(
        "file",
        &name,
        "",
        items.len(),
        false,
        described,
    ))
}

fn table_structure(state: &mut WebState, name: &str) -> Result<Value, String> {
    let (columns, items) = {
        let page = state
            .fetched_pages
            .get(name)
            .ok_or_else(|| "table page is not loaded".to_string())?;
        (
            columns_with_shape_keys(&page.columns, page.shape.as_ref()),
            page.items.clone(),
        )
    };
    let described = structure::describe(&columns, &items);
    let mut value = structure::payload("table", name, "", items.len(), true, described);
    let (create_query, definition, source) = cached_table_schema(&mut state.session, name);
    let Some(object) = value.as_object_mut() else {
        return Ok(value);
    };
    object.insert("createQuery".to_string(), json!(create_query));
    object.insert("definition".to_string(), definition);
    object.insert("definitionSource".to_string(), json!(source));
    Ok(value)
}

/// Create query, attribute rows, and where they came from, without a database call.
///
/// `cached_definition` reads a local or temp table's in-memory definition, or a DynamoDB
/// table's snapshot in memory or in the saved cache. It does not describe the table, so a
/// structure view never refreshes the database. Creating a table stores the snapshot, and
/// describing the table replaces it with the current definition.
///
/// The source is `local` or `temp` for those tables, whose create query is their own
/// `CREATE LOCALTABLE` or `CREATE TEMPTABLE` text, and `cache` for a DynamoDB table.
fn cached_table_schema(
    session: &mut crate::session::Session,
    name: &str,
) -> (String, Value, &'static str) {
    let virtual_line = session.engine.virtual_schema_line(name);
    let source = virtual_line.as_ref().map_or("cache", |(kind, _)| *kind);
    let Some(meta) = session.engine.cached_definition(name) else {
        return (String::new(), json!([]), source);
    };
    if !meta.has_definition() {
        return (String::new(), json!([]), source);
    }
    let create_query = match virtual_line {
        Some((_, line)) => line,
        None => meta.schema_dql_formatted(),
    };
    (create_query, json!(definition_rows(&meta)), source)
}

fn definition_rows(meta: &dql_models::TableMeta) -> Vec<Value> {
    let mut names = Vec::new();
    if !meta.hash_key.is_empty() {
        names.push(meta.hash_key.clone());
    }
    if let Some(range) = &meta.range_key {
        if !names.iter().any(|name| name == range) {
            names.push(range.clone());
        }
    }
    let mut rest: Vec<String> = meta
        .attrs
        .keys()
        .filter(|name| !names.iter().any(|kept| kept == *name))
        .cloned()
        .collect();
    rest.sort();
    names.extend(rest);
    names
        .into_iter()
        .filter_map(|name| {
            let field = meta.attrs.get(&name)?;
            let role = if name == meta.hash_key {
                "hash"
            } else if meta.range_key.as_deref() == Some(name.as_str()) {
                "range"
            } else if meta
                .global_indexes
                .values()
                .any(|index| index.hash_key.name == name)
            {
                "index-hash"
            } else if meta.global_indexes.values().any(|index| {
                index.range_key.as_ref().map(|field| field.name.as_str()) == Some(name.as_str())
            }) || meta
                .local_indexes
                .values()
                .any(|index| index.range_key.as_deref() == Some(name.as_str()))
            {
                "index-range"
            } else {
                ""
            };
            let mut indexes = Vec::new();
            for index in meta.local_indexes.values() {
                if index.range_key.as_deref() == Some(name.as_str()) {
                    indexes.push(index.name.clone());
                }
            }
            for index in meta.global_indexes.values() {
                let on_hash = index.hash_key.name == name;
                let on_range = index.range_key.as_ref().map(|field| field.name.as_str())
                    == Some(name.as_str());
                if on_hash || on_range {
                    indexes.push(index.name.clone());
                }
            }
            indexes.sort();
            indexes.dedup();
            Some(json!({
                "name": name,
                "type": attribute_type_label(&field.data_type),
                "role": role,
                "index": indexes.join(", "),
            }))
        })
        .collect()
}

fn attribute_type_label(data_type: &dql_parser::AttributeType) -> String {
    match data_type {
        dql_parser::AttributeType::String => "STRING".to_string(),
        dql_parser::AttributeType::Number => "NUMBER".to_string(),
        dql_parser::AttributeType::Binary => "BINARY".to_string(),
        dql_parser::AttributeType::Bool => "BOOL".to_string(),
        dql_parser::AttributeType::Other(value) => value.clone(),
    }
}

fn columns_with_shape_keys(columns: &[Column], shape: Option<&TableShape>) -> Vec<Column> {
    let mut columns = columns.to_vec();
    let Some(shape) = shape else {
        return columns;
    };
    let mut add = |name: &str, role: &str| {
        if name.is_empty() || columns.iter().any(|column| column.name == name) {
            return;
        }
        columns.push(Column {
            name: name.to_string(),
            table: Some(role.to_string()),
            index: None,
        });
    };
    add(&shape.hash_key, "hash");
    if let Some(range) = &shape.range_key {
        add(range, "range");
    }
    columns
}

fn result_structure(
    store: query_cache::Store<'_>,
    state: &mut WebState,
    id: &str,
) -> Result<Value, PathError> {
    if let Some((statement, items, columns)) = state.held_result(id) {
        return Ok(result_payload(&statement, &columns, &items));
    }
    let listed = query_cache::load_by_id(store, id)?;
    let items = listed
        .result
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let columns = listed
        .result
        .get("columns")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<Column>>(value).ok())
        .unwrap_or_default();
    Ok(result_payload(&listed.query, &columns, &items))
}

fn result_payload(statement: &str, columns: &[Column], items: &[Value]) -> Value {
    let described = structure::describe(columns, items);
    structure::payload(
        "result",
        &structure::result_title(statement),
        statement,
        items.len(),
        false,
        described,
    )
}

impl WebState {
    fn remember_result(
        &mut self,
        statement: &str,
        items: Vec<Value>,
        columns: Vec<Column>,
    ) -> String {
        self.next_result = self.next_result.saturating_add(1);
        let id = format!("run-{}", self.next_result);
        if self.next_result > 50 {
            self.held_results
                .remove(&format!("run-{}", self.next_result - 50));
        }
        self.held_results.insert(
            id.clone(),
            HeldResult {
                statement: statement.to_string(),
                items,
                columns,
            },
        );
        id
    }

    fn held_result(&self, id: &str) -> Option<(String, Vec<Value>, Vec<Column>)> {
        self.held_results.get(id).map(|held| {
            (
                held.statement.clone(),
                held.items.clone(),
                held.columns.clone(),
            )
        })
    }
}

fn serve_static(app: &App, path: &str) -> Reply {
    let Some(relative) = static_relative(path) else {
        return Reply::Json(404, json!({"ok": false, "error": "not found"}));
    };
    match load_static(&relative) {
        Some((bytes, content_type)) => {
            let bytes = if relative == "index.html" {
                inject_page_token(&bytes, &app.token)
            } else {
                bytes
            };
            Reply::Bytes(200, content_type, bytes)
        }
        None => Reply::Json(404, json!({"ok": false, "error": "not found"})),
    }
}

fn inject_page_token(html: &[u8], token: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(html) else {
        return html.to_vec();
    };
    let tag = format!(r#"<meta name="dqlrs-token" content="{token}">"#);
    let Some(index) = text.find("</head>") else {
        return (tag + text).into_bytes();
    };
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..index]);
    out.push_str(&tag);
    out.push_str(&text[index..]);
    out.into_bytes()
}

fn load_static(relative: &str) -> Option<(Vec<u8>, &'static str)> {
    if let Some(root) = disk_static() {
        let file = root.join(relative);
        let canon = file.canonicalize().ok()?;
        if !canon.starts_with(&root) || !canon.is_file() {
            return None;
        }
        let bytes = fs::read(canon).ok()?;
        return Some((bytes, content_type(relative)));
    }
    let file = ASSETS.get_file(relative)?;
    Some((file.contents().to_vec(), content_type(relative)))
}

fn disk_static() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DQLRS_WEB_STATIC") {
        let path = PathBuf::from(path);
        if path.join("index.html").is_file() {
            return path.canonicalize().ok();
        }
    }
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../dqlrs-web/static");
    if checkout.join("index.html").is_file() {
        return checkout.canonicalize().ok();
    }
    None
}

fn static_relative(path: &str) -> Option<String> {
    let relative = if path.is_empty() || path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').replace('\\', "/")
    };
    if relative
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(relative)
}

fn content_type(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn parse_json(body: &[u8]) -> Value {
    if body.is_empty() || body.len() > BODY_LIMIT {
        return Value::Object(Default::default());
    }
    match serde_json::from_slice(body) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Default::default()),
    }
}

fn json_error(status: u16, err: &PathError) -> Reply {
    Reply::Json(status, json!({"ok": false, "error": err.to_string()}))
}

fn write_reply(stream: &mut TcpStream, reply: Reply, keep_alive: bool) -> io::Result<bool> {
    match reply {
        Reply::Json(status, value) => {
            let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
            write_bytes(
                stream,
                status,
                "application/json; charset=utf-8",
                &bytes,
                keep_alive,
                None,
            )?;
            Ok(keep_alive)
        }
        Reply::McpJson(status, value, session) => {
            let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
            write_bytes(
                stream,
                status,
                "application/json; charset=utf-8",
                &bytes,
                keep_alive,
                session.as_deref(),
            )?;
            Ok(keep_alive)
        }
        Reply::Bytes(status, content_type, bytes) => {
            write_bytes(stream, status, content_type, &bytes, keep_alive, None)?;
            Ok(keep_alive)
        }
        Reply::Ndjson(app, statements, file_base) => {
            write_ndjson(stream, app, statements, file_base)?;
            Ok(false)
        }
        Reply::Events(app, client_id) => {
            write_events(stream, app, &client_id)?;
            Ok(false)
        }
        Reply::McpEvents(app, client_id) => {
            write_mcp_events(stream, app, &client_id)?;
            Ok(false)
        }
    }
}

fn write_events(stream: &mut TcpStream, app: Arc<App>, client_id: &str) -> io::Result<()> {
    let header = "\
HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream; charset=utf-8\r\n\
Cache-Control: no-store\r\n\
Transfer-Encoding: chunked\r\n\
Connection: close\r\n\
\r\n";
    stream.write_all(header.as_bytes())?;
    stream.flush()?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    let (sub, rx) = app.hub.subscribe();
    let generation = app.presence.hold(client_id, Kind::Web);
    app.hub.presence(&app.presence.snapshot());
    let leave = EndStream {
        app: Arc::clone(&app),
        client_id: client_id.to_string(),
        generation,
        sub,
    };
    let result = stream_events(stream, &app, rx);
    drop(leave);
    result
}

struct EndStream {
    app: Arc<App>,
    client_id: String,
    generation: u64,
    sub: u64,
}

impl Drop for EndStream {
    fn drop(&mut self) {
        self.app.hub.unsubscribe(self.sub);
        if self
            .app
            .presence
            .release(&self.client_id, Kind::Web, self.generation)
        {
            self.app.hub.presence(&self.app.presence.snapshot());
        }
    }
}

fn write_mcp_events(stream: &mut TcpStream, app: Arc<App>, client_id: &str) -> io::Result<()> {
    let header = "\
HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream; charset=utf-8\r\n\
Cache-Control: no-store\r\n\
Transfer-Encoding: chunked\r\n\
Connection: close\r\n\
\r\n";
    stream.write_all(header.as_bytes())?;
    stream.flush()?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    if app.presence.touch(client_id, Kind::Mcp) {
        app.hub.presence(&app.presence.snapshot());
    }
    let current = {
        let state = lock_state(&app);
        state
            .session
            .engine
            .granted_tables()
            .into_iter()
            .map(|info| {
                let request = app.access.request_for_table(&info.name);
                super::mcp_stream::granted(&info.name, request.as_deref(), &info.granted_at)
            })
            .collect::<Vec<_>>()
    };
    let rx = app.mcp_hub.subscribe(&current);
    stream_events(stream, &app, rx)
}

fn stream_events(
    stream: &mut TcpStream,
    app: &App,
    rx: std::sync::mpsc::Receiver<String>,
) -> io::Result<()> {
    const PING: Duration = Duration::from_secs(15);
    // A closed tab is noticed on the next check, not the next failed write.
    const CHECK: Duration = Duration::from_secs(1);
    write_chunk(stream, ": ping\n\n")?;
    let mut last_ping = std::time::Instant::now();
    loop {
        match rx.recv_timeout(CHECK) {
            Ok(frame) => write_chunk(stream, &frame)?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if peer_closed(stream) {
                    break;
                }
                if last_ping.elapsed() >= PING {
                    write_chunk(stream, ": ping\n\n")?;
                    last_ping = std::time::Instant::now();
                    if app.presence.sweep() {
                        app.hub.presence(&app.presence.snapshot());
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

/// Whether the client has closed its end. An event-stream client sends nothing
/// after its request, so a read of zero bytes or an error means it is gone.
fn peer_closed(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return false;
    }
    let mut byte = [0u8; 1];
    let closed = match stream.peek(&mut byte) {
        Ok(0) => true,
        Ok(_) => false,
        Err(err) => err.kind() != io::ErrorKind::WouldBlock,
    };
    let _ = stream.set_nonblocking(false);
    closed
}

fn write_chunk(stream: &mut TcpStream, data: &str) -> io::Result<()> {
    let bytes = data.as_bytes();
    write!(stream, "{:X}\r\n", bytes.len())?;
    stream.write_all(bytes)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn write_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    keep_alive: bool,
    mcp_session: Option<&str>,
) -> io::Result<()> {
    let connection = if keep_alive { "keep-alive" } else { "close" };
    let session = mcp_session
        .map(|id| format!("Mcp-Session-Id: {id}\r\n"))
        .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\n{session}Connection: {connection}\r\n\r\n",
        reason = reason(status),
        len = body.len(),
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn write_ndjson(
    stream: &mut TcpStream,
    app: Arc<App>,
    statements: Vec<String>,
    file_base: dql_engine::FileBase,
) -> io::Result<()> {
    let header = "\
HTTP/1.1 200 OK\r\n\
Content-Type: application/x-ndjson; charset=utf-8\r\n\
Transfer-Encoding: chunked\r\n\
Cache-Control: no-store\r\n\
Connection: close\r\n\
\r\n";
    stream.write_all(header.as_bytes())?;
    stream.flush()?;
    let (mut reader, writer) = io::pipe()?;
    thread::spawn(move || run_script(app, statements, file_base, writer));
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        write!(stream, "{n:X}\r\n")?;
        stream.write_all(&buf[..n])?;
        stream.write_all(b"\r\n")?;
        stream.flush()?;
    }
    stream.write_all(b"0\r\n\r\n")?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        502 => "Bad Gateway",
        _ => "Error",
    }
}

fn write_json(writer: &mut dyn Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn lock_state(app: &App) -> std::sync::MutexGuard<'_, WebState> {
    app.state.lock().unwrap_or_else(|err| err.into_inner())
}

fn lock_writer<T>(writer: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    writer.lock().unwrap_or_else(|err| err.into_inner())
}

fn split_url(url: &str) -> (&str, &str) {
    url.split_once('?').unwrap_or((url, ""))
}

fn query_value(query: &str, key: &str) -> String {
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(name) == key {
            return percent_decode(value);
        }
    }
    String::new()
}

fn query_flag(query: &str, key: &str) -> bool {
    matches!(
        query_value(query, key).trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                    out.push(value);
                    index += 3;
                    continue;
                }
                out.push(b'%');
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_disconnect(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::TimedOut
    )
}

fn public_result(
    statement: &str,
    envelope: &ServeEnvelope,
    columns: Option<Vec<Column>>,
    elapsed_ms: u64,
    read_units: f64,
    write_units: f64,
) -> Value {
    let error = envelope
        .error
        .as_ref()
        .map(|err| err.message.as_str())
        .unwrap_or("");
    let message = match envelope.message.as_deref() {
        Some(message) if !message.is_empty() => message.to_string(),
        _ => error.to_string(),
    };
    let mut value = json!({
        "statement": statement,
        "ok": envelope.ok,
        "kind": envelope.kind,
        "message": message,
        "affected": envelope.affected,
        "items": envelope.items,
        "error": error,
        "elapsed_ms": elapsed_ms,
        "read_units": read_units,
        "write_units": write_units,
    });
    if envelope.ok && envelope.kind == "items" {
        if let Some(columns) = columns {
            value["columns"] = serde_json::to_value(columns).unwrap_or(Value::Null);
        }
    }
    if let Some(help) = &envelope.help {
        value["help"] = help.clone();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    #[test]
    fn serves_config_files_and_a_query() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());

        let config = http(addr, "GET", "/api/config", "");
        assert!(config.contains("\"page_size\":50"), "{config}");
        assert!(config.contains("\"endpoint\":\"memory\""));
        assert!(config.contains("\"kind\":\"memory\""), "{config}");
        assert!(config.contains("\"label\":\"memory\""), "{config}");
        let identity = http(addr, "GET", "/api/identity", "");
        assert!(identity.contains("\"ok\":false"), "{identity}");

        let created = http(addr, "POST", "/api/file", r#"{"path":"notes.dql"}"#);
        assert!(created.contains("notes.dql"));
        assert!(root.path().join("notes.dql").is_file());

        let page = http(addr, "GET", "/", "");
        assert!(page.contains("DQLRS Web"));
        assert!(page.contains("Content-Length:"));

        let script = "CREATE TABLE shelf (id STRING HASH KEY); INSERT INTO shelf (id, apple) VALUES ('a', 'p'); SELECT * FROM shelf WHERE id = 'a';";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let ran = http_body(addr, "POST", "/api/run", &body);
        assert!(ran.contains("\"event\":\"result\""), "{ran}");
        assert!(ran.contains("\"apple\":\"p\""), "{ran}");
        assert!(ran.contains("\"id\":\"a\""), "{ran}");
        assert!(ran.contains("\"elapsed_ms\":"), "{ran}");
        assert!(ran.contains("\"read_units\":"), "{ran}");
        assert!(ran.contains("\"write_units\":"), "{ran}");
        drop(guard);
    }

    #[test]
    fn api_and_mcp_require_the_startup_token() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);
        let token = server_token(addr);

        let page = http(addr, "GET", "/", "");
        assert!(
            page.contains(&format!(r#"name="dqlrs-token" content="{token}""#)),
            "{page}"
        );
        let script = http_anon(addr, "GET", "/app.js", "");
        assert!(script.contains("HTTP/1.1 200"), "{script}");
        assert!(!script.contains(&token), "{script}");
        let index = disk_static().unwrap().join("index.html");
        let disk = fs::read_to_string(&index).unwrap();
        assert!(!disk.contains("dqlrs-token"), "{disk}");
        assert!(!disk.contains(&token));

        let config = http(addr, "GET", "/api/config", "");
        assert!(config.contains("\"endpoint\":\"memory\""), "{config}");
        assert!(!config.contains(&token), "{config}");
        assert!(!config.contains("session_token"), "{config}");
        assert!(!config.contains("cache_key"), "{config}");
        let config_json: Value =
            serde_json::from_str(config.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert!(config_json.get("secret").is_none());
        assert!(config_json["environment"].get("secret").is_none());

        let script = "CREATE TABLE hidden (id STRING HASH KEY);";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let denied = http_anon(addr, "POST", "/api/run", &body);
        assert!(denied.contains("401"), "{denied}");
        assert!(denied.contains("unauthorized"), "{denied}");
        assert!(!denied.contains("\"event\":\"result\""), "{denied}");
        let wrong = http_bearer(addr, "POST", "/api/run", &body, "", Some("not-the-token"));
        assert!(wrong.contains("401"), "{wrong}");
        let probe = "SELECT * FROM hidden WHERE id = 'a';";
        let probe_body = format!("{{\"dql\":{}}}", serde_json::to_string(probe).unwrap());
        let probed = http_body(addr, "POST", "/api/run", &probe_body);
        assert!(probed.contains("not found"), "{probed}");

        let init = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(init.contains("serverInfo"), "{init}");
        let created = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_query_file","arguments":{"path":"leaked.dql","text":"SELECT 1;\n"}}}"#,
        );
        assert!(created.contains("leaked.dql"), "{created}");
        let sneaky = http_anon(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_query_file","arguments":{"path":"secret.dql","text":"SELECT 1;\n"}}}"#,
        );
        assert!(sneaky.contains("401"), "{sneaky}");
        assert!(sneaky.contains("unauthorized"), "{sneaky}");
        assert!(!root.path().join("secret.dql").exists());
        assert!(root.path().join("leaked.dql").is_file());
        drop(guard);
    }

    #[test]
    fn mcp_shares_the_web_session_and_workspace() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);

        let absent = http(addr, "GET", "/mcp", "");
        assert!(absent.contains("405"), "{absent}");

        let early = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        );
        assert!(early.contains("-32002"), "{early}");

        let init = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(init.contains("2025-06-18"), "{init}");
        let noted = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert!(noted.contains("202 Accepted"), "{noted}");

        let created = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_query_file","arguments":{"path":"queries/from-mcp.dql","text":"SELECT 1;\n"}}}"#,
        );
        assert!(created.contains("queries/from-mcp.dql"), "{created}");
        assert!(created.contains("\"isError\":false"), "{created}");
        let file = http(addr, "GET", "/api/file?path=queries/from-mcp.dql", "");
        assert!(file.contains("SELECT 1"), "{file}");

        let script = "CREATE TABLE shelf (id STRING HASH KEY);";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let ran = http_body(addr, "POST", "/api/run", &body);
        assert!(ran.contains("\"event\":\"result\""), "{ran}");
        let listed = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_tables","arguments":{}}}"#,
        );
        assert!(listed.contains("shelf"), "{listed}");

        let page = http(addr, "GET", "/", "");
        assert!(page.contains("DQLRS Web"), "{page}");

        let foreign = http_origin(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#,
            "http://evil.example",
        );
        assert!(foreign.contains("403"), "{foreign}");
        drop(guard);
    }

    #[test]
    fn request_access_runs_classifies_and_grants() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);
        let init = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(init.contains("Mcp-Session-Id:"), "{init}");

        let setup = "CREATE TABLE people (id STRING HASH KEY, email STRING); INSERT INTO people (id, email) VALUES ('1', 'ada@example.com');";
        let setup_body = format!("{{\"dql\":{}}}", serde_json::to_string(setup).unwrap());
        let created = http_body(addr, "POST", "/api/run", &setup_body);
        assert!(created.contains("\"ok\":true"), "{created}");

        let addr_wait = addr;
        let waiting = thread::spawn(move || {
            http(
                addr_wait,
                "POST",
                "/mcp",
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"request_access","arguments":{"query":"SCAN * FROM people"}}}"#,
            )
        });
        let started = Instant::now();
        let listed = loop {
            let body = http(addr, "GET", "/api/mcp-access", "");
            if body.contains("\"id\":\"r1\"") {
                break body;
            }
            if started.elapsed() > Duration::from_secs(2) {
                panic!("{body}");
            }
            thread::sleep(Duration::from_millis(20));
        };
        assert!(listed.contains("SCAN * FROM people"), "{listed}");

        let ran = http(addr, "POST", "/api/mcp-access/run", r#"{"request":"r1"}"#);
        assert!(ran.contains("Recommend not granting"), "{ran}");
        assert!(ran.contains("ada@example.com"), "{ran}");
        assert!(ran.contains("email"), "{ran}");

        let granted = http(addr, "POST", "/api/mcp-access/grant", r#"{"request":"r1"}"#);
        assert!(granted.contains("mcp_r1"), "{granted}");
        let answer = waiting.join().unwrap();
        assert!(
            answer.contains("\\\"status\\\": \\\"granted\\\""),
            "{answer}"
        );
        assert!(answer.contains("mcp_r1"), "{answer}");
        let again = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"request_access","arguments":{"request":"r1"}}}"#,
        );
        assert!(again.contains("\\\"status\\\": \\\"granted\\\""), "{again}");

        let denied = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"request_access","arguments":{"query":"INSERT INTO people (id) VALUES ('2')"}}}"#,
        );
        assert!(denied.contains("\"isError\":true"), "{denied}");
        drop(guard);
    }

    #[test]
    fn an_editor_drop_clears_the_run_and_a_revoke_answers_revoked() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);
        let init = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(init.contains("2025-06-18"), "{init}");
        let setup = "CREATE TABLE people (id STRING HASH KEY, email STRING); INSERT INTO people (id, email) VALUES ('1', 'ada@example.com');";
        let setup_body = format!("{{\"dql\":{}}}", serde_json::to_string(setup).unwrap());
        let created = http_body(addr, "POST", "/api/run", &setup_body);
        assert!(created.contains("\"ok\":true"), "{created}");

        let addr_wait = addr;
        let waiting = thread::spawn(move || {
            http(
                addr_wait,
                "POST",
                "/mcp",
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"request_access","arguments":{"query":"SCAN * FROM people"}}}"#,
            )
        });
        let started = Instant::now();
        loop {
            let body = http(addr, "GET", "/api/mcp-access", "");
            if body.contains("\"id\":\"r1\"") {
                break;
            }
            if started.elapsed() > Duration::from_secs(2) {
                panic!("{body}");
            }
            thread::sleep(Duration::from_millis(20));
        }
        let ran = http(addr, "POST", "/api/mcp-access/run", r#"{"request":"r1"}"#);
        assert!(ran.contains("\"status\":\"ran\""), "{ran}");

        let dropped = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"DROP TEMPTABLE mcp_r1;"}"#,
        );
        assert!(dropped.contains("\"ok\":true"), "{dropped}");
        let cleared = http(addr, "GET", "/api/mcp-access", "");
        assert!(cleared.contains("\"status\":\"open\""), "{cleared}");
        assert!(!cleared.contains("Recommend not granting"), "{cleared}");

        let ran_again = http(addr, "POST", "/api/mcp-access/run", r#"{"request":"r1"}"#);
        assert!(ran_again.contains("\"status\":\"ran\""), "{ran_again}");
        let granted = http(addr, "POST", "/api/mcp-access/grant", r#"{"request":"r1"}"#);
        assert!(granted.contains("mcp_r1"), "{granted}");
        let answer = waiting.join().unwrap();
        assert!(
            answer.contains("\\\"status\\\": \\\"granted\\\""),
            "{answer}"
        );

        let revoked = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"REVOKE MCP READ ON TEMPTABLE mcp_r1;"}"#,
        );
        assert!(revoked.contains("\"ok\":true"), "{revoked}");
        let later = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"request_access","arguments":{"request":"r1"}}}"#,
        );
        assert!(later.contains("\\\"status\\\": \\\"revoked\\\""), "{later}");
        assert!(later.contains("mcp_r1"), "{later}");
        drop(guard);
    }

    #[test]
    fn mcp_turns_on_and_off_from_the_web_app() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), false);
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
        let off = http(addr, "POST", "/mcp", init);
        assert!(off.contains("404"), "{off}");
        let config = http(addr, "GET", "/api/config", "");
        assert!(
            config.contains(r#""mcp":{"enabled":false,"path":"/mcp"}"#),
            "{config}"
        );

        // The menu reads the address and both configs, with this server's token in them.
        let info: Value = serde_json::from_str(&http_body(addr, "GET", "/api/mcp", "")).unwrap();
        assert_eq!(info["enabled"], false, "{info}");
        assert_eq!(info["url"], format!("http://{addr}/mcp"), "{info}");
        let token = server_token(addr);
        let cursor = info["config"]["cursor"].as_str().unwrap();
        let claude = info["config"]["claude"].as_str().unwrap();
        for text in [cursor, claude] {
            let parsed: Value = serde_json::from_str(text).unwrap();
            let server = &parsed["mcpServers"]["dqlrs"];
            assert_eq!(server["url"], format!("http://{addr}/mcp"), "{text}");
            assert_eq!(
                server["headers"]["Authorization"],
                format!("Bearer {token}"),
                "{text}"
            );
        }
        assert!(claude.contains(r#""type": "http""#), "{claude}");
        assert!(!cursor.contains(r#""type""#), "{cursor}");

        // A browser tab hears the change on its event stream.
        let mut events = TcpStream::connect(addr).unwrap();
        events
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        write!(
            events,
            "GET /api/events?id=web-mcp-pill HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut seen = String::new();
        let mut bytes = [0u8; 2048];
        read_until(&mut events, &mut seen, &mut bytes, |text| {
            text.contains("event: presence")
        });

        let bad = http(addr, "POST", "/api/mcp", r#"{"enabled":"yes"}"#);
        assert!(bad.contains("400"), "{bad}");
        let on = http(addr, "POST", "/api/mcp", r#"{"enabled":true}"#);
        assert!(on.contains(r#""enabled":true"#), "{on}");
        read_until(&mut events, &mut seen, &mut bytes, |text| {
            text.contains("event: mcp") && text.contains(r#""enabled":true"#)
        });
        assert!(seen.contains(r#"{"enabled":true}"#), "{seen}");
        let config = http(addr, "GET", "/api/config", "");
        assert!(config.contains(r#""mcp":{"enabled":true"#), "{config}");
        let answered = http(addr, "POST", "/mcp", init);
        assert!(answered.contains("2025-06-18"), "{answered}");

        // An agent's open stream ends when MCP is turned off, and /mcp stops answering.
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        write!(
            stream,
            "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut buf = String::new();
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("200 OK")
        });
        assert!(buf.contains("text/event-stream"), "{buf}");
        let turned_off = http(addr, "POST", "/api/mcp", r#"{"enabled":false}"#);
        assert!(turned_off.contains(r#""enabled":false"#), "{turned_off}");
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut closed = false;
        while Instant::now() < deadline {
            match stream.read(&mut bytes) {
                Ok(0) => {
                    closed = true;
                    break;
                }
                Ok(_) => {}
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    closed = true;
                    break;
                }
            }
        }
        assert!(
            closed,
            "the agent stream stayed open after MCP was turned off"
        );
        let after = http(addr, "POST", "/mcp", init);
        assert!(after.contains("404"), "{after}");
        read_until(&mut events, &mut seen, &mut bytes, |text| {
            text.contains(r#"{"enabled":false}"#)
        });
        assert!(seen.contains(r#"{"enabled":false}"#), "{seen}");
        drop(guard);
    }

    #[test]
    fn mcp_grant_stream_replays_changes_and_rejects_a_bare_get() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);
        let init = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(init.contains("2025-06-18"), "{init}");

        let bare = http(addr, "GET", "/mcp", "");
        assert!(bare.contains("405"), "{bare}");
        let anon = http_anon(addr, "GET", "/mcp", "");
        assert!(anon.contains("401"), "{anon}");
        let foreign = http_origin(addr, "GET", "/mcp", "", "http://evil.example");
        assert!(foreign.contains("403"), "{foreign}");

        let setup = "\
CREATE TABLE people (id STRING HASH KEY, email STRING);
INSERT INTO people (id, email) VALUES ('1', 'ada@example.com');
CREATE TEMPTABLE mail AS (SCAN * FROM people);
GRANT MCP READ ON TEMPTABLE mail;";
        let setup_body = format!("{{\"dql\":{}}}", serde_json::to_string(setup).unwrap());
        let created = http_body(addr, "POST", "/api/run", &setup_body);
        assert!(created.contains("\"ok\":true"), "{created}");

        let token = server_token(addr);
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        write!(
            stream,
            "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut buf = String::new();
        let mut bytes = [0u8; 2048];
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("\"change\":\"granted\"") && text.contains("\"table\":\"mail\"")
        });
        assert!(buf.contains("notifications/dqlrs/grant_changed"), "{buf}");
        assert!(buf.contains("\"change\":\"granted\""), "{buf}");
        assert!(buf.contains("\"table\":\"mail\""), "{buf}");

        let revoked = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"REVOKE MCP READ ON TEMPTABLE mail;"}"#,
        );
        assert!(revoked.contains("\"ok\":true"), "{revoked}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("\"change\":\"revoked\"") && text.contains("\"table\":\"mail\"")
        });
        assert!(buf.contains("\"change\":\"revoked\""), "{buf}");

        let granted = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"GRANT MCP READ ON TEMPTABLE mail;"}"#,
        );
        assert!(granted.contains("\"ok\":true"), "{granted}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.matches("\"change\":\"granted\"").count() >= 2
        });
        let before_second = buf.matches("\"change\":\"granted\"").count();
        let again = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"GRANT MCP READ ON TEMPTABLE mail;"}"#,
        );
        assert!(again.contains("\"ok\":true"), "{again}");
        thread::sleep(Duration::from_millis(300));
        if let Ok(n) = stream.read(&mut bytes) {
            if n > 0 {
                buf.push_str(&String::from_utf8_lossy(&bytes[..n]));
            }
        }
        assert_eq!(
            buf.matches("\"change\":\"granted\"").count(),
            before_second,
            "{buf}"
        );

        let all = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"REVOKE MCP READ ALL;"}"#,
        );
        assert!(all.contains("\"ok\":true"), "{all}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.matches("\"change\":\"revoked\"").count() >= 2
        });

        let replaced = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"GRANT MCP READ ON TEMPTABLE mail; DROP TEMPTABLE mail; CREATE TEMPTABLE mail AS (SCAN * FROM people);"}"#,
        );
        assert!(replaced.contains("\"ok\":true"), "{replaced}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.matches("\"change\":\"revoked\"").count() >= 3
        });
        let granted_after_replace = buf.matches("\"change\":\"granted\"").count();
        thread::sleep(Duration::from_millis(200));
        if let Ok(n) = stream.read(&mut bytes) {
            if n > 0 {
                buf.push_str(&String::from_utf8_lossy(&bytes[..n]));
            }
        }
        assert_eq!(
            buf.matches("\"change\":\"granted\"").count(),
            granted_after_replace,
            "{buf}"
        );

        let addr_wait = addr;
        let waiting = thread::spawn(move || {
            http(
                addr_wait,
                "POST",
                "/mcp",
                r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"request_access","arguments":{"query":"SCAN * FROM people"}}}"#,
            )
        });
        let started = Instant::now();
        loop {
            let body = http(addr, "GET", "/api/mcp-access", "");
            if body.contains("\"id\":\"r1\"") {
                break;
            }
            if started.elapsed() > Duration::from_secs(2) {
                panic!("{body}");
            }
            thread::sleep(Duration::from_millis(20));
        }
        let quiet = buf.matches("grant_changed").count();
        let ran = http(addr, "POST", "/api/mcp-access/run", r#"{"request":"r1"}"#);
        assert!(ran.contains("Recommend not granting"), "{ran}");
        thread::sleep(Duration::from_millis(200));
        if let Ok(n) = stream.read(&mut bytes) {
            if n > 0 {
                buf.push_str(&String::from_utf8_lossy(&bytes[..n]));
            }
        }
        assert_eq!(buf.matches("grant_changed").count(), quiet, "{buf}");

        let denied = http(addr, "POST", "/api/mcp-access/deny", r#"{"request":"r1"}"#);
        assert!(
            denied.contains("\"ok\":true") || denied.contains("\"requests\""),
            "{denied}"
        );
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("\"change\":\"denied\"") && text.contains("\"request\":\"r1\"")
        });
        assert!(buf.contains("\"change\":\"denied\""), "{buf}");
        assert!(!buf.contains("\"table\":\"mcp_r1\""), "{buf}");
        let answer = waiting.join().unwrap();
        assert!(
            answer.contains("\\\"status\\\": \\\"denied\\\""),
            "{answer}"
        );
        drop(stream);
        drop(guard);
    }

    #[test]
    fn lint_endpoint_reports_a_bad_query() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let reply = http(
            addr,
            "POST",
            "/api/lint",
            r#"{"text":"SELEKT * FROM shelf;\n"}"#,
        );
        assert!(reply.contains("\"ok\":false"), "{reply}");
        assert!(reply.contains("\"severity\":\"error\""), "{reply}");
        let clean = http(
            addr,
            "POST",
            "/api/lint",
            r#"{"text":"CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf WHERE id = 'a';\n"}"#,
        );
        assert!(clean.contains("\"ok\":true"), "{clean}");
        assert!(clean.contains("\"issues\":[]"), "{clean}");
        drop(guard);
    }

    #[test]
    fn lint_endpoint_does_not_run_the_script() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let setup = "CREATE TABLE shelf (id STRING HASH KEY); INSERT INTO shelf (id, title) VALUES ('a', 'kept');";
        let setup_body = format!("{{\"dql\":{}}}", serde_json::to_string(setup).unwrap());
        let created = http_body(addr, "POST", "/api/run", &setup_body);
        assert!(created.contains("\"event\":\"result\""), "{created}");

        let script = "\
CREATE TABLE other (id STRING HASH KEY);
INSERT INTO shelf (id, title) VALUES ('b', 'new');
UPDATE shelf SET title = 'changed' WHERE id = 'a';
DELETE FROM shelf WHERE id = 'a';
SELECT title FROM shelf WHERE id = 'a';
SELECT * FROM shelf;
SCAN * FROM shelf;
DROP TABLE shelf;
DROP TABLE other;
";
        let lint_body = format!("{{\"text\":{}}}", serde_json::to_string(script).unwrap());
        let linted = http(addr, "POST", "/api/lint", &lint_body);
        assert!(linted.contains("\"ok\":false"), "{linted}");
        assert!(linted.contains("indexed WHERE"), "{linted}");

        let check = "SELECT * FROM shelf WHERE id = 'a';";
        let check_body = format!("{{\"dql\":{}}}", serde_json::to_string(check).unwrap());
        let kept = http_body(addr, "POST", "/api/run", &check_body);
        assert!(kept.contains("\"title\":\"kept\""), "{kept}");
        assert!(!kept.contains("changed"), "{kept}");
        assert!(!kept.contains("\"id\":\"b\""), "{kept}");

        let missing = "SELECT * FROM other WHERE id = 'a';";
        let missing_body = format!("{{\"dql\":{}}}", serde_json::to_string(missing).unwrap());
        let absent = http_body(addr, "POST", "/api/run", &missing_body);
        assert!(absent.contains("not found"), "{absent}");
        drop(guard);
    }

    #[test]
    fn mcp_path_stays_closed_without_the_flag() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let missing = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert!(missing.contains("404"), "{missing}");
        let page = http(addr, "GET", "/api/config", "");
        assert!(page.contains("\"endpoint\":\"memory\""), "{page}");
        drop(guard);
    }

    #[test]
    fn stop_ends_a_throttled_insert() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let mut values = Vec::new();
        for index in 0..30 {
            values.push(format!("('{index}')"));
        }
        let script = format!(
            "CREATE TABLE shelf (id STRING HASH KEY); INSERT INTO shelf (id) VALUES {} THROTTLE 10 10;",
            values.join(", ")
        );
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(&script).unwrap());
        let pending = thread::spawn(move || http_body(addr, "POST", "/api/run", &body));
        let started = Instant::now();
        let mut stopped = false;
        while started.elapsed() < Duration::from_secs(4) {
            thread::sleep(Duration::from_millis(50));
            let reply = http(addr, "POST", "/api/stop", "");
            if reply.contains("\"ok\":true") {
                stopped = true;
                break;
            }
        }
        assert!(stopped, "stop request did not succeed");
        let ran = pending.join().expect("run thread");
        assert!(
            ran.contains("Query stopped") || ran.contains("\"code\":\"cancelled\""),
            "{ran}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "{:?}",
            started.elapsed()
        );
        drop(guard);
    }

    #[test]
    fn answers_pipelined_static_gets() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let request = "\
GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: keep-alive\r\n\r\n\
GET /app.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: keep-alive\r\n\r\n\
GET /vendor/codemirror-language.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        let started = Instant::now();
        stream.write_all(request.as_bytes()).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "pipelined static responses took {:?}",
            started.elapsed()
        );
        let text = String::from_utf8_lossy(&buf);
        assert_eq!(text.matches("HTTP/1.1 200 OK").count(), 3, "{text}");
        assert!(text.contains("DQLRS Web"));
        assert!(text.contains("Content-Length:"));
        assert!(!text.contains("Transfer-Encoding: chunked"));
        drop(guard);
    }

    #[test]
    fn stale_save_conflicts_and_presence_lists_web_and_mcp() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server_mcp(root.path().to_path_buf(), true);
        let created = http(addr, "POST", "/api/file", r#"{"path":"notes.dql"}"#);
        assert!(created.contains("notes.dql"), "{created}");

        let read = http(addr, "GET", "/api/file?path=notes.dql", "");
        let revision = json_string_field(&read, "revision");
        let wrote = http(
            addr,
            "PUT",
            "/api/file",
            &format!(r#"{{"path":"notes.dql","text":"from-mcp\n","base":"{revision}"}}"#),
        );
        assert!(wrote.contains("HTTP/1.1 200"), "{wrote}");
        let again = http(addr, "GET", "/api/file?path=notes.dql", "");
        assert!(again.contains("from-mcp"), "{again}");
        let current = json_string_field(&again, "revision");

        let stale = http(
            addr,
            "PUT",
            "/api/file",
            &format!(r#"{{"path":"notes.dql","text":"web-old\n","base":"{revision}"}}"#),
        );
        assert!(stale.contains("HTTP/1.1 409"), "{stale}");
        assert!(stale.contains("from-mcp"), "{stale}");
        assert!(stale.contains("\"conflict\":true"), "{stale}");
        let still = http(addr, "GET", "/api/file?path=notes.dql", "");
        assert!(still.contains("from-mcp"), "{still}");
        assert!(!still.contains("web-old"), "{still}");
        assert_eq!(json_string_field(&still, "revision"), current);

        let first = http(
            addr,
            "POST",
            "/api/presence",
            r#"{"id":"web-window-a","kind":"web"}"#,
        );
        assert!(first.contains("web-window-a"), "{first}");
        let second = http(
            addr,
            "POST",
            "/api/presence",
            r#"{"id":"web-window-b","kind":"web"}"#,
        );
        assert!(second.contains("web-window-a"), "{second}");
        assert!(second.contains("web-window-b"), "{second}");
        let mcp = http(
            addr,
            "POST",
            "/mcp",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert!(
            mcp.contains("serverInfo") || mcp.contains("protocolVersion"),
            "{mcp}"
        );
        let listed = http(addr, "GET", "/api/presence", "");
        assert!(listed.contains("\"kind\":\"web\""), "{listed}");
        assert!(listed.contains("\"kind\":\"mcp\""), "{listed}");
        assert!(listed.contains("web-window-a"), "{listed}");
        assert!(listed.contains("web-window-b"), "{listed}");
        assert!(listed.contains("\"label\":\"W1\""), "{listed}");
        assert!(listed.contains("\"label\":\"W2\""), "{listed}");
        assert!(listed.contains("\"label\":\"M1\""), "{listed}");
        drop(guard);
    }

    fn json_string_field(raw: &str, key: &str) -> String {
        let needle = format!("\"{key}\":\"");
        let start = raw
            .find(&needle)
            .unwrap_or_else(|| panic!("no {key} in {raw}"))
            + needle.len();
        raw[start..]
            .split('"')
            .next()
            .unwrap_or_else(|| panic!("unterminated {key} in {raw}"))
            .to_string()
    }

    struct ServerGuard {
        stop: Arc<AtomicBool>,
        addr: SocketAddr,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Drop for ServerGuard {
        fn drop(&mut self) {
            test_tokens()
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .remove(&self.addr);
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(self.addr);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    #[test]
    fn the_connection_line_names_the_role_or_user() {
        assert_eq!(
            principal_label("arn:aws:sts::123456789012:assumed-role/Admin/jonsmith"),
            "role Admin (jonsmith)"
        );
        assert_eq!(
            principal_label("arn:aws:iam::123456789012:user/alice"),
            "user alice"
        );
        assert_eq!(principal_label("arn:aws:iam::123456789012:root"), "root");
    }

    #[test]
    fn load_tables_describes_every_memory_table() {
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(App::new(
            crate::session::Session::new_memory_headless("memory"),
            root.path().to_path_buf(),
            false,
            crate::api_token::load().unwrap(),
        ));
        {
            let mut state = lock_state(&app);
            let created = state
                .session
                .execute_for_serve("CREATE TABLE posts (id STRING HASH KEY, sk NUMBER RANGE KEY);");
            assert!(created.ok, "{created:?}");
            let created = state
                .session
                .execute_for_serve("CREATE TABLE comments (id STRING HASH KEY);");
            assert!(created.ok, "{created:?}");
            state.session.engine.set_table_cache(None);
        }
        let mut events = Vec::new();
        load_session_tables(&app, &mut |event| {
            events.push(match event {
                LoadEvent::Listed(total) => format!("listed {total}"),
                LoadEvent::Described {
                    index,
                    total,
                    name,
                    result,
                } => {
                    format!(
                        "{index}/{total} {name} {}",
                        result.map(|meta| meta.is_some()).unwrap_or(false)
                    )
                }
            })
        })
        .unwrap();
        assert_eq!(events, ["listed 2", "1/2 comments true", "2/2 posts true"]);
        let mut state = lock_state(&app);
        let posts = state
            .session
            .engine
            .cached_definition("posts")
            .expect("posts");
        assert_eq!(posts.hash_key, "id");
        assert_eq!(posts.range_key.as_deref(), Some("sk"));
        assert!(state.session.engine.cached_definition("comments").is_some());
    }

    #[test]
    fn structure_reads_the_whole_file_and_only_a_loaded_page() {
        let root = tempfile::tempdir().unwrap();
        let mut lines = vec!["{\"id\":\"a\",\"name\":\"ada\"}".to_string()];
        for index in 1..50 {
            lines.push(format!("{{\"id\":\"r{index}\"}}"));
        }
        lines.push("{\"id\":\"last\",\"extra\":true}".to_string());
        fs::write(root.path().join("rows.json"), lines.join("\n") + "\n").unwrap();
        fs::write(root.path().join("notes.dql"), "SELECT 1;\n").unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());

        let unloaded = http(addr, "GET", "/api/structure?table=posts", "");
        assert!(unloaded.contains("table page is not loaded"), "{unloaded}");
        assert!(!unloaded.contains("\"columns\""), "{unloaded}");

        let file = http_body(addr, "GET", "/api/structure?path=rows.json", "");
        let file_json: serde_json::Value = serde_json::from_str(&file).expect(&file);
        assert_eq!(file_json["kind"], "file");
        assert_eq!(file_json["rows"], 51);
        assert_eq!(file_json["paged"], false);
        let extra = file_json["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["name"] == "extra")
            .expect("extra column from the last row");
        assert_eq!(extra["kinds"], serde_json::json!(["BOOL"]));
        assert_eq!(extra["present"], 1);
        assert_eq!(extra["total"], 51);
        let name = file_json["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["name"] == "name")
            .unwrap();
        assert_eq!(name["present"], 1);
        assert_eq!(name["kinds"], serde_json::json!(["STRING"]));

        let setup = "CREATE TABLE posts (id STRING HASH KEY, sk NUMBER RANGE KEY); INSERT INTO posts (id, sk) VALUES ('a', 1), ('b', 2);";
        let setup_body = format!("{{\"dql\":{}}}", serde_json::to_string(setup).unwrap());
        let created = http_body(addr, "POST", "/api/run", &setup_body);
        assert!(created.contains("\"ok\":true"), "{created}");
        let page = http_body(addr, "GET", "/api/rows?table=posts&page=0", "");
        assert!(page.contains("\"id\""), "{page}");
        // The table browser's Explain tab: the plan of what Run would read, LIMIT included.
        let explain = http_body(addr, "GET", "/api/explain?table=posts", "");
        let explain_json: Value = serde_json::from_str(&explain).expect(&explain);
        assert_eq!(
            explain_json["plans"][0]["statement"],
            "SCAN * FROM posts LIMIT 51"
        );
        assert!(
            explain_json["plans"][0]["plan"]
                .as_str()
                .unwrap()
                .contains("posts"),
            "{explain}"
        );
        let bad = http(addr, "GET", "/api/explain?table=no%20such", "");
        assert!(bad.contains("502"), "{bad}");
        // The table browser shows these as the read's time and cost pills.
        let page_json: Value = serde_json::from_str(&page).expect(&page);
        assert!(page_json["elapsed_ms"].is_u64(), "{page}");
        assert!(page_json["read_units"].is_number(), "{page}");
        assert!(page_json["write_units"].is_number(), "{page}");
        let structure = http_body(addr, "GET", "/api/structure?table=posts", "");
        let structure_json: serde_json::Value = serde_json::from_str(&structure).expect(&structure);
        assert_eq!(structure_json["kind"], "table");
        assert_eq!(structure_json["paged"], true);
        assert_eq!(structure_json["rows"], 2);
        let id = structure_json["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["name"] == "id")
            .unwrap();
        assert_eq!(id["table"], "hash");
        assert_eq!(id["kinds"], serde_json::json!(["STRING"]));
        let sk = structure_json["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["name"] == "sk")
            .unwrap();
        assert_eq!(sk["table"], "range");
        assert_eq!(sk["kinds"], serde_json::json!(["NUMBER"]));
        let create_query = structure_json["createQuery"].as_str().unwrap_or("");
        assert!(
            create_query.contains("CREATE TABLE posts ("),
            "{create_query}"
        );
        assert!(
            create_query.contains("\n  id STRING HASH KEY,"),
            "{create_query}"
        );
        assert!(
            create_query.contains("sk NUMBER RANGE KEY"),
            "{create_query}"
        );
        let definition = structure_json["definition"]
            .as_array()
            .expect("definition rows");
        assert_eq!(definition.len(), 2);
        assert_eq!(definition[0]["name"], "id");
        assert_eq!(definition[0]["type"], "STRING");
        assert_eq!(definition[0]["role"], "hash");
        assert_eq!(definition[1]["name"], "sk");
        assert_eq!(definition[1]["type"], "NUMBER");
        assert_eq!(definition[1]["role"], "range");

        // The test server runs Session::new_memory_headless("memory"): key memory|memory.
        let results = super::query_cache::Store {
            workspace: root.path(),
            connection_key: "memory|memory",
        };
        let saved = super::query_cache::save(
            results,
            "notes.dql",
            "SCAN * FROM posts",
            serde_json::json!({"items": [{"id": "a", "sk": 1}], "columns": [{"name": "id", "table": "hash"}, {"name": "sk", "table": "range"}]}),
            true,
        )
        .unwrap();
        let listed = super::query_cache::list(results, "notes.dql").unwrap();
        assert_eq!(listed.len(), 1);
        let result_path = format!("/api/structure?result={}", listed[0].id);
        let result = http_body(addr, "GET", &result_path, "");
        let result_json: serde_json::Value = serde_json::from_str(&result).expect(&result);
        assert_eq!(result_json["kind"], "result");
        assert_eq!(result_json["title"], "posts");
        assert_eq!(result_json["rows"], 1);
        assert_eq!(result_json["paged"], false);
        assert_eq!(result_json["statement"], "SCAN * FROM posts");
        let _ = saved;
        drop(guard);
    }

    #[test]
    fn tree_lists_workspace_files_and_file_rows_pages_csv() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("queries")).unwrap();
        fs::write(root.path().join("queries/edit.dql"), "SELECT 1;\n").unwrap();
        fs::write(root.path().join("users.csv"), "id,name\na,ada\nb,grace\n").unwrap();
        fs::write(root.path().join("notes.txt"), "hidden\n").unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());

        let tree = http_body(addr, "GET", "/api/tree", "");
        let tree_json: Value = serde_json::from_str(&tree).expect(&tree);
        assert_eq!(tree_json["type"], "dir");
        let names: Vec<&str> = tree_json["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|child| child["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["queries", "users.csv"]);
        let queries = tree_json["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|child| child["name"] == "queries")
            .unwrap();
        assert_eq!(queries["children"][0]["path"], "queries/edit.dql");
        assert!(!tree.contains("notes.txt"), "{tree}");

        let rows = http_body(addr, "GET", "/api/file-rows?path=users.csv&page=0", "");
        let rows_json: Value = serde_json::from_str(&rows).expect(&rows);
        assert_eq!(rows_json["path"], "users.csv");
        assert_eq!(rows_json["page"], 0);
        assert_eq!(rows_json["has_more"], false);
        assert_eq!(rows_json["items"][0]["name"], "ada");
        assert_eq!(rows_json["items"][1]["id"], "b");
        assert_eq!(rows_json["columns"][0]["name"], "id");

        let rejected = http(addr, "GET", "/api/file-rows?path=queries/edit.dql", "");
        assert!(rejected.contains("HTTP/1.1 400"), "{rejected}");
        assert!(
            rejected.contains("only .csv and .json files can be viewed as tables"),
            "{rejected}"
        );
        drop(guard);
    }

    #[test]
    fn search_finds_a_query_file() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("shelf.dql"), "SCAN * FROM shelf;\n").unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());

        let found = http_body(addr, "GET", "/api/search?q=shelf", "");
        let found_json: Value = serde_json::from_str(&found).expect(&found);
        let hit = found_json["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["path"] == "shelf.dql")
            .expect(&found);
        assert_eq!(hit["kind"], "query");
        assert_eq!(hit["title"], "SCAN * FROM shelf;");
        assert_eq!(hit["detail"], "shelf.dql");

        let empty = http_body(addr, "GET", "/api/search?q=", "");
        let empty_json: Value = serde_json::from_str(&empty).expect(&empty);
        assert!(
            empty_json["items"].as_array().unwrap().is_empty(),
            "{empty}"
        );
        drop(guard);
    }

    #[test]
    fn tables_and_table_cache_follow_the_session() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let script = "CREATE TABLE posts (id STRING HASH KEY);";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let ran = http_body(addr, "POST", "/api/run", &body);
        assert!(ran.contains("\"ok\":true"), "{ran}");

        let listed = http_body(addr, "GET", "/api/tables", "");
        let listed_json: Value = serde_json::from_str(&listed).expect(&listed);
        let posts = listed_json["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == "posts")
            .expect(&listed);
        assert_eq!(posts["kind"], "dynamodb");
        assert_eq!(posts["keys"][0]["name"], "id");
        assert_eq!(posts["keys"][0]["role"], "hash");
        assert_eq!(posts["attributes"][0]["name"], "id");
        assert_eq!(posts["attributes"][0]["type"], "STRING");
        assert!(posts["indexes"].as_array().unwrap().is_empty(), "{posts}");

        let filtered = http_body(addr, "GET", "/api/tables?pattern=zzzz", "");
        let filtered_json: Value = serde_json::from_str(&filtered).expect(&filtered);
        assert!(
            filtered_json["tables"].as_array().unwrap().is_empty(),
            "{filtered}"
        );

        let one = http_body(addr, "GET", "/api/tables?name=posts", "");
        let one_json: Value = serde_json::from_str(&one).expect(&one);
        assert_eq!(one_json["tables"].as_array().unwrap().len(), 1);
        assert_eq!(one_json["tables"][0]["name"], "posts");
        assert_eq!(one_json["tables"][0]["kind"], "dynamodb");
        assert_eq!(one_json["tables"][0]["keys"][0]["name"], "id");
        let missing = http_body(addr, "GET", "/api/tables?name=not_a_table", "");
        let missing_json: Value = serde_json::from_str(&missing).expect(&missing);
        assert!(
            missing_json["tables"].as_array().unwrap().is_empty(),
            "{missing}"
        );

        let cache = http_body(addr, "GET", "/api/table-cache", "");
        let cache_json: Value = serde_json::from_str(&cache).expect(&cache);
        assert_eq!(cache_json["text"], "No table list cached.");
        assert_eq!(cache_json["fromCache"], false);
        assert_eq!(cache_json["refreshing"], false);
        drop(guard);
    }

    #[test]
    fn table_browser_runs_select_scan_and_rejects_writes() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let script = "CREATE TABLE posts (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-sk', sk NUMBER, id STRING); INSERT INTO posts (id, sk, status) VALUES ('a', 1, 'open'), ('a', 2, 'shut'), ('b', 3, 'open');";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let created = http_body(addr, "POST", "/api/run", &body);
        assert!(created.contains("\"ok\":true"), "{created}");

        let listed = http_body(addr, "GET", "/api/tables?name=posts", "");
        let listed_json: Value = serde_json::from_str(&listed).expect(&listed);
        let posts = &listed_json["tables"][0];
        assert_eq!(posts["keys"][1]["name"], "sk");
        assert_eq!(posts["indexes"][0]["name"], "by-sk");
        assert_eq!(posts["indexes"][0]["kind"], "global");
        assert_eq!(posts["indexes"][0]["hashKey"], "sk");
        let attr_names: Vec<&str> = posts["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|attr| attr["name"].as_str().unwrap())
            .collect();
        assert!(attr_names.contains(&"id"), "{posts}");
        assert!(attr_names.contains(&"sk"), "{posts}");

        let page = http_body(addr, "GET", "/api/rows?table=posts&page=0", "");
        let page_json: Value = serde_json::from_str(&page).expect(&page);
        assert_eq!(page_json["items"].as_array().unwrap().len(), 3);
        assert_eq!(page_json["dql"], "SCAN * FROM posts LIMIT 51");

        let queried = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SELECT * FROM posts WHERE id = 'a'")
            ),
            "",
        );
        let queried_json: Value = serde_json::from_str(&queried).expect(&queried);
        assert_eq!(queried_json["items"].as_array().unwrap().len(), 2);
        assert!(
            queried_json["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["id"] == "a"),
            "{queried}"
        );

        let newest = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SELECT * FROM posts WHERE id = 'a' DESC")
            ),
            "",
        );
        let newest_json: Value = serde_json::from_str(&newest).expect(&newest);
        assert_eq!(newest_json["items"][0]["sk"], 2);
        assert_eq!(newest_json["items"][1]["sk"], 1);

        let filtered = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SCAN * FROM posts WHERE status = 'open'")
            ),
            "",
        );
        let filtered_json: Value = serde_json::from_str(&filtered).expect(&filtered);
        assert_eq!(filtered_json["items"].as_array().unwrap().len(), 2);

        let indexed = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SELECT * FROM posts WHERE sk = 1 USING by-sk")
            ),
            "",
        );
        let indexed_json: Value = serde_json::from_str(&indexed).expect(&indexed);
        assert_eq!(indexed_json["items"].as_array().unwrap().len(), 1);
        assert_eq!(indexed_json["items"][0]["id"], "a");

        let by_keys = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SELECT * FROM posts KEYS IN ('a', 1), ('b', 3)")
            ),
            "",
        );
        let by_keys_json: Value = serde_json::from_str(&by_keys).expect(&by_keys);
        assert_eq!(by_keys_json["items"].as_array().unwrap().len(), 2);
        assert_eq!(
            by_keys_json["dql"],
            "SELECT * FROM posts KEYS IN ('a', 1), ('b', 3)"
        );
        assert_eq!(by_keys_json["has_more"], false);

        let grouped_keys = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SELECT * FROM posts KEYS IN ((a, 1), (b, 3)),")
            ),
            "",
        );
        let grouped_keys_json: Value = serde_json::from_str(&grouped_keys).expect(&grouped_keys);
        assert_eq!(grouped_keys_json["items"].as_array().unwrap().len(), 2);

        let by_index_keys = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode(
                    "SELECT * FROM posts WHERE sk = 1 USING by-sk; SELECT * FROM posts WHERE sk = 3 USING by-sk"
                )
            ),
            "",
        );
        let by_index_keys_json: Value = serde_json::from_str(&by_index_keys).expect(&by_index_keys);
        assert_eq!(by_index_keys_json["items"].as_array().unwrap().len(), 2);
        assert_eq!(by_index_keys_json["has_more"], false);

        let write = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("INSERT INTO posts (id) VALUES ('z')")
            ),
            "",
        );
        assert!(write.contains("only SELECT or SCAN"), "{write}");

        let other = http_body(
            addr,
            "GET",
            &format!(
                "/api/rows?table=posts&page=0&dql={}",
                query_encode("SCAN * FROM comments")
            ),
            "",
        );
        assert!(other.contains("read must use table posts"), "{other}");
        drop(guard);
    }

    #[test]
    fn table_browser_pages_a_thousand_numeric_ids() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let mut script =
            String::from("CREATE TABLE nums (gid STRING HASH KEY, id NUMBER RANGE KEY);");
        for chunk in (1..=1000).collect::<Vec<_>>().chunks(100) {
            let values = chunk
                .iter()
                .map(|id| format!("('g', {id})"))
                .collect::<Vec<_>>()
                .join(", ");
            script.push_str(&format!(" INSERT INTO nums (gid, id) VALUES {values};"));
        }
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(&script).unwrap());
        let created = http_body(addr, "POST", "/api/run", &body);
        assert!(created.contains("\"ok\":true"), "{created}");

        let dql = query_encode("SELECT * FROM nums WHERE gid = 'g'");
        let page_ids = |page: usize| {
            let body = http_body(
                addr,
                "GET",
                &format!("/api/rows?table=nums&page={page}&dql={dql}"),
                "",
            );
            let json: Value = serde_json::from_str(&body).unwrap_or_else(|_| panic!("{body}"));
            let ids: Vec<i64> = json["items"]
                .as_array()
                .unwrap_or_else(|| panic!("{body}"))
                .iter()
                .map(|item| {
                    item["id"]
                        .as_i64()
                        .or_else(|| item["id"].as_u64().map(|value| value as i64))
                        .or_else(|| item["id"].as_f64().map(|value| value as i64))
                        .unwrap_or_else(|| panic!("id {}", item["id"]))
                })
                .collect();
            (
                ids,
                json["has_more"].as_bool().unwrap(),
                json["dql"].as_str().unwrap().to_string(),
            )
        };

        let expect_page = |page: usize, has_more: bool| {
            let start = page * 50 + 1;
            let end = (start + 49).min(1000);
            let (ids, more, _) = page_ids(page);
            assert_eq!(ids, (start as i64..=end as i64).collect::<Vec<_>>());
            assert_eq!(more, has_more, "page {page}");
        };

        expect_page(0, true);
        expect_page(1, true);
        expect_page(10, true);
        expect_page(9, true);
        expect_page(19, false);
        expect_page(18, true);
        expect_page(0, true);
        let (_, _, dql_text) = page_ids(0);
        assert_eq!(dql_text, "SELECT * FROM nums WHERE gid = 'g' LIMIT 51");
        drop(guard);
    }

    #[test]
    fn query_results_round_trip_and_clear() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let created = http(addr, "POST", "/api/file", r#"{"path":"notes.dql"}"#);
        assert!(created.contains("HTTP/1.1 200"), "{created}");

        let missing = http(
            addr,
            "POST",
            "/api/query-results",
            r#"{"path":"notes.dql","query":"SELECT 1","result":{"ok":true}}"#,
        );
        assert!(missing.contains("HTTP/1.1 400"), "{missing}");
        assert!(missing.contains("success must be a boolean"), "{missing}");

        let saved = http_body(
            addr,
            "POST",
            "/api/query-results",
            r#"{"path":"notes.dql","query":"SELECT 1","success":true,"result":{"ok":true}}"#,
        );
        let saved_json: Value = serde_json::from_str(&saved).expect(&saved);
        assert_eq!(saved_json["query"], "SELECT 1");
        assert_eq!(saved_json["success"], true);
        assert_eq!(saved_json["result"]["ok"], true);

        let listed = http_body(addr, "GET", "/api/query-results?path=notes.dql", "");
        let listed_json: Value = serde_json::from_str(&listed).expect(&listed);
        assert_eq!(listed_json["results"].as_array().unwrap().len(), 1);
        assert_eq!(listed_json["results"][0]["query"], "SELECT 1");
        assert!(
            listed_json["results"][0]["id"]
                .as_str()
                .unwrap()
                .contains(':'),
            "{listed}"
        );

        let cleared = http_body(
            addr,
            "POST",
            "/api/query-results/clear",
            r#"{"path":"notes.dql"}"#,
        );
        let cleared_json: Value = serde_json::from_str(&cleared).expect(&cleared);
        assert_eq!(cleared_json["ok"], true);
        assert_eq!(cleared_json["paths"][0], "notes.dql");

        let empty = http_body(addr, "GET", "/api/query-results?path=notes.dql", "");
        let empty_json: Value = serde_json::from_str(&empty).expect(&empty);
        assert!(
            empty_json["results"].as_array().unwrap().is_empty(),
            "{empty}"
        );
        drop(guard);
    }

    #[test]
    fn mkdir_duplicate_move_and_delete() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());

        let made = http_body(addr, "POST", "/api/mkdir", r#"{"path":"queries"}"#);
        let made_json: Value = serde_json::from_str(&made).expect(&made);
        assert_eq!(made_json["path"], "queries");
        assert_eq!(made_json["kind"], "dir");
        assert!(root.path().join("queries").is_dir());

        let again = http(addr, "POST", "/api/mkdir", r#"{"path":"queries"}"#);
        assert!(again.contains("HTTP/1.1 400"), "{again}");
        assert!(again.contains("already exists"), "{again}");
        let escaped = http(addr, "POST", "/api/mkdir", r#"{"path":"../outside"}"#);
        assert!(escaped.contains("HTTP/1.1 400"), "{escaped}");

        let created = http(addr, "POST", "/api/file", r#"{"path":"queries/notes.dql"}"#);
        assert!(created.contains("HTTP/1.1 200"), "{created}");
        let copied = http_body(
            addr,
            "POST",
            "/api/duplicate",
            r#"{"path":"queries/notes.dql"}"#,
        );
        let copied_json: Value = serde_json::from_str(&copied).expect(&copied);
        assert_eq!(copied_json["path"], "queries/notes copy.dql");
        assert_eq!(copied_json["kind"], "file");
        assert!(root.path().join("queries/notes copy.dql").is_file());

        let busy = http(addr, "POST", "/api/delete", r#"{"path":"queries"}"#);
        assert!(busy.contains("HTTP/1.1 400"), "{busy}");
        assert!(busy.contains("not empty"), "{busy}");

        let moved = http_body(
            addr,
            "POST",
            "/api/move",
            r#"{"from":"queries/notes copy.dql","to":"kept.dql"}"#,
        );
        let moved_json: Value = serde_json::from_str(&moved).expect(&moved);
        assert_eq!(moved_json["path"], "kept.dql");
        assert_eq!(moved_json["kind"], "file");
        assert!(root.path().join("kept.dql").is_file());
        assert!(!root.path().join("queries/notes copy.dql").exists());

        let removed = http(
            addr,
            "POST",
            "/api/delete",
            r#"{"path":"queries/notes.dql"}"#,
        );
        assert!(removed.contains("HTTP/1.1 200"), "{removed}");
        assert!(removed.contains("\"ok\":true"), "{removed}");
        let folder = http(addr, "POST", "/api/delete", r#"{"path":"queries"}"#);
        assert!(folder.contains("HTTP/1.1 200"), "{folder}");
        let kept = http(addr, "POST", "/api/delete", r#"{"path":"kept.dql"}"#);
        assert!(kept.contains("HTTP/1.1 200"), "{kept}");
        assert!(!root.path().join("kept.dql").exists());
        assert!(!root.path().join("queries").exists());

        let missing = http(addr, "POST", "/api/delete", r#"{"path":"kept.dql"}"#);
        assert!(missing.contains("HTTP/1.1 400"), "{missing}");
        drop(guard);
    }

    #[test]
    fn events_stream_reports_a_file_and_a_tree_change() {
        let root = tempfile::tempdir().unwrap();
        let (addr, guard) = spawn_server(root.path().to_path_buf());
        let token = server_token(addr);
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        write!(
            stream,
            "GET /api/events?id=web-window-a HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut buf = String::new();
        let mut bytes = [0u8; 1024];
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("event: presence")
        });
        assert!(buf.contains("event: presence"), "{buf}");
        assert!(buf.contains("\"label\":\"W1\""), "{buf}");
        let created = http(addr, "POST", "/api/file", r#"{"path":"notes.dql"}"#);
        assert!(created.contains("notes.dql"), "{created}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("event: file") && text.contains("event: tree")
        });
        assert!(
            buf.contains("event: tree"),
            "tree event was not announced\n{buf}"
        );
        assert!(buf.contains("event: file"), "{buf}");
        assert!(buf.contains("notes.dql"), "{buf}");
        std::fs::write(root.path().join("notes.dql"), "from outside\n").unwrap();
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.matches("event: file").count() >= 2
        });
        assert!(
            buf.matches("event: file").count() >= 2,
            "outside write was not announced\n{buf}"
        );
        let created_table = http_body(
            addr,
            "POST",
            "/api/run",
            r#"{"dql":"CREATE TABLE shelf (id STRING HASH KEY);"}"#,
        );
        assert!(created_table.contains("\"ok\":true"), "{created_table}");
        read_until(&mut stream, &mut buf, &mut bytes, |text| {
            text.contains("event: tables")
        });
        assert!(
            buf.contains("event: tables\ndata: {\"tables\":[\"shelf\"]}"),
            "tables event did not name the table\n{buf}"
        );
        drop(stream);
        drop(guard);
    }

    fn read_until(
        stream: &mut TcpStream,
        buf: &mut String,
        bytes: &mut [u8],
        done: impl Fn(&str) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline && !done(buf) {
            match stream.read(bytes) {
                Ok(0) => return,
                Ok(n) => buf.push_str(&String::from_utf8_lossy(&bytes[..n])),
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(err) => panic!("{err}"),
            }
        }
    }

    fn spawn_server(root: PathBuf) -> (SocketAddr, ServerGuard) {
        spawn_server_mcp(root, false)
    }

    fn spawn_server_mcp(root: PathBuf, mcp: bool) -> (SocketAddr, ServerGuard) {
        let token = crate::api_token::load().unwrap();
        let app = Arc::new(App::new(
            Session::new_memory_headless("memory"),
            root,
            mcp,
            token.clone(),
        ));
        super::super::events::start(&app);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _ = app.mcp_url.set(format!("http://{addr}/mcp"));
        test_tokens()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(addr, token);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let thread = thread::spawn(move || accept(app, listener, stop_thread));
        (
            addr,
            ServerGuard {
                stop,
                addr,
                thread: Some(thread),
            },
        )
    }

    fn test_tokens() -> &'static Mutex<std::collections::HashMap<SocketAddr, String>> {
        use std::sync::OnceLock;
        static TOKENS: OnceLock<Mutex<std::collections::HashMap<SocketAddr, String>>> =
            OnceLock::new();
        TOKENS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
    }

    fn server_token(addr: SocketAddr) -> String {
        test_tokens()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(&addr)
            .cloned()
            .expect("server token")
    }

    fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> String {
        let token = server_token(addr);
        http_bearer(addr, method, path, body, "", Some(&token))
    }

    #[test]
    fn the_page_bundles_its_font_and_links_nothing_from_outside() {
        assert_eq!(
            content_type("vendor/fonts/noto-sans-latin-wght-normal.woff2"),
            "font/woff2"
        );
        let index = ASSETS.get_file("index.html").unwrap();
        let page = index.contents_utf8().unwrap();
        assert!(page.contains(r#"href="/vendor/fonts/noto-sans.css""#));
        assert!(!page.contains("http://"), "index.html links an outside URL");
        assert!(
            !page.contains("https://"),
            "index.html links an outside URL"
        );
        let css = ASSETS.get_file("vendor/fonts/noto-sans.css").unwrap();
        for line in css.contents_utf8().unwrap().lines() {
            if let Some(rest) = line.trim().strip_prefix("src: url(\"/") {
                let path = rest.split('"').next().unwrap();
                assert!(ASSETS.get_file(path).is_some(), "missing font file {path}");
            }
        }
    }

    fn http_anon(addr: SocketAddr, method: &str, path: &str, body: &str) -> String {
        http_bearer(addr, method, path, body, "", None)
    }

    fn http_origin(addr: SocketAddr, method: &str, path: &str, body: &str, origin: &str) -> String {
        let token = server_token(addr);
        http_bearer(addr, method, path, body, origin, Some(&token))
    }

    fn http_bearer(
        addr: SocketAddr,
        method: &str,
        path: &str,
        body: &str,
        origin: &str,
        bearer: Option<&str>,
    ) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let origin = if origin.is_empty() {
            String::new()
        } else {
            format!("Origin: {origin}\r\n")
        };
        let authorization = match bearer {
            Some(token) => format!("Authorization: Bearer {token}\r\n"),
            None => String::new(),
        };
        write!(
            stream,
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{origin}{authorization}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }

    /// Response body with chunk framing removed. `/api/run` flushes each NDJSON
    /// event as its own chunk, so a raw read can split a JSON string.
    fn query_encode(text: &str) -> String {
        let mut out = String::new();
        for byte in text.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char);
                }
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        out
    }

    fn http_body(addr: SocketAddr, method: &str, path: &str, body: &str) -> String {
        let raw = http(addr, method, path, body);
        let Some((headers, rest)) = raw.split_once("\r\n\r\n") else {
            return raw;
        };
        if !headers
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            return rest.to_string();
        }
        let bytes = rest.as_bytes();
        let mut index = 0;
        let mut out = Vec::new();
        while index < bytes.len() {
            let Some(line_end) = bytes[index..].windows(2).position(|pair| pair == b"\r\n") else {
                break;
            };
            let Ok(line) = std::str::from_utf8(&bytes[index..index + line_end]) else {
                break;
            };
            let Ok(size) = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
            else {
                break;
            };
            index += line_end + 2;
            if size == 0 {
                break;
            }
            let end = index + size;
            if end + 2 > bytes.len() || &bytes[end..end + 2] != b"\r\n" {
                break;
            }
            out.extend_from_slice(&bytes[index..end]);
            index = end + 2;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}
