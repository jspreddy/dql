//! Standing `/mcp` event stream for grant, revoke, and deny.
//!
//! One subscriber per `GET`. A failed write drops that subscriber. Reconnect
//! replays the grants that are live now; denials are not replayed.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Mutex;

use serde_json::{json, Value};

struct Inner {
    subs: Vec<SyncSender<String>>,
}

pub struct Hub {
    inner: Mutex<Inner>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner { subs: Vec::new() }),
        }
    }

    /// Replay `granted` for each live table, then keep the subscriber.
    /// The channel fits the replay plus later live events, so a long grant
    /// list is not truncated on connect.
    pub fn subscribe(&self, current: &[Value]) -> Receiver<String> {
        let (tx, rx) = mpsc::sync_channel(current.len().saturating_add(64));
        for event in current {
            let _ = tx.try_send(frame(event));
        }
        self.lock().subs.push(tx);
        rx
    }

    pub fn publish(&self, event: &Value) {
        let frame = frame(event);
        self.lock()
            .subs
            .retain(|tx| match tx.try_send(frame.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            });
    }
}

pub fn granted(table: &str, request: Option<&str>, at: &str) -> Value {
    change("granted", Some(table), request, at)
}

pub fn revoked(table: &str, request: Option<&str>, at: &str) -> Value {
    change("revoked", Some(table), request, at)
}

pub fn denied(request: &str, at: &str) -> Value {
    change("denied", None, Some(request), at)
}

fn change(kind: &str, table: Option<&str>, request: Option<&str>, at: &str) -> Value {
    let mut params = json!({ "change": kind, "at": at });
    if let Some(table) = table {
        params["table"] = json!(table);
    }
    if let Some(request) = request {
        params["request"] = json!(request);
    }
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/dqlrs/grant_changed",
        "params": params,
    })
}

fn frame(event: &Value) -> String {
    format!("event: message\ndata: {event}\n\n")
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}
