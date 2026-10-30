//! HTTP API and static files for the web UI.
//!
//! Each connection answers one request completely before reading the next.
//! The editor loads many scripts at once; overlapping keep-alive writes were
//! leaving some of those responses unsent.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use include_dir::{include_dir, Dir};
use serde_json::{json, Value};

use super::files::{self, PathError};
use super::presence::{self, Kind};
use super::query_cache;
use super::shape::{
    changes_schema, column_order, explain_index, parse_table_shape, read_query, safe_table_name,
    split_statements, Column, ExecText, ListedTable, TableShape, PAGE_SIZE,
};
use crate::serve::protocol::{envelope_from_error, ServeEnvelope};
use crate::session::Session;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../../dqlrs-web/static");

const HEADER_LIMIT: usize = 64 * 1024;
const BODY_LIMIT: usize = 2_000_000;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

struct WebState {
    session: Session,
    shapes: std::collections::HashMap<String, TableShape>,
    table_catalog: Option<Vec<ListedTable>>,
    mcp_initialized: bool,
}

struct App {
    root: PathBuf,
    endpoint: String,
    state: Mutex<WebState>,
    cancel: dql_engine::CancelFlag,
    identity: Mutex<Option<dql_engine::CallerIdentity>>,
    presence: presence::Book,
    mcp: bool,
    token: String,
}

struct Incoming {
    method: String,
    path: String,
    body: Vec<u8>,
    origin: Option<String>,
    mcp_session: Option<String>,
    authorization: Option<String>,
    keep_alive: bool,
    overflow: bool,
}

enum Reply {
    Json(u16, Value),
    Bytes(u16, &'static str, Vec<u8>),
    Ndjson(Arc<App>, Vec<String>),
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
    let token = crate::api_token::mint()?;
    let app = Arc::new(App::new(session, root, mcp, token));
    let listener = TcpListener::bind(listen)?;
    let bound = listener.local_addr().unwrap_or(listen);
    println!("DQLRS Web  http://{bound}");
    if load_tables {
        begin_table_load(&app);
        println!("tables     loading in the background");
    }
    if mcp {
        let url = format!("http://{bound}/mcp");
        println!("MCP        {url}");
        println!("workspace  {}", app.root.display());
        println!("endpoint   {}", app.endpoint);
        println!();
        println!("{}", crate::mcp::http_client_config(&url, &app.token));
    } else {
        println!("{}", crate::mcp::web_mcp_hint(&bound.to_string()));
        println!("workspace  {}", app.root.display());
        println!("endpoint   {}", app.endpoint);
    }
    let _ = io::stdout().flush();
    eprintln!("{}", crate::api_token::stderr_line(&app.token));
    let _ = io::stderr().flush();
    accept(app, listener, Arc::new(AtomicBool::new(false)));
    Ok(())
}

fn begin_table_load(app: &Arc<App>) {
    let disk = {
        let state = lock_state(app);
        state.session.table_disk.clone()
    };
    if let Some(cache) = disk {
        cache.start_load();
        return;
    }
    let app = Arc::clone(app);
    thread::spawn(move || load_session_tables(app));
}

fn load_session_tables(app: Arc<App>) {
    let names = {
        let mut state = lock_state(&app);
        match state.session.engine.cached_table_names(true) {
            Ok(names) => names,
            Err(err) => {
                eprintln!("dqlrs: load tables: {err}");
                return;
            }
        }
    };
    for name in names {
        let mut state = lock_state(&app);
        match state.session.engine.describe(&name, false) {
            Ok(_) => {}
            Err(err) => eprintln!("dqlrs: load tables: {name}: {err}"),
        }
    }
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
                    eprintln!("dqlrs web: {err}");
                }
            }
        });
    }
}

