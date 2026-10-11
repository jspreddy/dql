//! HTTP API and static files for the web UI.

use std::fs::{self, File};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use include_dir::{include_dir, Dir};
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use super::files::{self, PathError};
use super::shape::{
    changes_schema, column_order, explain_index, list_tables, parse_table_shape, read_query,
    safe_table_name, split_statements, Column, ExecText, TableShape, PAGE_SIZE,
};
use crate::serve::protocol::ServeEnvelope;
use crate::session::Session;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../../dqlrs-web/static");

struct WebState {
    session: Session,
    shapes: std::collections::HashMap<String, TableShape>,
}

struct App {
    root: PathBuf,
    endpoint: String,
    state: Mutex<WebState>,
}

pub fn serve(
    session: Session,
    root: PathBuf,
    listen: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let app = Arc::new(App::new(session, root));
    let server = Server::http(listen)?;
    let bound = server.server_addr().to_ip().unwrap_or(listen);
    println!("DQLRS Web  http://{bound}");
    println!("workspace  {}", app.root.display());
    println!("endpoint   {}", app.endpoint);
    let _ = io::stdout().flush();
    accept(app, &server);
    Ok(())
}

fn accept(app: Arc<App>, server: &Server) {
    for request in server.incoming_requests() {
        dispatch(&app, request);
    }
}

impl App {
    fn new(session: Session, root: PathBuf) -> Self {
        let endpoint = match &session.local_endpoint {
            Some((host, port)) => format!("{host}:{port}"),
            None => session.region.clone(),
        };
        Self {
            root,
            endpoint,
            state: Mutex::new(WebState {
                session,
                shapes: std::collections::HashMap::new(),
            }),
        }
    }
}

fn dispatch(app: &Arc<App>, request: Request) {
    let method = request.method().clone();
    let url = request.url().to_string();
    let (path, query) = split_url(&url);
    let handled = match method {
        Method::Get => dispatch_get(app, request, path, query),
        Method::Post => dispatch_post(app, request, path),
        Method::Put => dispatch_put(app, request, path),
        _ => respond_json(request, 404, &json!({"ok": false, "error": "not found"})),
    };
    if let Err(err) = handled {
        eprintln!("dqlrs web: {err}");
    }
}

fn dispatch_get(app: &App, request: Request, path: &str, query: &str) -> io::Result<()> {
    match path {
        "/api/config" => respond_json(
            request,
            200,
            &json!({
                "endpoint": app.endpoint,
                "root": app.root.display().to_string(),
                "page_size": PAGE_SIZE,
            }),
        ),
        "/api/tree" => match files::tree(&app.root) {
            Ok(node) => respond_json(
                request,
                200,
                &serde_json::to_value(node).unwrap_or(Value::Null),
            ),
            Err(err) => respond_error(request, 400, &err),
        },
        "/api/file" => {
            let relative = query_value(query, "path");
            match files::read_text(&app.root, &relative) {
                Ok(text) => respond_json(request, 200, &json!({"path": relative, "text": text})),
                Err(err) => respond_error(request, 400, &err),
            }
        }
        "/api/tables" => {
            let pattern = query_value(query, "pattern");
            let refresh = query_flag(query, "refresh");
            let mut state = lock_state(app);
            let mut shapes = std::mem::take(&mut state.shapes);
            let listed = list_tables(&pattern, refresh, &mut shapes, |command| {
                ExecText::from_envelope(&state.session.execute_for_serve(command))
            });
            state.shapes = shapes;
            match listed {
                Ok(listed) => {
                    drop(state);
                    respond_json(
                        request,
                        200,
                        &serde_json::to_value(listed).unwrap_or(Value::Null),
                    )
                }
                Err(message) => {
                    drop(state);
                    respond_json(request, 502, &json!({"ok": false, "error": message}))
                }
            }
        }
        "/api/rows" => {
            let name = query_value(query, "table");
            let page = query_value(query, "page")
                .parse::<i64>()
                .unwrap_or(0)
                .max(0) as usize;
            let mut state = lock_state(app);
            match table_rows(&mut state, &name, page) {
                Ok(payload) => {
                    drop(state);
                    respond_json(request, 200, &payload)
                }
                Err(message) => {
                    drop(state);
                    respond_json(request, 502, &json!({"ok": false, "error": message}))
                }
            }
        }
        _ => serve_static(request, path),
    }
}

