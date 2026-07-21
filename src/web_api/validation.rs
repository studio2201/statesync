const ITEM_ID_RE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
const MAX_ITEM_ID_LEN: usize = 64;
const MAX_SERVER_NAME_LEN: usize = 64;

/// Missing documentation.
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

pub fn valid_server_url(u: &str) -> bool {
    let normalized = crate::config::normalize_server_url(u);
    let lower = normalized.to_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && normalized.len() <= 512
        && !normalized.contains("..")
}

/// Block cloud metadata / link-local SSRF targets for proxy endpoints.
pub fn validate_upstream_url(url: &str) -> Result<(), &'static str> {
    if !valid_server_url(url) {
        return Err("Invalid URL");
    }
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host_port = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("");
    let host = if host_port.starts_with('[') {
        host_port
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let host_l = host.to_lowercase();
    if host_l.is_empty() {
        return Err("URL missing host");
    }
    if host_l == "metadata.google.internal"
        || host_l == "metadata"
        || host_l == "169.254.169.254"
        || host_l.starts_with("169.254.")
        || host_l == "metadata.aws.internal"
        || host_l.ends_with(".metadata.google.internal")
        || host_l.starts_with("fe80:")
        || host_l == "::ffff:169.254.169.254"
    {
        return Err("Blocked host (cloud metadata / link-local)");
    }
    Ok(())
}
