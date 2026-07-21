use super::MediaClient;
use super::request::send_with_retry;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;

impl MediaClient {
    /// Missing documentation.
    pub async fn get_public_server_info(&self) -> Result<serde_json::Value> {
        let clean_url = self.url.trim_end_matches('/');
        let primary_url = format!("{}/System/Info/Public", clean_url);
        let emby_url = format!("{}/emby/System/Info/Public", clean_url);

        let resp = match self.client.get(&primary_url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => self
                .client
                .get(&emby_url)
                .send()
                .await
                .context("Failed to fetch /System/Info/Public")?,
        };

        if !resp.status().is_success() {
            return Err(anyhow!(
                "/System/Info/Public returned HTTP {}",
                resp.status()
            ));
        }

        let mut data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse /System/Info/Public response")?;

        let product_name = data
            .get("ProductName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let is_emby = product_name.contains("emby")
            || (data.get("LocalAddress").is_some() && !product_name.contains("jellyfin"));

        if let Some(obj) = data.as_object_mut() {
            obj.insert("is_emby".to_string(), serde_json::Value::Bool(is_emby));
        }

        Ok(data)
    }

    /// Fetch users from Emby/Jellyfin.
    ///
    /// Tries both `/Users` and `/emby/Users` (order depends on `is_emby`) so
    /// reverse-proxy and native installs both work.
    pub async fn get_users(&self) -> Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        let mut start_index: usize = 0;
        let page_size: usize = 500;
        // Prefer the prefix that matches server type, then fall back.
        let prefixes: &[&str] = if self.is_emby {
            &["/emby", ""]
        } else {
            &["", "/emby"]
        };
        // Remember which prefix worked so pagination stays consistent.
        let mut working_prefix: Option<&'static str> = None;

        loop {
            let page = start_index / page_size;
            let resp = self
                .get_users_page(start_index, page_size, prefixes, &mut working_prefix)
                .await
                .with_context(|| {
                    format!(
                        "Failed to get users list from {} (page {}). \
                         If StateSync runs in Docker bridge mode, use a LAN IP \
                         or resolvable hostname (not localhost). Also check API key and type.",
                        self.url, page
                    )
                })?;

            let data: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse users response")?;
            let arr = data
                .get("Items")
                .and_then(|v| v.as_array())
                .or_else(|| data.as_array())
                .cloned()
                .unwrap_or_default();
            let total = data
                .get("TotalRecordCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let page_count = arr.len();
            for u in arr {
                if let (Some(name), Some(id)) = (
                    u.get("Name").and_then(|n| n.as_str()),
                    u.get("Id").and_then(|id| id.as_str()),
                ) {
                    map.insert(name.to_lowercase(), id.to_string());
                }
            }
            if page_count < page_size || map.len() >= total {
                break;
            }
            start_index += page_size;
            if start_index > 50_000 {
                break;
            }
        }
        Ok(map)
    }

    async fn get_users_page(
        &self,
        start_index: usize,
        page_size: usize,
        prefixes: &[&'static str],
        working_prefix: &mut Option<&'static str>,
    ) -> Result<reqwest::Response> {
        let try_prefixes: Vec<&'static str> = if let Some(p) = *working_prefix {
            vec![p]
        } else {
            prefixes.to_vec()
        };
        let mut last_err: Option<anyhow::Error> = None;
        for prefix in try_prefixes {
            let path = if prefix.is_empty() {
                format!("/Users?StartIndex={}&Limit={}", start_index, page_size)
            } else {
                format!(
                    "{}/Users?StartIndex={}&Limit={}",
                    prefix, start_index, page_size
                )
            };
            let url = self.url_path(&path);
            match send_with_retry(self.add_auth_headers(self.client.get(&url)), "get_users").await {
                Ok(resp) => {
                    *working_prefix = Some(prefix);
                    return Ok(resp);
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("get_users: no path succeeded")))
    }
}
