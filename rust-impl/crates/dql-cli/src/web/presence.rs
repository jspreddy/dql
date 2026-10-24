//! Who is using this web server right now.
//!
//! Web tabs heartbeat. MCP sessions are touched on each `/mcp` request.
//! A tab that stops heartbeating drops off quickly. An MCP client stays
//! through the quiet gaps between tool calls, then drops off too.
//!
//! The first time a client is seen it receives a stable identity, `W1`
//! or `M1`. That number is remembered for the life of the process, so a
//! missed heartbeat does not rename the tab.

use std::collections::HashMap;
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

    fn letter(self) -> char {
        match self {
            Kind::Web => 'W',
            Kind::Mcp => 'M',
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
    number: u32,
    color: &'static str,
}

struct State {
    live: Vec<Client>,
    next_web: u32,
    next_mcp: u32,
    assigned: HashMap<String, (u32, &'static str)>,
}

pub(crate) struct Book {
    state: Mutex<State>,
}

impl Book {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State {
                live: Vec::new(),
                next_web: 1,
                next_mcp: 1,
                assigned: HashMap::new(),
            }),
        }
    }

    pub(crate) fn touch(&self, id: &str, kind: Kind) {
        let mut state = self.lock();
        let now = Instant::now();
        expire(&mut state.live, now);
        if let Some(client) = state
            .live
            .iter_mut()
            .find(|client| client.kind == kind && client.id == id)
        {
            client.seen = now;
            return;
        }
        let (number, color) = assign(&mut state, id, kind);
        state.live.push(Client {
            id: id.to_string(),
            kind,
            seen: now,
            number,
            color,
        });
    }

    pub(crate) fn snapshot(&self) -> Value {
        let mut state = self.lock();
        expire(&mut state.live, Instant::now());
        let listed: Vec<Value> = state
            .live
            .iter()
            .map(|client| {
                json!({
                    "id": client.id,
                    "kind": client.kind.as_str(),
                    "label": format!("{}{}", client.kind.letter(), client.number),
                    "color": client.color,
                })
            })
            .collect();
        json!({ "clients": listed })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
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

fn assign(state: &mut State, id: &str, kind: Kind) -> (u32, &'static str) {
    let key = format!("{}:{id}", kind.as_str());
    if let Some(kept) = state.assigned.get(&key).copied() {
        return kept;
    }
    let number = match kind {
        Kind::Web => {
            let number = state.next_web;
            state.next_web += 1;
            number
        }
        Kind::Mcp => {
            let number = state.next_mcp;
            state.next_mcp += 1;
            number
        }
    };
    let color = color_for(number, kind);
    state.assigned.insert(key, (number, color));
    (number, color)
}

fn expire(clients: &mut Vec<Client>, now: Instant) {
    clients.retain(|client| now.saturating_duration_since(client.seen) <= client.kind.ttl());
}

fn color_for(number: u32, kind: Kind) -> &'static str {
    let offset = match kind {
        Kind::Web => 0,
        Kind::Mcp => 4,
    };
    let index = (number.saturating_sub(1) as usize + offset) % COLORS.len();
    COLORS[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(snapshot: &Value) -> Vec<&str> {
        snapshot["clients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|client| client["label"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn web_clients_expire_sooner_than_mcp() {
        let book = Book::new();
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-b", Kind::Web);
        book.touch("mcp", Kind::Mcp);
        {
            let mut state = book.lock();
            let stale = Instant::now() - Duration::from_secs(10);
            for client in &mut state.live {
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
        assert_eq!(labels(&snapshot), ["M1"]);
    }

    #[test]
    fn the_same_client_keeps_one_row() {
        let book = Book::new();
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-a", Kind::Web);
        let snapshot = book.snapshot();
        assert_eq!(snapshot["clients"].as_array().unwrap().len(), 1);
        assert_eq!(labels(&snapshot), ["W1"]);
        assert_eq!(snapshot["clients"][0]["color"], "#c2410c");
    }

    #[test]
    fn identities_stay_with_the_client() {
        let book = Book::new();
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-b", Kind::Web);
        book.touch("mcp", Kind::Mcp);
        let first = book.snapshot();
        assert_eq!(labels(&first), ["W1", "W2", "M1"]);
        assert_eq!(first["clients"][0]["color"], "#c2410c");
        assert_eq!(first["clients"][1]["color"], "#0067a5");
        assert_eq!(first["clients"][2]["color"], "#b45309");

        {
            let mut state = book.lock();
            let stale = Instant::now() - Duration::from_secs(10);
            for client in &mut state.live {
                if client.kind == Kind::Web {
                    client.seen = stale;
                }
            }
        }
        book.touch("web-window-a", Kind::Web);
        book.touch("web-window-c", Kind::Web);
        assert_eq!(labels(&book.snapshot()), ["M1", "W1", "W3"]);
    }
}
