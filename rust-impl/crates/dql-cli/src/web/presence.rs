//! Who is using this web server right now.
//!
//! Web tabs heartbeat. MCP sessions are touched on each `/mcp` request.
//! A tab that stops heartbeating drops off quickly. An MCP client stays
//! through the quiet gaps between tool calls, then drops off too.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const WEB_TTL: Duration = Duration::from_secs(4);
const MCP_TTL: Duration = Duration::from_secs(60);
const COLORS: [&str; 8] = [
    "#c2410c", "#0067a5", "#6a3d9a", "#2e7d32", "#b45309", "#be185d", "#0f766e", "#7c3aed",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Web,
    Mcp,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Web => "web",
            Kind::Mcp => "mcp",
        }
    }

    fn ttl(self) -> Duration {
        match self {
            Kind::Web => WEB_TTL,
            Kind::Mcp => MCP_TTL,
        }
    }
}

struct Client {
    id: String,
    kind: Kind,
    seen: Instant,
}

pub(crate) struct Book {
    clients: Mutex<Vec<Client>>,
}

impl Book {
    pub(crate) fn new() -> Self {
        Self {
            clients: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn touch(&self, id: &str, kind: Kind) {
        let mut clients = self.lock();
        let now = Instant::now();
        expire(&mut clients, now);
        if let Some(client) = clients
            .iter_mut()
            .find(|client| client.kind == kind && client.id == id)
        {
            client.seen = now;
            return;
        }
        clients.push(Client {
            id: id.to_string(),
            kind,
            seen: now,
        });
    }

    pub(crate) fn snapshot(&self) -> Value {
        let mut clients = self.lock();
        expire(&mut clients, Instant::now());
        let listed: Vec<Value> = clients
            .iter()
            .map(|client| {
                json!({
                    "id": client.id,
                    "kind": client.kind.as_str(),
                    "color": color_for(&client.id, client.kind),
                })
            })
            .collect();
        json!({ "clients": listed })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Client>> {
        self.clients.lock().unwrap_or_else(|err| err.into_inner())
    }
}

/// Web heartbeats must name themselves. Short ids are reserved for the server.
pub(crate) fn valid_web_id(raw: &str) -> Option<&str> {
    let id = raw.trim();
    if (8..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        Some(id)
    } else {
        None
    }
}

pub(crate) fn valid_mcp_id(raw: &str) -> Option<&str> {
    valid_web_id(raw)
}

fn expire(clients: &mut Vec<Client>, now: Instant) {
    clients.retain(|client| now.saturating_duration_since(client.seen) <= client.kind.ttl());
}

fn color_for(id: &str, kind: Kind) -> &'static str {
    let mut hash: u32 = 2166136261;
    hash ^= match kind {
        Kind::Web => 1,
        Kind::Mcp => 2,
    };
    hash = hash.wrapping_mul(16777619);
    for byte in id.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16777619);
    }
    COLORS[(hash as usize) % COLORS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_clients_expire_sooner_than_mcp() {
        let book = Book::new();
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-b", Kind::Web);
        book.touch("mcp", Kind::Mcp);
        {
            let mut clients = book.lock();
            let stale = Instant::now() - Duration::from_secs(10);
            for client in clients.iter_mut() {
                if client.kind == Kind::Web {
                    client.seen = stale;
                }
            }
        }
        let snapshot = book.snapshot();
        let kinds: Vec<&str> = snapshot["clients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|client| client["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["mcp"]);
    }

    #[test]
    fn the_same_client_keeps_one_row() {
        let book = Book::new();
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-a", Kind::Web);
        let snapshot = book.snapshot();
        assert_eq!(snapshot["clients"].as_array().unwrap().len(), 1);
    }
}
