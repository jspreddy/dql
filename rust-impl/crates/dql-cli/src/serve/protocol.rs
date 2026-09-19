use dql_engine::{json_util::item_to_json, EngineError, StatementResult};
use serde::{Deserialize, Serialize};

/// Client → worker request. Unknown fields are ignored.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ClientRequest {
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub op: Option<String>,
    #[serde(default)]
    pub dql: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServeError {
    pub code: String,
    pub message: String,
}

/// One JSON envelope per request (serve-only; `-c --json` is unchanged).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServeEnvelope {
    pub id: Option<serde_json::Value>,
    pub ok: bool,
    pub kind: String,
    pub items: Option<Vec<serde_json::Value>>,
    pub affected: Option<u64>,
    pub message: Option<String>,
    pub partial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ServeError>,
}

impl ServeEnvelope {
    pub fn success(kind: &str) -> Self {
        Self {
            id: None,
            ok: true,
            kind: kind.to_string(),
            items: None,
            affected: None,
            message: None,
            partial: false,
            error: None,
        }
    }

    pub fn err(code: &str, message: impl Into<String>) -> Self {
        Self {
            id: None,
            ok: false,
            kind: "error".to_string(),
            items: None,
            affected: None,
            message: None,
            partial: false,
            error: Some(ServeError {
                code: code.to_string(),
                message: message.into(),
            }),
        }
    }

    pub fn with_id(mut self, id: Option<serde_json::Value>) -> Self {
        self.id = id;
        self
    }
}

pub fn parse_request_line(line: &str) -> Result<ClientRequest, String> {
    serde_json::from_str::<ClientRequest>(line).map_err(|err| format!("invalid JSON: {err}"))
}

pub fn envelope_from_result(result: StatementResult, partial: bool) -> ServeEnvelope {
    let mut envelope = match result {
        StatementResult::None => ServeEnvelope::success("none"),
        StatementResult::Status(message) => {
            let mut envelope = ServeEnvelope::success("status");
            envelope.message = Some(message);
            envelope
        }
        StatementResult::Affected(count) => {
            let mut envelope = ServeEnvelope::success("affected");
            envelope.affected = Some(count as u64);
            envelope
        }
        StatementResult::Items(items) => {
            let mut envelope = ServeEnvelope::success("items");
            envelope.items = Some(items.iter().map(item_to_value).collect());
            envelope
        }
        StatementResult::Schema(schema) => {
            let mut envelope = ServeEnvelope::success("schema");
            envelope.message = Some(schema);
            envelope
        }
    };
    envelope.partial = partial;
    envelope
}

pub fn envelope_from_error(err: EngineError) -> ServeEnvelope {
    match err {
        EngineError::Parse(parse_err) => ServeEnvelope::err("parse", parse_err.to_string()),
        EngineError::Runtime(message) => ServeEnvelope::err("runtime", message),
    }
}

fn item_to_value(item: &dql_engine::Item) -> serde_json::Value {
    let raw = item_to_json(item, 0);
    serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_engine::Item;
    use dql_parser::Value;

    #[test]
    fn parses_exec_ping_shutdown() {
        let exec: ClientRequest =
            serde_json::from_str(r#"{"id":"1","op":"exec","dql":"SCAN * FROM t;"}"#).unwrap();
        assert_eq!(exec.op.as_deref(), Some("exec"));
        assert_eq!(exec.dql.as_deref(), Some("SCAN * FROM t;"));
        assert_eq!(exec.id, Some(serde_json::json!("1")));

        let ping: ClientRequest = serde_json::from_str(r#"{"id":"2","op":"ping"}"#).unwrap();
        assert_eq!(ping.op.as_deref(), Some("ping"));
        assert!(ping.dql.is_none());

        let shutdown: ClientRequest = serde_json::from_str(r#"{"id":3,"op":"shutdown"}"#).unwrap();
        assert_eq!(shutdown.op.as_deref(), Some("shutdown"));
        assert_eq!(shutdown.id, Some(serde_json::json!(3)));
    }

    #[test]
    fn ignores_unknown_fields() {
        let request: ClientRequest =
            serde_json::from_str(r#"{"id":"1","op":"ping","extra":true}"#).unwrap();
        assert_eq!(request.op.as_deref(), Some("ping"));
    }

    #[test]
    fn invalid_json_is_protocol_error() {
        let err = parse_request_line("not-json\n").unwrap_err();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn maps_statement_results() {
        let none = envelope_from_result(StatementResult::None, false);
        assert!(none.ok);
        assert_eq!(none.kind, "none");

        let status = envelope_from_result(StatementResult::Status("created".into()), false);
        assert_eq!(status.kind, "status");
        assert_eq!(status.message.as_deref(), Some("created"));

        let affected = envelope_from_result(StatementResult::Affected(2), false);
        assert_eq!(affected.kind, "affected");
        assert_eq!(affected.affected, Some(2));

        let schema = envelope_from_result(StatementResult::Schema("CREATE TABLE t".into()), true);
        assert_eq!(schema.kind, "schema");
        assert!(schema.partial);

        let mut item = Item::new();
        item.insert("id".into(), Value::String("a".into()));
        let items = envelope_from_result(StatementResult::Items(vec![item]), false);
        assert_eq!(items.kind, "items");
        assert_eq!(items.items.as_ref().unwrap()[0]["id"], "a");
    }

    #[test]
    fn maps_engine_errors() {
        let runtime = envelope_from_error(EngineError::Runtime("boom".into()));
        assert_eq!(runtime.error.as_ref().unwrap().code, "runtime");
        assert_eq!(runtime.error.as_ref().unwrap().message, "boom");
    }

    #[test]
    fn success_json_includes_null_fields() {
        let json = serde_json::to_value(
            ServeEnvelope::success("none").with_id(Some(serde_json::json!("1"))),
        )
        .unwrap();
        assert_eq!(json["id"], "1");
        assert_eq!(json["ok"], true);
        assert_eq!(json["kind"], "none");
        assert!(json["items"].is_null());
        assert!(json["affected"].is_null());
        assert!(json["message"].is_null());
        assert_eq!(json["partial"], false);
        assert!(json.get("error").is_none());
    }
}
