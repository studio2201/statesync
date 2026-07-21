use anyhow::{Result, anyhow};
use reqwest::Response;
use std::time::Duration;
use super::MediaClient;

pub fn retry_enabled() -> bool {
    std::env::var("STATESYNC_HTTP_RETRY")
        .map(|v| !v.eq_ignore_ascii_case("off"))
        .unwrap_or(true)
}

pub async fn send_with_retry(req: reqwest::RequestBuilder, label: &str) -> Result<Response> {
    let enabled = retry_enabled();
    let mut last_err: Option<anyhow::Error> = None;
    let mut backoff_ms = 500u64;
    let max_attempts = if enabled { 3 } else { 1 };
    for attempt in 0..max_attempts {
        let attempt_req = req
            .try_clone()
            .ok_or_else(|| anyhow!("request body not cloneable for retry"))?;
        match attempt_req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }
                if !enabled
                    || status == reqwest::StatusCode::UNAUTHORIZED
                    || status == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(anyhow!("{}: HTTP {}", label, status));
                }
                if status.is_server_error() {
                    last_err = Some(anyhow!("{}: HTTP {}", label, status));
                } else {
                    return Err(anyhow!("{}: HTTP {}", label, status));
                }
            }
            Err(e) => {
                last_err = Some(anyhow!("{}: {}", label, e));
            }
        }
        if attempt + 1 < max_attempts {
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(8000);
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("{} failed", label)))
}

impl MediaClient {
    /// Builds a full URL path for an API endpoint on the target media server.
    pub fn url_path(&self, path: &str) -> String {
        let clean_url = self.url.trim_end_matches('/');
        let clean_path = if path.starts_with('/') { &path[1..] } else { path };
        format!("{}/{}", clean_url, clean_path)
    }

    /// Adds authentication headers expected by Emby and Jellyfin servers and reverse proxies.
    pub fn add_auth_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        let auth_val = format!(
            r#"MediaBrowser Client="StateSync", Device="StateSync", DeviceId="statesync-sidecar", Version="{}", Token="{}"#,
            env!("CARGO_PKG_VERSION"),
            self.api_key
        );
        builder = builder
            .header("Authorization", &auth_val)
            .header("X-Emby-Token", &self.api_key)
            .header("X-MediaBrowser-Token", &self.api_key)
            .header("X-Emby-Authorization", &auth_val);
        builder
    }
}
