//! Per-OS-user token for the loopback web API and TCP `--serve`.
//!
//! The token is loaded from the same key stores as the cache key, as its own
//! item. It is printed on stderr when a server starts and held in memory. It
//! is not written into the workspace or the cache, and it is not the cache key.

use std::io;
use std::sync::{Mutex, MutexGuard};

use zeroize::Zeroizing;

static TOKEN: Mutex<Option<String>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

/// The process token. The first call loads or creates it. Later calls reuse it.
pub fn load() -> io::Result<String> {
    let mut slot = lock(&TOKEN);
    if let Some(token) = slot.clone() {
        return Ok(token);
    }
    let bytes = Zeroizing::new(crate::cache_seal::load_api_token().map_err(io::Error::other)?);
    let token = hex_encode(&*bytes);
    *slot = Some(token.clone());
    Ok(token)
}

pub fn stderr_line(token: &str) -> String {
    format!("dqlrs token {token}")
}

/// Compare two tokens without stopping at the first difference.
pub fn matches(expected: &str, presented: &str) -> bool {
    let expected = expected.as_bytes();
    let presented = presented.as_bytes();
    let mut diff = expected.len() ^ presented.len();
    let longest = expected.len().max(presented.len());
    for index in 0..longest {
        let left = expected.get(index).copied().unwrap_or(0);
        let right = presented.get(index).copied().unwrap_or(0);
        diff |= usize::from(left ^ right);
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_token_is_64_hex_chars_and_stable() {
        let token = load().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(load().unwrap(), token);
        assert!(stderr_line(&token).starts_with("dqlrs token "));
    }

    #[test]
    fn match_rejects_a_different_length_and_a_near_miss() {
        assert!(matches("abcd", "abcd"));
        assert!(!matches("abcd", "abce"));
        assert!(!matches("abcd", "abc"));
        assert!(!matches("abcd", "abcde"));
        assert!(!matches("abcd", ""));
    }
}