fn dispatch_post(app: &Arc<App>, mut request: Request, path: &str) -> io::Result<()> {
    if path == "/api/run" {
        let body = read_body(&mut request);
        let dql = body
            .get("dql")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return handle_run(Arc::clone(app), request, dql);
    }
    let body = read_body(&mut request);
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    match path {
        "/api/file" => match files::create_file(&app.root, relative) {
            Ok(path) => respond_json(request, 200, &json!({"path": path, "text": ""})),
            Err(err) => respond_error(request, 400, &err),
        },
        "/api/mkdir" => match files::make_dir(&app.root, relative) {
            Ok(path) => respond_json(request, 200, &json!({"path": path, "kind": "dir"})),
            Err(err) => respond_error(request, 400, &err),
        },
        "/api/duplicate" => match files::duplicate_path(&app.root, relative) {
            Ok((path, kind)) => respond_json(request, 200, &json!({"path": path, "kind": kind})),
            Err(err) => respond_error(request, 400, &err),
        },
        "/api/move" => {
            let from = body.get("from").and_then(Value::as_str).unwrap_or("");
            let to = body.get("to").and_then(Value::as_str).unwrap_or("");
            match files::move_path(&app.root, from, to) {
                Ok((path, kind)) => {
                    respond_json(request, 200, &json!({"path": path, "kind": kind}))
                }
                Err(err) => respond_error(request, 400, &err),
            }
        }
        "/api/delete" => match files::delete_path(&app.root, relative) {
            Ok(()) => respond_json(request, 200, &json!({"ok": true})),
            Err(err) => respond_error(request, 400, &err),
        },
        _ => respond_json(request, 404, &json!({"ok": false, "error": "not found"})),
    }
}

fn dispatch_put(app: &App, mut request: Request, path: &str) -> io::Result<()> {
    if path != "/api/file" {
        return respond_json(request, 404, &json!({"ok": false, "error": "not found"}));
    }
    let body = read_body(&mut request);
    let relative = body.get("path").and_then(Value::as_str).unwrap_or("");
    let text = body.get("text").and_then(Value::as_str).unwrap_or("");
    match files::write_text(&app.root, relative, text) {
        Ok(()) => respond_json(request, 200, &json!({"ok": true})),
        Err(err) => respond_error(request, 400, &err),
    }
}

fn handle_run(app: Arc<App>, request: Request, dql: String) -> io::Result<()> {
    let statements = split_statements(&dql);
    if statements.is_empty() {
        return respond_json(
            request,
            502,
            &json!({"ok": false, "error": "nothing to run"}),
        );
    }
    let (reader, writer) = io::pipe()?;
    thread::spawn(move || run_script(app, statements, writer));
    let response = Response::new(
        StatusCode(200),
        vec![
            header(b"Content-Type", b"application/x-ndjson; charset=utf-8"),
            header(b"Cache-Control", b"no-store"),
        ],
        reader,
        None,
        None,
    );
    request.respond(response)
}

fn run_script(app: Arc<App>, statements: Vec<String>, writer: impl Write + Send + 'static) {
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
        let envelope = state.session.execute_for_serve(statement);
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
        }
        let result = public_result(statement, &envelope, columns);
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

fn public_result(statement: &str, envelope: &ServeEnvelope, columns: Option<Vec<Column>>) -> Value {
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
    });
    if envelope.ok && envelope.kind == "items" {
        if let Some(columns) = columns {
            value["columns"] = serde_json::to_value(columns).unwrap_or(Value::Null);
        }
    }
    value
}

