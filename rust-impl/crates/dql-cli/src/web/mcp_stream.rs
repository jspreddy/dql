//! Standing `/mcp` event stream for grant, revoke, and deny.
//!
//! One subscriber per `GET`. A failed write drops that subscriber. Reconnect
//! replays the grants that are live now; denials are not replayed.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub fn subscribe(&self, current: &[Value]) -> Receiver<String> {
        let (tx, rx) = mpsc::sync_channel(64);
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

pub fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as i32;
    (year, month, day)
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}