impl App {
    fn new(session: Session, root: PathBuf, mcp: bool, token: String) -> Self {
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
                shapes: std::collections::HashMap::new(),
                table_catalog: None,
                mcp_initialized: false,
            }),
            cancel,
            identity: Mutex::new(None),
            presence: presence::Book::new(),
            mcp,
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
    let path = target.split('?').next().unwrap_or("/");
    let mut content_length = None;
    let mut connection = String::new();
    let mut origin = None;
    let mut mcp_session = None;
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
            body: Vec::new(),
            origin,
            mcp_session,
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
            body: Vec::new(),
            origin,
            mcp_session,
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
        body,
        origin,
        mcp_session,
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
        if !app.mcp {
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
    match request.method.as_str() {
        "GET" => dispatch_get(app, path),
        "POST" => dispatch_post(app, path, &request.body),
        "PUT" => dispatch_put(app, path, &request.body),
        _ => Reply::Json(404, json!({"ok": false, "error": "not found"})),
    }
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

fn dispatch_get(app: &App, path: &str) -> Reply {
    match path {
        "/api/config" => Reply::Json(200, config_payload(app)),
        "/api/identity" => Reply::Json(200, identity_reply(app)),
        "/api/tree" => match files::tree(&app.root) {
            Ok(node) => Reply::Json(200, serde_json::to_value(node).unwrap_or(Value::Null)),
            Err(err) => json_error(400, &err),
        },
        "/api/presence" => Reply::Json(200, app.presence.snapshot()),
        "/api/table-cache" => {
            let state = lock_state(app);
            let hint = crate::meta::ls::web_table_cache_status(&state.session);
            Reply::Json(200, serde_json::to_value(hint).unwrap_or(Value::Null))
        }
        _ => serve_static(app, path),
    }
}

fn dispatch_mcp(app: &App, request: &Incoming) -> Reply {
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
    let mcp_id = request
        .mcp_session
        .as_deref()
        .and_then(presence::valid_mcp_id)
        .unwrap_or("mcp");
    app.presence.touch(mcp_id, Kind::Mcp);
    let text = String::from_utf8_lossy(&request.body);
    let mut state = lock_state(app);
    let WebState {
        session,
        mcp_initialized,
        ..
    } = &mut *state;
    match crate::mcp::exchange(mcp_initialized, session, &app.root, text.trim()) {
        Some(response) => {
            let value = serde_json::from_str(&response).unwrap_or_else(|_| {
                json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32603, "message": "bad response" }
                })
            });
            let status = if value["error"]["code"] == -32700 {
                400
            } else {
                200
            };
            Reply::Json(status, value)
        }
        None => Reply::Bytes(202, "application/json", Vec::new()),
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
    if path == "/api/run" {
        let parsed = parse_json(body);
        let dql = parsed
            .get("dql")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return handle_run(Arc::clone(app), dql);
    }
    let body = parse_json(body);
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    match path {
        "/api/presence" => {
            let id = body.get("id").and_then(Value::as_str).unwrap_or("");
            let Some(id) = presence::valid_web_id(id) else {
                return Reply::Json(400, json!({"ok": false, "error": "client id is invalid"}));
            };
            app.presence.touch(id, Kind::Web);
            Reply::Json(200, app.presence.snapshot())
        }
        "/api/file" => match files::create_file(&app.root, relative) {
            Ok(path) => Reply::Json(200, json!({"path": path, "text": ""})),
            Err(err) => json_error(400, &err),
        },
        "/api/file/read" => read_file_reply(app, json_str(&body, "path")),
        "/api/query-results/list" => list_query_results_reply(app, json_str(&body, "path")),
        "/api/search" => search_reply(app, json_str(&body, "q")),
        "/api/tables" => tables_reply(
            app,
            json_str(&body, "pattern"),
            json_bool(&body, "refresh"),
            json_bool(&body, "async"),
        ),
        "/api/rows" => rows_reply(app, json_str(&body, "table"), json_page(&body)),
        "/api/mkdir" => match files::make_dir(&app.root, relative) {
            Ok(path) => Reply::Json(200, json!({"path": path, "kind": "dir"})),
            Err(err) => json_error(400, &err),
        },
        "/api/duplicate" => match files::duplicate_path(&app.root, relative) {
            Ok((path, kind)) => Reply::Json(200, json!({"path": path, "kind": kind})),
            Err(err) => json_error(400, &err),
        },
        "/api/move" => {
            let from = body.get("from").and_then(Value::as_str).unwrap_or("");
            let to = body.get("to").and_then(Value::as_str).unwrap_or("");
            match files::move_path(&app.root, from, to) {
                Ok((path, kind)) => Reply::Json(200, json!({"path": path, "kind": kind})),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/delete" => match files::delete_path(&app.root, relative) {
            Ok(()) => Reply::Json(200, json!({"ok": true})),
            Err(err) => json_error(400, &err),
        },
        "/api/query-results" => save_query_result(&app.root, &body),
        "/api/query-results/clear" => match query_cache::clear_at(&app.root, relative) {
            Ok(paths) => Reply::Json(200, json!({"ok": true, "paths": paths})),
            Err(err) => json_error(400, &err),
        },
        "/api/lint" => {
            let text = body.get("text").and_then(Value::as_str).unwrap_or("");
            let mut state = lock_state(app);
            let report = super::lint::lint_script(&mut state.session, text);
            Reply::Json(200, serde_json::to_value(report).unwrap_or(Value::Null))
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
        Ok(files::SaveFile::Stored(file)) => Reply::Json(
            200,
            json!({"ok": true, "path": file.path, "revision": file.revision}),
        ),
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

fn json_str<'a>(body: &'a Value, key: &str) -> &'a str {
    body.get(key).and_then(Value::as_str).unwrap_or("")
}

fn json_bool(body: &Value, key: &str) -> bool {
    body.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn json_page(body: &Value) -> usize {
    body.get("page")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0) as usize
}

fn read_file_reply(app: &App, relative: &str) -> Reply {
    match files::read_file(&app.root, relative) {
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

fn list_query_results_reply(app: &App, relative: &str) -> Reply {
    match query_cache::list(&app.root, relative) {
        Ok(results) => Reply::Json(200, json!({ "results": results })),
        Err(err) => json_error(400, &err),
    }
}

fn search_reply(app: &App, query: &str) -> Reply {
    let mut state = lock_state(app);
    let results = super::search::search(&mut state.session, &app.root, query);
    Reply::Json(200, serde_json::to_value(results).unwrap_or(Value::Null))
}

fn tables_reply(app: &App, pattern: &str, refresh: bool, background: bool) -> Reply {
    let mut state = lock_state(app);
    match crate::meta::ls::web_table_list(&mut state.session, pattern, refresh, background) {
        Ok(listed) => Reply::Json(200, serde_json::to_value(listed).unwrap_or(Value::Null)),
        Err(message) => Reply::Json(502, json!({"ok": false, "error": message})),
    }
}

fn rows_reply(app: &App, name: &str, page: usize) -> Reply {
    let mut state = lock_state(app);
    match table_rows(&mut state, name, page) {
        Ok(payload) => Reply::Json(200, payload),
        Err(message) => Reply::Json(502, json!({"ok": false, "error": message})),
    }
}

fn save_query_result(root: &Path, body: &Value) -> Reply {
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    let query = body.get("query").and_then(Value::as_str).unwrap_or("");
    let Some(success) = body.get("success").and_then(Value::as_bool) else {
        return Reply::Json(
            400,
            json!({"ok": false, "error": "success must be a boolean"}),
        );
    };
    let result = body.get("result").cloned().unwrap_or(Value::Null);
    match query_cache::save(root, relative, query, result, success) {
        Ok(stored) => Reply::Json(200, serde_json::to_value(stored).unwrap_or(Value::Null)),
        Err(err) => json_error(400, &err),
    }
}

fn handle_run(app: Arc<App>, dql: String) -> Reply {
    let statements = split_statements(&dql);
    if statements.is_empty() {
        return Reply::Json(502, json!({"ok": false, "error": "nothing to run"}));
    }
    Reply::Ndjson(app, statements)
}

fn run_script(app: Arc<App>, statements: Vec<String>, writer: impl Write + Send + 'static) {
    app.cancel.clear();
    let writer = Arc::new(Mutex::new(writer));
    let mut state = lock_state(&app);
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
        let envelope = state.session.execute_for_serve(statement);
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
        if envelope.ok && changes_schema(statement) {
            state.shapes.clear();
            state.table_catalog = None;
        }
        let result = public_result(
            statement,
            &envelope,
            columns,
            elapsed_ms,
            read_units,
            write_units,
        );
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

fn table_rows(state: &mut WebState, name: &str, page: usize) -> Result<Value, String> {
    if !safe_table_name(name) {
        return Err(format!("unsupported table name {name:?}"));
    }
    let limit = PAGE_SIZE.saturating_mul(page.saturating_add(1));
    let envelope = state
        .session
        .execute_for_serve(&format!("SCAN * FROM {name} LIMIT {limit}"));
    if !envelope.ok {
        return Err(ExecText::from_envelope(&envelope).failure());
    }
    let items = envelope.items.unwrap_or_default();
    let start = PAGE_SIZE.saturating_mul(page);
    let end = (start + PAGE_SIZE).min(items.len());
    let window = items.get(start..end).unwrap_or(&[]);
    let shape = load_shape(state, name);
    let columns = column_order(window, shape.as_ref(), None, None);
    Ok(json!({
        "name": name,
        "page": page,
        "page_size": PAGE_SIZE,
        "items": window,
        "columns": columns,
        "has_more": items.len() >= limit,
    }))
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
            )?;
            Ok(keep_alive)
        }
        Reply::Bytes(status, content_type, bytes) => {
            write_bytes(stream, status, content_type, &bytes, keep_alive)?;
            Ok(keep_alive)
        }
        Reply::Ndjson(app, statements) => {
            write_ndjson(stream, app, statements)?;
            Ok(false)
        }
    }
}

fn write_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    keep_alive: bool,
) -> io::Result<()> {
    let connection = if keep_alive { "keep-alive" } else { "close" };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nConnection: {connection}\r\n\r\n",
        reason = reason(status),
        len = body.len(),
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn write_ndjson(stream: &mut TcpStream, app: Arc<App>, statements: Vec<String>) -> io::Result<()> {
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
    thread::spawn(move || run_script(app, statements, writer));
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
        let file = http(
            addr,
            "POST",
            "/api/file/read",
            r#"{"path":"queries/from-mcp.dql"}"#,
        );
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

        let read = http(addr, "POST", "/api/file/read", r#"{"path":"notes.dql"}"#);
        let revision = json_string_field(&read, "revision");
        let wrote = http(
            addr,
            "PUT",
            "/api/file",
            &format!(r#"{{"path":"notes.dql","text":"from-mcp\n","base":"{revision}"}}"#),
        );
        assert!(wrote.contains("HTTP/1.1 200"), "{wrote}");
        let again = http(addr, "POST", "/api/file/read", r#"{"path":"notes.dql"}"#);
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
        let still = http(addr, "POST", "/api/file/read", r#"{"path":"notes.dql"}"#);
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
    fn load_tables_describes_every_memory_table() {
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(App::new(
            crate::session::Session::new_memory_headless("memory"),
            root.path().to_path_buf(),
            false,
            crate::api_token::mint().unwrap(),
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
        load_session_tables(Arc::clone(&app));
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

    fn spawn_server(root: PathBuf) -> (SocketAddr, ServerGuard) {
        spawn_server_mcp(root, false)
    }

    fn spawn_server_mcp(root: PathBuf, mcp: bool) -> (SocketAddr, ServerGuard) {
        let token = crate::api_token::mint().unwrap();
        let app = Arc::new(App::new(
            Session::new_memory_headless("memory"),
            root,
            mcp,
            token.clone(),
        ));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
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