fn serve_static(request: Request, path: &str) -> io::Result<()> {
    let Some(relative) = static_relative(path) else {
        return respond_json(request, 404, &json!({"ok": false, "error": "not found"}));
    };
    if let Some(root) = disk_static() {
        let file = root.join(&relative);
        if let Ok(canon) = file.canonicalize() {
            if canon.starts_with(&root) && canon.is_file() {
                let response = Response::from_file(File::open(&canon)?)
                    .with_header(header(b"Content-Type", content_type(&relative).as_bytes()))
                    .with_header(header(b"Cache-Control", b"no-store"));
                return request.respond(response);
            }
        }
        return respond_json(request, 404, &json!({"ok": false, "error": "not found"}));
    }
    let Some(file) = ASSETS.get_file(&relative) else {
        return respond_json(request, 404, &json!({"ok": false, "error": "not found"}));
    };
    let response = Response::from_data(file.contents().to_vec())
        .with_header(header(b"Content-Type", content_type(&relative).as_bytes()))
        .with_header(header(b"Cache-Control", b"no-store"));
    request.respond(response)
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

fn read_body(request: &mut Request) -> Value {
    let Some(len) = request.body_length() else {
        return Value::Object(Default::default());
    };
    if len == 0 || len > 2_000_000 {
        return Value::Object(Default::default());
    }
    let mut buf = vec![0; len];
    if request.as_reader().read_exact(&mut buf).is_err() {
        return Value::Object(Default::default());
    }
    match serde_json::from_slice(&buf) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Default::default()),
    }
}

fn respond_error(request: Request, status: u16, err: &PathError) -> io::Result<()> {
    respond_json(
        request,
        status,
        &json!({"ok": false, "error": err.to_string()}),
    )
}

fn respond_json(request: Request, status: u16, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let response = Response::from_data(bytes)
        .with_status_code(StatusCode(status))
        .with_header(header(b"Content-Type", b"application/json; charset=utf-8"))
        .with_header(header(b"Cache-Control", b"no-store"));
    request.respond(response)
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).expect("static header")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    #[test]
    fn serves_config_files_and_a_query() {
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(App::new(
            Session::new_memory_headless("memory"),
            root.path().to_path_buf(),
        ));
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let server = Arc::new(server);
        let guard = ServerGuard {
            server: Arc::clone(&server),
            thread: Some(thread::spawn({
                let app = Arc::clone(&app);
                move || accept(app, &server)
            })),
        };

        let config = http(addr, "GET", "/api/config", "");
        assert!(config.contains("\"page_size\":50"));
        assert!(config.contains("\"endpoint\":\"memory\""));

        let created = http(addr, "POST", "/api/file", r#"{"path":"notes.dql"}"#);
        assert!(created.contains("notes.dql"));
        assert!(root.path().join("notes.dql").is_file());

        let page = http(addr, "GET", "/", "");
        assert!(page.contains("DQLRS Web"));

        let script = "CREATE TABLE shelf (id STRING HASH KEY); INSERT INTO shelf (id, apple) VALUES ('a', 'p'); SELECT * FROM shelf WHERE id = 'a';";
        let body = format!("{{\"dql\":{}}}", serde_json::to_string(script).unwrap());
        let ran = http(addr, "POST", "/api/run", &body);
        assert!(ran.contains("\"event\":\"result\""), "{ran}");
        assert!(ran.contains("\"apple\""), "{ran}");
        assert!(ran.contains("\"a\""), "{ran}");
        drop(guard);
    }

    struct ServerGuard {
        server: Arc<Server>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Drop for ServerGuard {
        fn drop(&mut self) {
            self.server.unblock();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut buf = String::new();
        let _ = stream.read_to_string(&mut buf);
        buf
    }
}
