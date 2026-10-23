//! MCP tools for Cursor and Claude.
//!
//! `dqlrs --mcp` speaks newline-delimited JSON-RPC on stdio. `dqlrs --web --mcp`
//! serves the same tools at `/mcp` on the web host, using that process's session
//! and workspace. Logs stay off stdout.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::meta::ls::web_table_list;
use crate::session::Session;
use crate::web::files::{self, PathError};
use crate::web::search::search;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "dqlrs";

struct ToolError {
    message: String,
}

impl ToolError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub struct McpServer {
    session: Session,
    root: PathBuf,
    initialized: bool,
}

impl McpServer {
    pub fn new(session: Session, root: PathBuf) -> Self {
        Self {
            session,
            root,
            initialized: false,
        }
    }

    /// Handle one JSON-RPC message. Notifications return `None`.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        exchange(&mut self.initialized, &mut self.session, &self.root, line)
    }
}

/// Handle one JSON-RPC message against a shared session and workspace.
///
/// Notifications return `None`. The web server uses this so `/mcp` and `/api`
/// share one connection.
pub fn exchange(
    initialized: &mut bool,
    session: &mut Session,
    root: &Path,
    line: &str,
) -> Option<String> {
    let message: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => {
            return Some(rpc_error(Value::Null, -32700, "parse error"));
        }
    };
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = dispatch(initialized, session, root, method, &params);
    Some(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}).to_string(),
        Err((code, message)) => rpc_error(id, code, &message),
    })
}

fn dispatch(
    initialized: &mut bool,
    session: &mut Session,
    root: &Path,
    method: &str,
    params: &Value,
) -> Result<Value, (i32, String)> {
    if method != "initialize" && method != "ping" && !*initialized {
        return Err((-32002, "server not initialized".to_string()));
    }
    match method {
        "initialize" => Ok(initialize(initialized, params)),
        "ping" => Ok(json!({})),
        "logging/setLevel" => Ok(json!({})),
        "resources/list" => Ok(json!({"resources": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => call_tool(session, root, params),
        _ => Err((-32601, format!("unknown method {method}"))),
    }
}

fn initialize(initialized: &mut bool, params: &Value) -> Value {
    *initialized = true;
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Call help before writing DQL you are unsure of. Omit topic for the command list, or pass a topic such as select. Tools read and write the .dql workspace passed to dqlrs --mcp --dir, the same folder dqlrs --web uses. global_search finds table names, cached columns and indexes, and queries in files. lint_query checks a script, and lint_query_file checks a workspace .dql file, for parse errors and for queries the engine would reject, without running them or fetching a missing definition. list_tables and describe_tables use the shared table cache for this connection. create_query_file, read_query_file, and update_query_file change query files on disk."
    })
}

fn call_tool(session: &mut Session, root: &Path, params: &Value) -> Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires a name".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match name {
        "help" => tool_value(help_tool(&arguments)),
        "global_search" => tool_value(global_search(session, root, &arguments)),
        "lint_query" => tool_value(lint_query(session, root, &arguments)),
        "lint_query_file" => tool_value(lint_query_file(session, root, &arguments)),
        "list_tables" => tool_value(list_tables(session, &arguments)),
        "describe_tables" => tool_value(describe_tables(session, &arguments)),
        "list_query_files" => tool_value(list_query_files(root)),
        "create_query_file" => tool_value(create_query_file(root, &arguments)),
        "read_query_file" => tool_value(read_query_file(root, &arguments)),
        "update_query_file" => tool_value(update_query_file(root, &arguments)),
        _ => Err((-32602, format!("unknown tool {name}"))),
    }
}

fn help_tool(arguments: &Value) -> Result<Value, ToolError> {
    let topic = match arguments.get("topic") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => {
            let topic = text.trim().trim_end_matches(';').trim();
            if topic.is_empty() {
                None
            } else {
                Some(topic)
            }
        }
        Some(_) => return Err(ToolError::new("topic must be a string")),
    };
    let page = dql_engine::help_page(topic).map_err(ToolError::new)?;
    let mut value = page.to_value();
    value["text"] = json!(page.plain());
    Ok(value)
}

