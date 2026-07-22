use super::MediaClient;
use super::request::send_with_retry;
use super::types::PlayedItem;
use anyhow::{Context, Result};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

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
    pub async fn get_user_played_items(
        &self,
        user_id: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<PlayedItem>> {
        let path = format!(
            "/Users/{}/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex={}&Limit={}",
            utf8_percent_encode(user_id, PATH_SEGMENT),
            start_index,
            limit
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
        let count = data
            .get("TotalRecordCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
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
        let p = super::ProviderIds::from_json(v.get("ProviderIds"));
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
            if !p.imdb.is_empty() {
                map.insert("imdb_id".to_string(), serde_json::json!(p.imdb));
            }
            if !p.tmdb.is_empty() {
                map.insert("tmdb_id".to_string(), serde_json::json!(p.tmdb));
            }
            if !p.tvdb.is_empty() {
                map.insert("tvdb_id".to_string(), serde_json::json!(p.tvdb));
            }
        }
        match serde_json::from_value::<PlayedItem>(v) {
            Ok(item) => out.push(item),
            Err(_) => continue,
        }
    }
    out
}
