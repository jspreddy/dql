//! Per-process token for the loopback web API and TCP `--serve`.
//!
//! The token is printed once on stderr and held in memory. It is not written
//! into the workspace or the cache.

use std::io;

/// 256 bits, hex-encoded. Safe to drop into an HTML attribute and a header.
pub fn mint() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(io::Error::other)?;
    Ok(hex_encode(&bytes))
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
    fn minted_token_is_64_hex_chars() {
        let token = mint().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(mint().unwrap(), token);
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
