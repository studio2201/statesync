use super::{
    Config, MAX_GROUP_MEMBERS, MAX_KEY_LEN, MAX_MAPPING_GROUPS, MAX_MEMBER_LEN, MAX_NAME_LEN,
    MAX_URL_LEN, ServerConfig, name_from_url, normalize_server_url,
};
use anyhow::{Result, anyhow};

/// Fill empty names from URL host and normalize URLs in place.
pub fn normalize_config(cfg: &mut Config) {
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for s in &mut cfg.servers {
        s.url = normalize_server_url(&s.url);
        s.name = s.name.trim().to_string();
        let was_empty = s.name.is_empty();
        if was_empty {
            s.name = name_from_url(&s.url);
        }
        // Only auto-suffix when the name was derived (not user-provided duplicates —
        // those still fail validate_config so the user can fix them).
        if was_empty {
            let base = s.name.clone();
            let mut n = 2u32;
            let mut key = base.to_lowercase();
            while used.contains(&key) {
                s.name = format!("{}-{}", base, n);
                key = s.name.to_lowercase();
                n += 1;
            }
        }
        used.insert(s.name.to_lowercase());
        if s.sync_direction.is_empty() {
            s.sync_direction = "both".to_string();
        }
    }
}

pub(super) fn validate_server(s: &ServerConfig) -> Result<()> {
    if s.name.len() > MAX_NAME_LEN {
        return Err(anyhow!(
            "server name must be <= {} chars (got {})",
            MAX_NAME_LEN,
            s.name.len()
        ));
    }
    // Empty name is OK — normalize_config fills from URL before validate.
    if s.name.is_empty() {
        return Err(anyhow!(
            "server name is empty and could not be derived from url '{}'",
            s.url
        ));
    }
    if s.url.len() > MAX_URL_LEN || !(s.url.starts_with("http://") || s.url.starts_with("https://"))
    {
        return Err(anyhow!(
            "server '{}': url must start with http:// or https:// (or be host:port) and be <={} chars",
            s.name,
            MAX_URL_LEN
        ));
    }
    if s.url.starts_with("http://") && !s.allow_insecure_http {
        return Err(anyhow!(
            "server '{}': http:// url rejected (set allow_insecure_http: true to override)",
            s.name
        ));
    }
    if let Err(msg) = super::url_safety::validate_upstream_url(&s.url) {
        return Err(anyhow!("server '{}': {}", s.name, msg));
    }
    if s.api_key.trim().is_empty() {
        return Err(anyhow!("server '{}': api_key is required", s.name));
    }
    if s.api_key.len() > MAX_KEY_LEN {
        return Err(anyhow!(
            "server '{}': api_key too long ({} > {})",
            s.name,
            s.api_key.len(),
            MAX_KEY_LEN
        ));
    }
    match s.sync_direction.as_str() {
        "both" | "send" | "receive" => {}
        _ => {
            return Err(anyhow!(
                "server '{}': sync_direction must be one of both|send|receive",
                s.name
            ));
        }
    }
    Ok(())
}

pub fn validate_config(cfg: &Config) -> Result<()> {
    let mut cfg = cfg.clone();
    normalize_config(&mut cfg);
    if cfg.servers.len() > 20 {
        return Err(anyhow!(
            "too many servers configured ({} > 20)",
            cfg.servers.len()
        ));
    }
    let mut names = std::collections::HashSet::new();
    for s in &cfg.servers {
        validate_server(s)?;
        if !names.insert(s.name.to_lowercase()) {
            return Err(anyhow!("duplicate server name '{}' in config", s.name));
        }
    }
    if cfg.user_mappings.len() > MAX_MAPPING_GROUPS {
        return Err(anyhow!(
            "too many user_mapping groups ({} > {})",
            cfg.user_mappings.len(),
            MAX_MAPPING_GROUPS
        ));
    }
    for group in &cfg.user_mappings {
        if group.len() > MAX_GROUP_MEMBERS {
            return Err(anyhow!(
                "user_mapping group has too many members ({} > {})",
                group.len(),
                MAX_GROUP_MEMBERS
            ));
        }
        for name in group {
            if name.is_empty() || name.len() > MAX_MEMBER_LEN {
                return Err(anyhow!(
                    "user_mapping member name must be 1..={} chars",
                    MAX_MEMBER_LEN
                ));
            }
        }
    }
    Ok(())
}

pub fn is_loopback_bind(addr: &str) -> bool {
    let host = if let Some(rest) = addr.strip_prefix('[') {
        match rest.find(']') {
            Some(end) => &rest[..end],
            None => return false,
        }
    } else if addr.matches(':').count() > 1 {
        return addr == "::1";
    } else {
        match addr.rsplit_once(':') {
            Some((h, _)) => h,
            None => addr,
        }
    };
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}
