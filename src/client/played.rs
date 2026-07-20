use anyhow::{Context, Result, anyhow};
use super::MediaClient;
use super::types::PlayedItem;
use super::request::send_with_retry;

impl MediaClient {
    pub async fn find_item_by_provider(
        &self,
        user_id: &str,
        imdb_id: &str,
        tmdb_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let mut path = format!("/Users/{}/Items?Recursive=true&Fields=ProviderIds", user_id);
        if !imdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Imdb&ProviderIds={}",
                percent_encoding::utf8_percent_encode(imdb_id, percent_encoding::NON_ALPHANUMERIC)
            ));
        } else if !tmdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Tmdb&ProviderIds={}",
                percent_encoding::utf8_percent_encode(tmdb_id, percent_encoding::NON_ALPHANUMERIC)
            ));
        } else {
            return Ok(None);
        }

        let url = self.url_path(&path);
        let resp = self
            .add_auth_headers(self.client.get(&url))
            .send()
            .await
            .context("Failed to search item by provider ID")?;

        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse search response")?;

        if let Some(items) = data.get("Items").and_then(|i| i.as_array()) {
            if let Some(item) = items.first() {
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
                    return Ok(Some((id.to_string(), imdb, tmdb)));
                }
            }
        }
        Ok(None)
    }

    pub async fn update_progress(
        &self,
        user_id: &str,
        item_id: &str,
        position_ticks: i64,
        played: bool,
    ) -> Result<()> {
        let path = format!("/Users/{}/Items/{}/UserData", user_id, item_id);
        let url = self.url_path(&path);
        let body = serde_json::json!({
            "PlaybackPositionTicks": position_ticks,
            "Played": played,
        });

        let resp = self
            .add_auth_headers(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .context("Failed to send UserData progress update request")?;

        if !resp.status().is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "UserData progress update failed: {} - {}",
                url,
                body_text
            ));
        }
        Ok(())
    }

    pub async fn get_user_played_items(
        &self,
        user_id: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<PlayedItem>> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex={}&Limit={}",
            user_id, start_index, limit
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_user_played_items",
        )
        .await
        .with_context(|| {
            format!(
                "Failed to list played items for user {} (page {})",
                user_id,
                start_index / limit
            )
        })?;
        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse played items response")?;
        let arr = data
            .get("Items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::with_capacity(arr.len());
        for mut v in arr {
            let imdb = v
                .get("ProviderIds")
                .and_then(|p| p.get("Imdb"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let tmdb = v
                .get("ProviderIds")
                .and_then(|p| p.get("Tmdb"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            if let Some(map) = v.as_object_mut() {
                if let Some(imdb) = &imdb {
                    map.insert(
                        "imdb_id".to_string(),
                        serde_json::Value::String(imdb.clone()),
                    );
                }
                if let Some(tmdb) = &tmdb {
                    map.insert(
                        "tmdb_id".to_string(),
                        serde_json::Value::String(tmdb.clone()),
                    );
                }
            }
            match serde_json::from_value::<PlayedItem>(v) {
                Ok(item) => out.push(item),
                Err(_) => continue,
            }
        }
        Ok(out)
    }

    pub async fn get_user_played_items_count(&self, user_id: &str) -> Result<u64> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Filters=IsPlayed&Limit=0",
            user_id
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_user_played_items_count",
        )
        .await?;
        let data: serde_json::Value = resp.json().await?;
        let count = data.get("TotalRecordCount").and_then(|v| v.as_u64()).unwrap_or(0);
        Ok(count)
    }
}
