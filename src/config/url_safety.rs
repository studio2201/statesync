//! Upstream URL safety (SSRF guards for user-supplied media-server URLs).

use super::helpers::normalize_server_url;

/// True when `url` is an acceptable Emby/Jellyfin origin (http/https, length-bounded).
pub fn valid_server_url(u: &str) -> bool {
    let normalized = normalize_server_url(u);
    let lower = normalized.to_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && normalized.len() <= 512
        && !normalized.contains("..")
}

/// Extract hostname from a normalized or raw URL (handles userinfo and IPv6).
pub fn extract_host(url: &str) -> Option<String> {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .trim();
    if authority.is_empty() {
        return None;
    }
    // user:pass@host → host (use last @ so passwords with @ are less wrong)
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        // hostname or IPv4 — strip :port
        host_port.split(':').next().unwrap_or("").to_string()
    };
    if host.is_empty() { None } else { Some(host) }
}

fn ipv4_octets(host: &str) -> Option<[u8; 4]> {
    // Dotted decimal
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 {
        let mut out = [0u8; 4];
        for (i, p) in parts.iter().enumerate() {
            let n: u32 = p.parse().ok()?;
            if n > 255 {
                return None;
            }
            out[i] = n as u8;
        }
        return Some(out);
    }
    // Single integer form (e.g. 2852039166 → 169.254.169.254)
    if host.chars().all(|c| c.is_ascii_digit()) {
        let n: u32 = host.parse().ok()?;
        return Some([
            ((n >> 24) & 0xff) as u8,
            ((n >> 16) & 0xff) as u8,
            ((n >> 8) & 0xff) as u8,
            (n & 0xff) as u8,
        ]);
    }
    // 0x… hex integer
    if let Some(hex) = host.strip_prefix("0x").or_else(|| host.strip_prefix("0X")) {
        if hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let n = u32::from_str_radix(hex, 16).ok()?;
            return Some([
                ((n >> 24) & 0xff) as u8,
                ((n >> 16) & 0xff) as u8,
                ((n >> 8) & 0xff) as u8,
                (n & 0xff) as u8,
            ]);
        }
    }
    None
}

fn is_cloud_metadata_host(host: &str) -> bool {
    let host_l = host.to_lowercase();
    if host_l == "metadata.google.internal"
        || host_l == "metadata"
        || host_l == "metadata.aws.internal"
        || host_l.ends_with(".metadata.google.internal")
        || host_l.starts_with("fe80:")
        || host_l == "::ffff:169.254.169.254"
        || host_l == "169.254.169.254"
        || host_l.starts_with("169.254.")
    {
        return true;
    }
    if let Some([a, b, _, _]) = ipv4_octets(&host_l) {
        // Link-local 169.254.0.0/16 (cloud metadata, APIPA)
        if a == 169 && b == 254 {
            return true;
        }
    }
    false
}

/// Block cloud metadata / link-local SSRF targets for proxy & probe endpoints.
pub fn validate_upstream_url(url: &str) -> Result<(), &'static str> {
    let normalized = normalize_server_url(url);
    if !valid_server_url(&normalized) {
        return Err("Invalid URL");
    }
    let host = extract_host(&normalized).ok_or("URL missing host")?;
    if is_cloud_metadata_host(&host) {
        return Err("Blocked host (cloud metadata / link-local)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_metadata_ip_and_aliases() {
        assert!(validate_upstream_url("http://169.254.169.254/").is_err());
        assert!(validate_upstream_url("http://metadata.google.internal/").is_err());
        assert!(validate_upstream_url("http://169.254.1.1:80").is_err());
        // Integer-encoded 169.254.169.254
        assert!(validate_upstream_url("http://2852039166/").is_err());
        assert!(validate_upstream_url("http://0xA9FEA9FE/").is_err());
    }

    #[test]
    fn blocks_userinfo_smuggling_to_metadata() {
        assert!(validate_upstream_url("http://evil@169.254.169.254/").is_err());
        assert!(validate_upstream_url("http://user:pass@169.254.169.254:80/").is_err());
    }

    #[test]
    fn allows_lan_media_servers() {
        assert!(validate_upstream_url("http://10.0.0.10:8096").is_ok());
        assert!(validate_upstream_url("http://192.168.1.50:8096").is_ok());
        assert!(validate_upstream_url("http://localhost:8096").is_ok());
        assert!(validate_upstream_url("https://emby.example.com").is_ok());
    }

    #[test]
    fn extract_host_handles_ipv6_and_userinfo() {
        assert_eq!(
            extract_host("http://[::1]:8096/path").as_deref(),
            Some("::1")
        );
        assert_eq!(
            extract_host("http://user@10.0.0.5:8096").as_deref(),
            Some("10.0.0.5")
        );
    }
}
