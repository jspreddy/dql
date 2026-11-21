//! Pending `request_access` calls for `dqlrs web --mcp`.
//!
//! The tool waits at most 30 seconds, then returns `pending`. The answer stays
//! until the process stops, so a later call with the same id gets it.

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::shape::split_statements;
use dql_parser::{parse_statement, Statement};
use serde_json::{json, Value};

pub const WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Open,
    Ran {
        table: String,
        classification: Value,
        rows: Value,
        total: usize,
    },
    Granted {
        table: String,
    },
    Denied,
}

struct Request {
    id: String,
    query: String,
    agent: String,
    phase: Phase,
    owns_table: bool,
}

struct Inner {
    next: u64,
    requests: BTreeMap<String, Request>,
}

pub struct Store {
    inner: Mutex<Inner>,
    changed: Condvar,
}

impl Store {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                next: 1,
                requests: BTreeMap::new(),
            }),
            changed: Condvar::new(),
        }
    }

    /// One `SELECT` or `SCAN`. Anything else is an error and is not stored.
    pub fn check_query(query: &str) -> Result<String, String> {
        let statements = split_statements(query);
        if statements.len() != 1 {
            return Err("request_access takes one SELECT or SCAN".to_string());
        }
        let text = statements[0].trim().trim_end_matches(';').trim();
        match parse_statement(text) {
            Ok(Statement::Select { .. } | Statement::Scan { .. }) => Ok(text.to_string()),
            Ok(_) => Err("request_access takes one SELECT or SCAN".to_string()),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn open(&self, query: &str, agent: &str) -> Result<String, String> {
        let query = Self::check_query(query)?;
        let mut inner = self.lock();
        let id = format!("r{}", inner.next);
        inner.next = inner.next.saturating_add(1);
        inner.requests.insert(
            id.clone(),
            Request {
                id: id.clone(),
                query,
                agent: agent.to_string(),
                phase: Phase::Open,
                owns_table: false,
            },
        );
        self.changed.notify_all();
        Ok(id)
    }

    /// Wait until the request is granted or denied, or until `timeout`.
    pub fn wait(&self, id: &str, timeout: Duration) -> Result<Value, String> {
        let inner = self.lock();
        if !inner.requests.contains_key(id) {
            return Err(format!("unknown request {id}"));
        }
        let (guard, _) = self
            .changed
            .wait_timeout_while(inner, timeout, |inner| {
                matches!(
                    inner.requests.get(id).map(|request| &request.phase),
                    Some(Phase::Open | Phase::Ran { .. })
                )
            })
            .unwrap_or_else(|err| err.into_inner());
        drop(guard);
        self.answer(id)
    }

    pub fn answer(&self, id: &str) -> Result<Value, String> {
        let inner = self.lock();
        let request = inner
            .requests
            .get(id)
            .ok_or_else(|| format!("unknown request {id}"))?;
        Ok(answer(request))
    }

    /// `Ok(true)` means Run should drop the temp table from the previous run first.
    pub fn can_run(&self, id: &str) -> Result<bool, String> {
        let inner = self.lock();
        let request = inner
            .requests
            .get(id)
            .ok_or_else(|| format!("unknown request {id}"))?;
        match request.phase {
            Phase::Open => Ok(false),
            Phase::Ran { .. } => Ok(true),
            Phase::Granted { .. } => Err(format!("{id} is already granted")),
            Phase::Denied => Err(format!("{id} is already denied")),
        }
    }

    pub fn query(&self, id: &str) -> Result<String, String> {
        let inner = self.lock();
        inner
            .requests
            .get(id)
            .map(|request| request.query.clone())
            .ok_or_else(|| format!("unknown request {id}"))
    }

    pub fn table_name(id: &str) -> String {
        format!("mcp_{id}")
    }

    pub fn ran(
        &self,
        id: &str,
        classification: Value,
        rows: Value,
        total: usize,
    ) -> Result<String, String> {
        let table = Self::table_name(id);
        let mut inner = self.lock();
        let request = inner
            .requests
            .get_mut(id)
            .ok_or_else(|| format!("unknown request {id}"))?;
        if matches!(request.phase, Phase::Granted { .. } | Phase::Denied) {
            return Err(format!("{id} is already decided"));
        }
        request.phase = Phase::Ran {
            table: table.clone(),
            classification,
            rows,
            total,
        };
        request.owns_table = true;
        self.changed.notify_all();
        Ok(table)
    }

    /// `Some(table)` when this request may be granted.
    pub fn grant(&self, id: &str) -> Result<String, String> {
        let inner = self.lock();
        let request = inner
            .requests
            .get(id)
            .ok_or_else(|| format!("unknown request {id}"))?;
        match &request.phase {
            Phase::Ran { table, .. } => Ok(table.clone()),
            Phase::Granted { .. } => Err(format!("{id} is already granted")),
            Phase::Denied => Err(format!("{id} is already denied")),
            Phase::Open => Err("run the query before granting".to_string()),
        }
    }

    /// Mark the request granted after `GRANT` succeeded.
    pub fn finish_grant(&self, id: &str) {
        let mut inner = self.lock();
        if let Some(request) = inner.requests.get_mut(id) {
            if let Phase::Ran { table, .. } = &request.phase {
                request.phase = Phase::Granted {
                    table: table.clone(),
                };
            }
        }
        self.changed.notify_all();
    }

    /// Drop the temp table this request created, when it has one.
    pub fn deny(&self, id: &str) -> Result<Option<String>, String> {
        let mut inner = self.lock();
        let request = inner
            .requests
            .get_mut(id)
            .ok_or_else(|| format!("unknown request {id}"))?;
        if matches!(request.phase, Phase::Granted { .. }) {
            return Err(format!("{id} is already granted"));
        }
        if matches!(request.phase, Phase::Denied) {
            return Err(format!("{id} is already denied"));
        }
        let table = request.owns_table.then(|| Self::table_name(id));
        request.phase = Phase::Denied;
        request.owns_table = false;
        self.changed.notify_all();
        Ok(table)
    }

    pub fn request_for_table(&self, table: &str) -> Option<String> {
        let inner = self.lock();
        inner.requests.iter().find_map(|(id, request)| {
            let owned = match &request.phase {
                Phase::Ran { table: name, .. } | Phase::Granted { table: name } => name == table,
                Phase::Open | Phase::Denied => false,
            };
            owned.then(|| id.clone())
        })
    }

    /// A `GRANT` typed in the editor completes the matching request.
    pub fn note_granted(&self, table: &str) {
        let mut inner = self.lock();
        let mut changed = false;
        for request in inner.requests.values_mut() {
            if let Phase::Ran { table: name, .. } = &request.phase {
                if name == table {
                    request.phase = Phase::Granted {
                        table: name.clone(),
                    };
                    changed = true;
                }
            }
        }
        if changed {
            self.changed.notify_all();
        }
    }

    /// The temp table is gone, so Grant stays off until Run again.
    pub fn note_missing(&self, exists: impl Fn(&str) -> bool) {
        let mut inner = self.lock();
        let mut changed = false;
        for request in inner.requests.values_mut() {
            if let Phase::Ran { table, .. } = &request.phase {
                if !exists(table) {
                    request.phase = Phase::Open;
                    request.owns_table = false;
                    changed = true;
                }
            }
        }
        if changed {
            self.changed.notify_all();
        }
    }

    pub fn list(&self, connected: impl Fn(&str) -> bool, label: impl Fn(&str) -> String) -> Value {
        let inner = self.lock();
        let requests: Vec<Value> = inner
            .requests
            .values()
            .filter(|request| !matches!(request.phase, Phase::Granted { .. } | Phase::Denied))
            .map(|request| request_json(request, connected(&request.agent), &label(&request.agent)))
            .collect();
        json!(requests)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

fn answer(request: &Request) -> Value {
    match &request.phase {
        Phase::Granted { table } => json!({
            "request": request.id,
            "status": "granted",
            "table": table,
        }),
        Phase::Denied => json!({
            "request": request.id,
            "status": "denied",
        }),
        Phase::Open | Phase::Ran { .. } => json!({
            "request": request.id,
            "status": "pending",
        }),
    }
}

fn request_json(request: &Request, connected: bool, label: &str) -> Value {
    let (table, classification, rows, total, status) = match &request.phase {
        Phase::Ran {
            table,
            classification,
            rows,
            total,
        } => (
            Some(table.clone()),
            Some(classification.clone()),
            Some(rows.clone()),
            Some(*total),
            "ran",
        ),
        Phase::Open => (None, None, None, None, "open"),
        Phase::Granted { .. } | Phase::Denied => (None, None, None, None, "done"),
    };
    json!({
        "id": request.id,
        "query": request.query,
        "agent": request.agent,
        "label": label,
        "connected": connected,
        "status": status,
        "table": table,
        "classification": classification,
        "rows": rows,
        "total": total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn only_one_read_is_accepted() {
        assert!(Store::check_query("SELECT * FROM orders").is_ok());
        assert!(Store::check_query("SCAN * FROM orders").is_ok());
        assert!(Store::check_query("INSERT INTO orders (id) VALUES ('a')").is_err());
        assert!(Store::check_query("SELECT * FROM a; SELECT * FROM b").is_err());
    }

    #[test]
    fn a_wait_returns_the_stored_answer() {
        let store = Store::new();
        let id = store.open("SELECT * FROM orders", "mcp-a").unwrap();
        let pending = store.wait(&id, Duration::from_millis(20)).unwrap();
        assert_eq!(pending["status"], "pending");
        let again = store.answer(&id).unwrap();
        assert_eq!(again["status"], "pending");

        let store = std::sync::Arc::new(store);
        let waiting = std::sync::Arc::clone(&store);
        let handle = thread::spawn(move || waiting.wait(&id, Duration::from_secs(2)).unwrap());
        thread::sleep(Duration::from_millis(30));
        let id = "r1";
        store
            .ran(
                id,
                serde_json::json!({"line": "clean"}),
                serde_json::json!([]),
                0,
            )
            .unwrap();
        store.grant(id).unwrap();
        store.finish_grant(id);
        let answer = handle.join().unwrap();
        assert_eq!(answer["status"], "granted");
        assert_eq!(answer["table"], "mcp_r1");
        assert_eq!(store.answer(id).unwrap()["status"], "granted");
    }

    #[test]
    fn deny_is_remembered() {
        let store = Store::new();
        let id = store.open("SCAN * FROM orders", "mcp-a").unwrap();
        assert_eq!(store.deny(&id).unwrap(), None);
        assert_eq!(store.answer(&id).unwrap()["status"], "denied");
    }
}
