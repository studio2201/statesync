use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use super::MediaClient;
use super::request::send_with_retry;

impl MediaClient {
    pub async fn get_public_server_info(&self) -> Result<serde_json::Value> {
        let path = "/System/Info/Public".to_string();
        let url = self.url_path(&path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch /System/Info/Public")?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "/System/Info/Public returned HTTP {}",
                resp.status()
            ));
        }
        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse /System/Info/Public response")?;
        Ok(data)
    }

    pub async fn get_users(&self) -> Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        let mut start_index: usize = 0;
        let page_size: usize = 500;
        loop {
            let path = format!("/Users?StartIndex={}&Limit={}", start_index, page_size);
            let url = self.url_path(&path);
            let resp = send_with_retry(self.add_auth_headers(self.client.get(&url)), "get_users")
                .await
                .with_context(|| {
                    format!(
                        "Failed to get users list (page {})",
                        start_index / page_size
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

    pub async fn get_library_items(&self) -> Result<HashMap<String, (String, String)>> {
        let mut all_items: HashMap<String, (String, String)> = HashMap::new();
        let mut start_index: usize = 0;
        let page_size: usize = 500;
        loop {
            let path = format!(
                "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex={}&Limit={}",
                start_index, page_size
            );
            let url = self.url_path(&path);
            let resp = send_with_retry(
                self.add_auth_headers(self.client.get(&url)),
                "get_library_items",
            )
            .await
            .with_context(|| format!("Failed to get library items (StartIndex={})", start_index))?;

            let data: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse library response")?;

            let items = data.get("Items").and_then(|i| i.as_array());
            let total = data
                .get("TotalRecordCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let page_count = items.map(|a| a.len()).unwrap_or(0);
            if let Some(arr) = items {
                for item in arr {
                    if let Some(id) = item.get("Id").and_then(|id| id.as_str()) {
                        let mut imdb = String::new();
                        let mut tmdb = String::new();
                        if let Some(providers) = item.get("ProviderIds") {
                            if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) {
                                imdb = val.to_string();
                            }
                            if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) {
                                tmdb = val.to_string();
                            }
                        }
                        all_items.insert(id.to_string(), (imdb, tmdb));
                    }
                }
            }
            if page_count < page_size || all_items.len() >= total {
                break;
            }
            start_index += page_size;
            if start_index > 100_000 {
                break;
            }
        }
        Ok(all_items)
    }

    pub async fn get_item_providers(
        &self,
        user_id: &str,
        item_id: &str,
    ) -> Result<(String, String)> {
        let path = format!("/Users/{}/Items/{}", user_id, item_id);
        let url = self.url_path(&path);
        let resp = self
            .add_auth_headers(self.client.get(&url))
            .send()
            .await
            .context("Failed to get item details")?;

        let data: serde_json::Value = resp.json().await.context("Failed to parse item response")?;

        let mut imdb = String::new();
        let mut tmdb = String::new();
        if let Some(providers) = data.get("ProviderIds") {
            if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) {
                imdb = val.to_string();
            }
            if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) {
                tmdb = val.to_string();
            }
        }
        Ok((imdb, tmdb))
    }

    pub async fn get_item_name(&self, user_id: &str, item_id: &str) -> Result<String> {
        let path = format!("/Users/{}/Items/{}", user_id, item_id);
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_item_name",
        )
        .await?;
        let data: serde_json::Value = resp.json().await?;
        let name = data.get("Name").and_then(|v| v.as_str()).unwrap_or("Unknown Item").to_string();
        Ok(name)
    }
}