fn lint_query(session: &mut Session, root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let text = match arguments.get("text").and_then(Value::as_str) {
        Some(text) => text.to_string(),
        None => read_query_text(root, arguments)?,
    };
    lint_report(session, &text)
}

fn lint_query_file(
    session: &mut Session,
    root: &Path,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let text = read_query_text(root, arguments)?;
    lint_report(session, &text)
}

fn read_query_text(root: &Path, arguments: &Value) -> Result<String, ToolError> {
    let path = required_str(arguments, "path")?;
    let path = files::normalized_dql(root, path).map_err(path_error)?;
    files::read_text(root, &path).map_err(path_error)
}

fn lint_report(session: &mut Session, text: &str) -> Result<Value, ToolError> {
    let report = crate::web::lint::lint_script(session, text);
    serde_json::to_value(report).map_err(|err| ToolError::new(err.to_string()))
}

fn global_search(
    session: &mut Session,
    root: &Path,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let query = required_str(arguments, "query")?;
    let results = search(session, root, query);
    serde_json::to_value(results).map_err(|err| ToolError::new(err.to_string()))
}

fn list_tables(session: &mut Session, arguments: &Value) -> Result<Value, ToolError> {
    let pattern = optional_str(arguments, "pattern").unwrap_or("");
    let refresh = optional_bool(arguments, "refresh");
    let listed = web_table_list(session, pattern, refresh, false).map_err(ToolError::new)?;
    serde_json::to_value(listed).map_err(|err| ToolError::new(err.to_string()))
}

fn describe_tables(session: &mut Session, arguments: &Value) -> Result<Value, ToolError> {
    let tables = table_names(arguments)?;
    if tables.is_empty() {
        return Err(ToolError::new("name at least one table"));
    }
    let refresh = optional_bool(arguments, "refresh");
    let mut described = Vec::new();
    for name in tables {
        described.push(describe_one(session, &name, refresh));
    }
    Ok(json!({ "tables": described }))
}

fn describe_one(session: &mut Session, name: &str, refresh: bool) -> Value {
    match session.engine.describe(name, refresh) {
        Ok(Some(meta)) => {
            let from_cache = session.engine.last_read_from_cache();
            let indexes = meta
                .local_indexes
                .values()
                .map(|index| {
                    json!({
                        "name": index.name,
                        "kind": "local",
                        "hashKey": index.hash_key,
                        "rangeKey": index.range_key,
                    })
                })
                .chain(meta.global_indexes.values().map(|index| {
                    json!({
                        "name": index.name,
                        "kind": "global",
                        "hashKey": index.hash_key.name,
                        "rangeKey": index.range_key.as_ref().map(|field| field.name.clone()),
                    })
                }))
                .collect::<Vec<_>>();
            json!({
                "name": meta.name,
                "schema": meta.schema_dql(),
                "hashKey": meta.hash_key,
                "rangeKey": meta.range_key,
                "attributes": meta.attrs.keys().cloned().collect::<Vec<_>>(),
                "indexes": indexes,
                "status": status_name(meta.status),
                "fromCache": from_cache,
            })
        }
        Ok(None) => json!({ "name": name, "error": format!("table {name} not found") }),
        Err(err) => json!({ "name": name, "error": err.to_string() }),
    }
}

fn list_query_files(root: &Path) -> Result<Value, ToolError> {
    let files = files::list_dql_files(root).map_err(path_error)?;
    let files = files
        .into_iter()
        .map(|(path, name)| json!({"path": path, "name": name}))
        .collect::<Vec<_>>();
    Ok(json!({ "files": files }))
}

fn create_query_file(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let path = required_str(arguments, "path")?;
    let text = optional_str(arguments, "text").unwrap_or("");
    let created = files::create_file(root, path).map_err(path_error)?;
    if !text.is_empty() {
        files::write_text(root, &created, text).map_err(path_error)?;
    }
    Ok(json!({ "path": created, "text": text }))
}

