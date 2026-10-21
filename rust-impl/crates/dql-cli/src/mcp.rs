//! MCP server on stdio for Cursor and Claude.
//!
//! The process speaks newline-delimited JSON-RPC. Logs stay off stdout.
//! Tools use the same workspace directory as `dqlrs --web --dir` and the same
//! table cache for that connection.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

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
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                return Some(rpc_error(Value::Null, -32700, "parse error"));
            }
        };
        let id = message.get("id").cloned()?;
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = self.dispatch(method, &params);
        Some(match result {
            Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}).to_string(),
            Err((code, message)) => rpc_error(id, code, &message),
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, (i32, String)> {
        if method != "initialize" && method != "ping" && !self.initialized {
            return Err((-32002, "server not initialized".to_string()));
        }
        match method {
            "initialize" => Ok(self.initialize(params)),
            "ping" => Ok(json!({})),
            "logging/setLevel" => Ok(json!({})),
            "resources/list" => Ok(json!({"resources": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => self.call_tool(params),
            _ => Err((-32601, format!("unknown method {method}"))),
        }
    }

    fn initialize(&mut self, params: &Value) -> Value {
        self.initialized = true;
        let version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .filter(|version| !version.is_empty())
            .unwrap_or(PROTOCOL_VERSION);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Tools read and write the .dql workspace passed to dqlrs --mcp --dir, the same folder dqlrs --web uses. global_search finds table names, cached columns and indexes, and queries in files. list_tables and describe_tables use the shared table cache for this connection. create_query_file, read_query_file, and update_query_file change query files on disk."
        })
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i32, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "tools/call requires a name".to_string()))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        match name {
            "global_search" => tool_value(self.global_search(&arguments)),
            "list_tables" => tool_value(self.list_tables(&arguments)),
            "describe_tables" => tool_value(self.describe_tables(&arguments)),
            "list_query_files" => tool_value(self.list_query_files()),
            "create_query_file" => tool_value(self.create_query_file(&arguments)),
            "read_query_file" => tool_value(self.read_query_file(&arguments)),
            "update_query_file" => tool_value(self.update_query_file(&arguments)),
            _ => Err((-32602, format!("unknown tool {name}"))),
        }
    }

    fn global_search(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let query = required_str(arguments, "query")?;
        let results = search(&mut self.session, &self.root, query);
        serde_json::to_value(results).map_err(|err| ToolError::new(err.to_string()))
    }

    fn list_tables(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let pattern = optional_str(arguments, "pattern").unwrap_or("");
        let refresh = optional_bool(arguments, "refresh");
        let listed =
            web_table_list(&mut self.session, pattern, refresh, false).map_err(ToolError::new)?;
        serde_json::to_value(listed).map_err(|err| ToolError::new(err.to_string()))
    }

    fn describe_tables(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let tables = table_names(arguments)?;
        if tables.is_empty() {
            return Err(ToolError::new("name at least one table"));
        }
        let refresh = optional_bool(arguments, "refresh");
        let mut described = Vec::new();
        for name in tables {
            described.push(self.describe_one(&name, refresh));
        }
        Ok(json!({ "tables": described }))
    }

    fn describe_one(&mut self, name: &str, refresh: bool) -> Value {
        match self.session.engine.describe(name, refresh) {
            Ok(Some(meta)) => {
                let from_cache = self.session.engine.last_read_from_cache();
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

    fn list_query_files(&self) -> Result<Value, ToolError> {
        let files = files::list_dql_files(&self.root).map_err(path_error)?;
        let files = files
            .into_iter()
            .map(|(path, name)| json!({"path": path, "name": name}))
            .collect::<Vec<_>>();
        Ok(json!({ "files": files }))
    }

    fn create_query_file(&self, arguments: &Value) -> Result<Value, ToolError> {
        let path = required_str(arguments, "path")?;
        let text = optional_str(arguments, "text").unwrap_or("");
        let created = files::create_file(&self.root, path).map_err(path_error)?;
        if !text.is_empty() {
            files::write_text(&self.root, &created, text).map_err(path_error)?;
        }
        Ok(json!({ "path": created, "text": text }))
    }

    fn read_query_file(&self, arguments: &Value) -> Result<Value, ToolError> {
        let path = required_str(arguments, "path")?;
        let path = files::normalized_dql(&self.root, path).map_err(path_error)?;
        let text = files::read_text(&self.root, &path).map_err(path_error)?;
        Ok(json!({ "path": path, "text": text }))
    }

    fn update_query_file(&self, arguments: &Value) -> Result<Value, ToolError> {
        let path = required_str(arguments, "path")?;
        let text = required_str(arguments, "text")?;
        let path = files::normalized_dql(&self.root, path).map_err(path_error)?;
        if files::read_text(&self.root, &path).is_err() {
            return Err(ToolError::new(format!("{path} does not exist")));
        }
        files::write_text(&self.root, &path, text).map_err(path_error)?;
        Ok(json!({ "path": path, "text": text }))
    }
}

pub fn run(session: Session, root: PathBuf) -> std::io::Result<()> {
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
                "global_search",
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
}
