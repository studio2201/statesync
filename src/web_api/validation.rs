//! Request path / name validation for HTTP API handlers.
//! Upstream URL SSRF checks live in `config::url_safety`.

pub use crate::config::url_safety::{valid_server_url, validate_upstream_url};

const ITEM_ID_RE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
const MAX_ITEM_ID_LEN: usize = 64;
const MAX_SERVER_NAME_LEN: usize = 64;

pub fn valid_item_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ITEM_ID_LEN && id.bytes().all(|b| ITEM_ID_RE.contains(&b))
}

/// Display name used as a lookup key (poster proxy, etc.).
/// Allows spaces and common punctuation so auto-named servers work.
pub fn valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SERVER_NAME_LEN
        && !name.contains("..")
        && !name.contains('/')
        && !name.contains('\\')
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ' | ':' | '[' | ']' | '@')
        })
}