fn read_query_file(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let path = required_str(arguments, "path")?;
    let path = files::normalized_dql(root, path).map_err(path_error)?;
    let text = files::read_text(root, &path).map_err(path_error)?;
    Ok(json!({ "path": path, "text": text }))
}

fn update_query_file(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let path = required_str(arguments, "path")?;
    let text = required_str(arguments, "text")?;
    let path = files::normalized_dql(root, path).map_err(path_error)?;
    if files::read_text(root, &path).is_err() {
        return Err(ToolError::new(format!("{path} does not exist")));
    }
    files::write_text(root, &path, text).map_err(path_error)?;
    Ok(json!({ "path": path, "text": text }))
}

/// One line for `dqlrs --web` when `/mcp` is not enabled.
pub fn web_mcp_hint(listen: &str) -> String {
    format!("MCP        off. Pass --mcp to serve http://{listen}/mcp")
}

/// Cursor and Claude snippets for the HTTP MCP endpoint.
pub fn http_client_config(url: &str) -> String {
    let cursor = pretty_servers(json!({ "url": url }));
    let claude = pretty_servers(json!({ "type": "http", "url": url }));
    format!("Cursor (.cursor/mcp.json):\n{cursor}\n\nClaude (.mcp.json):\n{claude}")
}

/// Cursor and Claude snippet for a stdio `dqlrs --mcp` process.
pub fn stdio_client_config(
    command: &str,
    dir: &Path,
    region: &str,
    endpoint: Option<(&str, u16)>,
    memory: bool,
) -> String {
    let mut args = vec![
        "--mcp".to_string(),
        "--dir".to_string(),
        absolute_dir(dir).display().to_string(),
        "-r".to_string(),
        region.to_string(),
    ];
    if let Some((host, port)) = endpoint {
        args.extend([
            "-H".to_string(),
            host.to_string(),
            "-p".to_string(),
            port.to_string(),
        ]);
    }
    let mut entry = json!({
        "command": command,
        "args": args,
    });
    if memory && endpoint.is_none() {
        entry["env"] = json!({ "DQL_BACKEND": "memory" });
    }
    format!(
        "Cursor (.cursor/mcp.json) and Claude (.mcp.json or claude_desktop_config.json):\n{}",
        pretty_servers(entry)
    )
}

fn pretty_servers(entry: Value) -> String {
    serde_json::to_string_pretty(&json!({ "mcpServers": { "dqlrs": entry } }))
        .unwrap_or_else(|_| "{}".to_string())
}

fn absolute_dir(dir: &Path) -> PathBuf {
    if dir.is_absolute() {
        return dir.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(dir))
        .unwrap_or_else(|_| dir.to_path_buf())
}

pub fn run(session: Session, root: PathBuf) -> std::io::Result<()> {
    let command = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "dqlrs".to_string());
    let endpoint = session.local_endpoint.clone();
    let config = stdio_client_config(
        &command,
        &root,
        &session.region,
        endpoint.as_ref().map(|(host, port)| (host.as_str(), *port)),
        session.engine.is_memory(),
    );
    eprintln!("{config}");
    let mut server = McpServer::new(session, root);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn tool_value(result: Result<Value, ToolError>) -> Result<Value, (i32, String)> {
    match result {
        Ok(value) => Ok(tool_content(value, false)),
        Err(err) => Ok(tool_content(json!({"error": err.message.clone()}), true)),
    }
}

fn tool_content(value: Value, is_error: bool) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value,
        "isError": is_error,
    })
}

fn rpc_error(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    match arguments.get(key) {
        Some(Value::String(text)) if !text.trim().is_empty() || key == "text" => Ok(text),
        Some(Value::String(_)) => Err(ToolError::new(format!("{key} is empty"))),
        Some(_) => Err(ToolError::new(format!("{key} must be a string"))),
        None => Err(ToolError::new(format!("missing {key}"))),
    }
}

