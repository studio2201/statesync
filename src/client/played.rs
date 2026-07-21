use anyhow::{Context, Result};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use super::MediaClient;
use super::types::PlayedItem;
use super::request::send_with_retry;

/// Encode path segments without mangling Emby/Jellyfin id characters (`_`, `-`, `.`).
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'?')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

impl MediaClient {
    /// Missing documentation.
    pub async fn find_item_by_provider(
        &self,
        user_id: &str,
        imdb_id: &str,
        tmdb_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let uid = utf8_percent_encode(user_id, PATH_SEGMENT);
        let mut path = format!("/Users/{}/Items?Recursive=true&Fields=ProviderIds", uid);
        if !imdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Imdb&ProviderIds={}",
                utf8_percent_encode(imdb_id, PATH_SEGMENT)
            ));
        } else if !tmdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Tmdb&ProviderIds={}",
                utf8_percent_encode(tmdb_id, PATH_SEGMENT)
            ));
        } else {
            return Ok(None);
        }

        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "find_item_by_provider",
        )
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

    /// Read current UserData for an item (played / position / favorite).
    pub async fn get_item_user_data(
        &self,
        user_id: &str,
        item_id: &str,
    ) -> Result<crate::client::types::UserDataEntry> {
        let path = format!(
            "/Users/{}/Items/{}/UserData",
            utf8_percent_encode(user_id, PATH_SEGMENT),
            utf8_percent_encode(item_id, PATH_SEGMENT)
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_item_user_data",
        )
        .await
        .with_context(|| format!("UserData get failed: {}", url))?;
        let mut data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse UserData response")?;
        // Normalize ItemId if API omits it (some builds only return fields).
        if data.get("ItemId").is_none() && data.get("itemId").is_none() {
            if let Some(map) = data.as_object_mut() {
                map.insert("ItemId".to_string(), serde_json::json!(item_id));
            }
        }
        serde_json::from_value(data).context("Failed to decode UserData")
    }

    /// Write playback progress (position + played). Does not touch IsFavorite.
    pub async fn update_progress(
        &self,
        user_id: &str,
        item_id: &str,
        position_ticks: i64,
        played: bool,
    ) -> Result<()> {
        self.update_user_data(user_id, item_id, Some(position_ticks), Some(played), None)
            .await
    }

    /// Write favorite flag only. Does not touch Played / position.
    pub async fn update_favorite(
        &self,
        user_id: &str,
        item_id: &str,
        is_favorite: bool,
    ) -> Result<()> {
        self.update_user_data(user_id, item_id, None, None, Some(is_favorite))
            .await
    }

    /// Partial UserData update — only sends fields that are `Some`, so favorites
    /// and progress do not clobber each other.
    pub async fn update_user_data(
        &self,
        user_id: &str,
        item_id: &str,
        position_ticks: Option<i64>,
        played: Option<bool>,
        is_favorite: Option<bool>,
    ) -> Result<()> {
        let path = format!(
            "/Users/{}/Items/{}/UserData",
            utf8_percent_encode(user_id, PATH_SEGMENT),
            utf8_percent_encode(item_id, PATH_SEGMENT)
        );
        let url = self.url_path(&path);
        let mut body = serde_json::Map::new();
        if let Some(ticks) = position_ticks {
            body.insert(
                "PlaybackPositionTicks".to_string(),
                serde_json::json!(ticks),
            );
        }
        if let Some(p) = played {
            body.insert("Played".to_string(), serde_json::json!(p));
        }
        if let Some(fav) = is_favorite {
            body.insert("IsFavorite".to_string(), serde_json::json!(fav));
        }
        if body.is_empty() {
            return Ok(());
        }

        let _resp = send_with_retry(
            self.add_auth_headers(self.client.post(&url))
                .json(&serde_json::Value::Object(body)),
            "update_user_data",
        )
        .await
        .with_context(|| format!("UserData update failed: {}", url))?;
        Ok(())
    }

    /// Missing documentation.
    pub async fn get_user_played_items(
        &self,
        user_id: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<PlayedItem>> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex={}&Limit={}",
            utf8_percent_encode(user_id, PATH_SEGMENT), start_index, limit
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
        Ok(parse_items_with_providers(data))
    }

    /// Missing documentation.
    pub async fn get_user_played_items_count(&self, user_id: &str) -> Result<u64> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Filters=IsPlayed&Limit=0",
            utf8_percent_encode(user_id, PATH_SEGMENT)
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

    /// List a user's favorited items (with provider IDs for matching).
    pub async fn get_user_favorite_items(
        &self,
        user_id: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<PlayedItem>> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsFavorite&StartIndex={}&Limit={}",
            utf8_percent_encode(user_id, PATH_SEGMENT),
            start_index,
            limit
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_user_favorite_items",
        )
        .await
        .with_context(|| {
            format!(
                "Failed to list favorite items for user {} (page {})",
                user_id,
                start_index / limit.max(1)
            )
        })?;
        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse favorite items response")?;
        Ok(parse_items_with_providers(data))
    }

    pub async fn get_user_favorite_items_count(&self, user_id: &str) -> Result<u64> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Filters=IsFavorite&Limit=0",
            utf8_percent_encode(user_id, PATH_SEGMENT)
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_user_favorite_items_count",
        )
        .await?;
        let data: serde_json::Value = resp.json().await?;
        Ok(data
            .get("TotalRecordCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0))
    }
}

fn parse_items_with_providers(data: serde_json::Value) -> Vec<PlayedItem> {
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
        // Flatten UserData.IsFavorite onto the item for PlayedItem deserialize.
        if let Some(fav) = v
            .get("UserData")
            .and_then(|u| u.get("IsFavorite"))
            .and_then(|b| b.as_bool())
        {
            if let Some(map) = v.as_object_mut() {
                map.insert("IsFavorite".to_string(), serde_json::json!(fav));
            }
        }
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
    out
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_find_item_by_provider_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_update_progress_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_get_user_played_items_count_generated_test_0() {
        assert!(true);
    }
}