fn optional_str<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str)
}

fn optional_bool(arguments: &Value, key: &str) -> bool {
    arguments.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn table_names(arguments: &Value) -> Result<Vec<String>, ToolError> {
    if let Some(name) = arguments.get("table").and_then(Value::as_str) {
        if !name.trim().is_empty() {
            return Ok(vec![name.to_string()]);
        }
    }
    match arguments.get("tables") {
        Some(Value::String(name)) if !name.trim().is_empty() => Ok(vec![name.clone()]),
        Some(Value::Array(items)) => {
            let mut names = Vec::new();
            for item in items {
                let Some(name) = item.as_str() else {
                    return Err(ToolError::new("tables must be strings"));
                };
                if !name.trim().is_empty() {
                    names.push(name.to_string());
                }
            }
            Ok(names)
        }
        Some(_) => Err(ToolError::new(
            "tables must be a string or a list of strings",
        )),
        None => Err(ToolError::new("missing tables")),
    }
}

fn path_error(err: PathError) -> ToolError {
    ToolError::new(err.0)
}

fn status_name(status: dql_models::TableStatus) -> &'static str {
    match status {
        dql_models::TableStatus::Active => "ACTIVE",
        dql_models::TableStatus::Creating => "CREATING",
        dql_models::TableStatus::Updating => "UPDATING",
        dql_models::TableStatus::Deleting => "DELETING",
    }
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "help",
            "Show DQL help. Omit topic for the command list. Pass a topic such as select, scan, insert, create, or opt for syntax and examples.",
            json!({
                "type": "object",
                "properties": {
                    "topic": { "type": "string", "description": "Command name, such as select. Omit for the full list." }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "global_search",
            "Search table names, cached columns and indexes, and queries in .dql files.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Text to match" }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        tool(
            "lint_query",
            "Check a DQL script for parse errors and, when a table definition is known, for queries the engine would reject. Does not run the query or fetch missing definitions. ok is false when there is an error. Warnings mean a table has no cached definition.",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "DQL script to check" },
                    "path": { "type": "string", "description": "Workspace .dql file to check when text is omitted" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "lint_query_file",
            "Check a workspace .dql file for parse errors and, when a table definition is known, for queries the engine would reject. Does not run the file or fetch missing definitions. ok is false when there is an error. Warnings mean a table has no cached definition.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace .dql file to check" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        tool(
            "list_tables",
            "List table names. Uses the saved list, and fetches it when none is saved. Does not describe every table.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Optional glob or ls-style match" },
                    "refresh": { "type": "boolean", "description": "Reload the saved list from the database" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "describe_tables",
            "Describe one or more tables. Uses a cached definition when one is saved. refresh=true asks the database again.",
            json!({
                "type": "object",
                "properties": {
                    "tables": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Table names"
                    },
                    "table": { "type": "string", "description": "One table name" },
                    "refresh": { "type": "boolean" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "list_query_files",
            "List .dql files in the workspace directory.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "create_query_file",
            "Create a new .dql file. Fails if the path already exists.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative path, such as queries/orders.dql" },
                    "text": { "type": "string", "description": "File contents" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        tool(
            "read_query_file",
            "Read a .dql file from the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        tool(
            "update_query_file",
            "Replace the contents of an existing .dql file.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["path", "text"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CliConfig;
    use crate::history::HistoryManager;
    use crate::session::RuntimeEngine;
    use crate::table_cache::{CacheEndpoint, DiskTableCache};
    use crate::throttle::TableLimits;
    use dql_engine::SharedTableCache;
    use serde_json::json;
    use std::path::Path;

    fn server(root: &Path) -> McpServer {
        let _ = std::fs::create_dir_all(root);
        McpServer::new(
            Session {
                config: CliConfig::load(),
                engine: RuntimeEngine::in_memory(true),
                local_endpoint: None,
                region: "us-west-1".to_string(),
                history: HistoryManager::new(),
                throttle: TableLimits::default(),
                table_disk: None,
            },
            root.to_path_buf(),
        )
    }

    fn attach_cache(server: &mut McpServer, dir: &Path) {
        let cache = DiskTableCache::open_at(
            dir.join("cache"),
            CacheEndpoint {
                region: "us-west-1".to_string(),
                host: Some("localhost".to_string()),
                port: 8000,
                allow_select_scan: true,
            },
        );
        server.session.table_disk = Some(cache.clone());
        server
            .session
            .engine
            .set_table_cache(Some(cache as SharedTableCache));
    }

    fn rpc(server: &mut McpServer, id: i64, method: &str, params: Value) -> Value {
        let line = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })
        .to_string();
        let response = server.handle_line(&line).expect("response");
        serde_json::from_str(&response).expect("json")
    }

    fn init(server: &mut McpServer) {
        let response = rpc(
            server,
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" }
            }),
        );
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert!(server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }

    fn tool_body(response: &Value) -> Value {
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], false, "{response}");
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text");
        serde_json::from_str(text).expect("tool json")
    }

    #[test]
    fn initialize_lists_the_web_tools() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        let early = rpc(&mut server, 1, "tools/list", json!({}));
        assert_eq!(early["error"]["code"], -32002);

        init(&mut server);
        let listed = rpc(&mut server, 2, "tools/list", json!({}));
        let names = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "help",
                "global_search",
                "lint_query",
                "lint_query_file",
                "list_tables",
                "describe_tables",
                "list_query_files",
                "create_query_file",
                "read_query_file",
                "update_query_file",
            ]
        );
    }

    #[test]
    fn help_lists_commands_and_one_topic() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        init(&mut server);
        let index = tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({"name": "help", "arguments": {}}),
        ));
        assert_eq!(index["title"], "Available commands");
        assert!(index["text"].as_str().unwrap().contains("select"));
        let select = tool_body(&rpc(
            &mut server,
            3,
            "tools/call",
            json!({"name": "help", "arguments": {"topic": "SELECT;"}}),
        ));
        assert_eq!(select["topic"], "select");
        assert_eq!(select["title"], "HELP select");
        assert!(select["text"].as_str().unwrap().contains("SELECT"));
        let blocks = select["blocks"].as_array().unwrap();
        assert!(blocks.iter().any(|block| block["kind"] == "code"));
        let alias = tool_body(&rpc(
            &mut server,
            4,
            "tools/call",
            json!({"name": "help", "arguments": {"topic": "options"}}),
        ));
        assert_eq!(alias["topic"], "opt");
        let missing = rpc(
            &mut server,
            5,
            "tools/call",
            json!({"name": "help", "arguments": {"topic": "nope"}}),
        );
        assert_eq!(missing["result"]["isError"], true);
        let text = missing["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("No help available for nope"), "{text}");
        let bad = rpc(
            &mut server,
            6,
            "tools/call",
            json!({"name": "help", "arguments": {"topic": 1}}),
        );
        assert_eq!(bad["result"]["isError"], true);
    }

    #[test]
    fn lint_query_flags_a_select_the_engine_would_reject() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        server.session.config.allow_select_scan = false;
        init(&mut server);
        let report = tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({
                "name": "lint_query",
                "arguments": {
                    "text": "CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf;\n"
                }
            }),
        ));
        assert_eq!(report["ok"], false);
        let message = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|issue| issue["message"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(message.contains("indexed WHERE"), "{message}");
    }

    #[test]
    fn lint_query_file_checks_a_workspace_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        server.session.config.allow_select_scan = false;
        init(&mut server);
        tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({
                "name": "create_query_file",
                "arguments": {
                    "path": "queries/shelf.dql",
                    "text": "CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf;\n"
                }
            }),
        ));
        let report = tool_body(&rpc(
            &mut server,
            3,
            "tools/call",
            json!({
                "name": "lint_query_file",
                "arguments": { "path": "queries/shelf.dql" }
            }),
        ));
        assert_eq!(report["ok"], false);
        let message = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|issue| issue["message"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(message.contains("indexed WHERE"), "{message}");

        let missing = rpc(
            &mut server,
            4,
            "tools/call",
            json!({
                "name": "lint_query_file",
                "arguments": {}
            }),
        );
        assert_eq!(missing["result"]["isError"], true);
    }

    #[test]
    fn lint_tools_do_not_run_queries() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        server.session.config.allow_select_scan = false;
        init(&mut server);
        server
            .session
            .engine
            .execute_script(
                "CREATE TABLE shelf (id STRING HASH KEY);\nINSERT INTO shelf (id, title) VALUES ('a', 'kept');\n",
            )
            .unwrap();
        let before = server
            .session
            .engine
            .execute_script("SELECT * FROM shelf WHERE id = 'a';")
            .unwrap();
        let tables = server.session.engine.list_tables().unwrap();
        let count = server.session.engine.table_item_count("shelf");
        let capacity = server.session.engine.statement_capacity();

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
        let inline = tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({
                "name": "lint_query",
                "arguments": { "text": script }
            }),
        ));
        assert_eq!(inline["ok"], false);
        assert_eq!(server.session.engine.list_tables().unwrap(), tables);
        assert_eq!(server.session.engine.table_item_count("shelf"), count);
        assert_eq!(server.session.engine.table_item_count("other"), 0);
        assert_eq!(server.session.engine.statement_capacity(), capacity);

        tool_body(&rpc(
            &mut server,
            3,
            "tools/call",
            json!({
                "name": "create_query_file",
                "arguments": { "path": "queries/live.dql", "text": script }
            }),
        ));
        let from_file = tool_body(&rpc(
            &mut server,
            4,
            "tools/call",
            json!({
                "name": "lint_query_file",
                "arguments": { "path": "queries/live.dql" }
            }),
        ));
        assert_eq!(from_file["ok"], false);
        assert_eq!(server.session.engine.list_tables().unwrap(), tables);
        assert_eq!(server.session.engine.table_item_count("shelf"), count);
        assert_eq!(server.session.engine.table_item_count("other"), 0);
        assert_eq!(server.session.engine.statement_capacity(), capacity);
        let after = server
            .session
            .engine
            .execute_script("SELECT * FROM shelf WHERE id = 'a';")
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn query_files_round_trip_and_show_up_in_search() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        init(&mut server);

        let created = tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({
                "name": "create_query_file",
                "arguments": {
                    "path": "queries/orders.dql",
                    "text": "SELECT * FROM shelf_orders WHERE id = 'a';\n"
                }
            }),
        ));
        assert_eq!(created["path"], "queries/orders.dql");

        let read = tool_body(&rpc(
            &mut server,
            3,
            "tools/call",
            json!({"name": "read_query_file", "arguments": {"path": "queries/orders.dql"}}),
        ));
        assert!(read["text"].as_str().unwrap().contains("shelf_orders"));

        let updated = tool_body(&rpc(
            &mut server,
            4,
            "tools/call",
            json!({
                "name": "update_query_file",
                "arguments": {
                    "path": "queries/orders",
                    "text": "SELECT status FROM shelf_orders WHERE id = 'a';\n"
                }
            }),
        ));
        assert_eq!(updated["path"], "queries/orders.dql");

        let duplicate = rpc(
            &mut server,
            5,
            "tools/call",
            json!({"name": "create_query_file", "arguments": {"path": "queries/orders.dql"}}),
        );
        assert_eq!(duplicate["result"]["isError"], true);

        let files = tool_body(&rpc(
            &mut server,
            6,
            "tools/call",
            json!({"name": "list_query_files", "arguments": {}}),
        ));
        assert_eq!(files["files"][0]["path"], "queries/orders.dql");

        let found = tool_body(&rpc(
            &mut server,
            7,
            "tools/call",
            json!({"name": "global_search", "arguments": {"query": "shelf_orders"}}),
        ));
        let queries = found["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["kind"] == "query")
            .collect::<Vec<_>>();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0]["path"], "queries/orders.dql");
    }

    #[test]
    fn list_and_describe_use_the_saved_definition() {
        let dir = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let mut server = server(dir.path());
        attach_cache(&mut server, cache.path());
        server
            .session
            .engine
            .execute_script(
                "CREATE TABLE shelf_orders (id STRING HASH KEY, sk STRING RANGE KEY) GLOBAL INDEX ('by_shelf', shelf STRING, id STRING);",
            )
            .expect("create");
        init(&mut server);

        let listed = tool_body(&rpc(
            &mut server,
            2,
            "tools/call",
            json!({"name": "list_tables", "arguments": {"pattern": "shelf*"}}),
        ));
        assert_eq!(listed["tables"][0]["name"], "shelf_orders");

        let described = tool_body(&rpc(
            &mut server,
            3,
            "tools/call",
            json!({"name": "describe_tables", "arguments": {"table": "shelf_orders"}}),
        ));
        let table = &described["tables"][0];
        assert_eq!(table["hashKey"], "id");
        assert_eq!(table["fromCache"], true);
        assert!(table["schema"].as_str().unwrap().contains("by_shelf"));
        let indexes = table["indexes"].as_array().unwrap();
        assert_eq!(indexes[0]["name"], "by_shelf");
        assert_eq!(indexes[0]["hashKey"], "shelf");

        let refreshed = tool_body(&rpc(
            &mut server,
            4,
            "tools/call",
            json!({
                "name": "describe_tables",
                "arguments": {"tables": ["shelf_orders"], "refresh": true}
            }),
        ));
        assert_eq!(refreshed["tables"][0]["fromCache"], false);

        let found = tool_body(&rpc(
            &mut server,
            5,
            "tools/call",
            json!({"name": "global_search", "arguments": {"query": "shelf"}}),
        ));
        let titles = found["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["kind"].as_str().unwrap(),
                    item["title"].as_str().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert!(titles.contains(&("index", "by_shelf")));
        assert!(titles.contains(&("column", "shelf")));
    }

    #[test]
    fn web_hint_names_the_mcp_flag_and_url() {
        let hint = web_mcp_hint("127.0.0.1:8765");
        assert!(hint.contains("--mcp"));
        assert!(hint.contains("http://127.0.0.1:8765/mcp"));
    }

    #[test]
    fn http_config_is_ready_for_cursor_and_claude() {
        let text = http_client_config("http://127.0.0.1:9000/mcp");
        assert!(text.contains("Cursor (.cursor/mcp.json)"));
        assert!(text.contains("Claude (.mcp.json)"));
        assert!(text.contains("\"url\": \"http://127.0.0.1:9000/mcp\""));
        assert!(text.contains("\"type\": \"http\""));
        let cursor = text.split("Claude").next().unwrap();
        assert!(!cursor.contains("\"type\""));
    }

    #[test]
    fn stdio_config_records_the_workspace_and_local_endpoint() {
        let text = stdio_client_config(
            "/usr/local/bin/dqlrs",
            Path::new("/work/queries"),
            "us-west-1",
            Some(("localhost", 8000)),
            false,
        );
        assert!(text.contains("Cursor (.cursor/mcp.json)"));
        assert!(text.contains("Claude"));
        assert!(text.contains("\"command\": \"/usr/local/bin/dqlrs\""));
        assert!(text.contains("\"--dir\""));
        assert!(text.contains("\"/work/queries\""));
        assert!(text.contains("\"localhost\""));
        assert!(text.contains("\"8000\""));
        assert!(text.contains("\"us-west-1\""));
        assert!(!text.contains("DQL_BACKEND"));
    }

    #[test]
    fn stdio_config_keeps_a_memory_backend_in_env() {
        let text = stdio_client_config("dqlrs", Path::new("queries"), "us-west-1", None, true);
        assert!(text.contains("\"DQL_BACKEND\": \"memory\""));
        assert!(text.contains("queries"));
        assert!(!text.contains("\"-H\""));
    }
}
