use super::MediaClient;
use super::ProviderIds;
use super::request::send_with_retry;
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
    /// Search library by Imdb, then Tmdb, then Tvdb (first hit wins).
    pub async fn find_item_by_provider(
        &self,
        user_id: &str,
        providers: &ProviderIds,
    ) -> Result<Option<(String, ProviderIds)>> {
        let attempts: [(&str, &str); 3] = [
            ("Imdb", providers.imdb.as_str()),
            ("Tmdb", providers.tmdb.as_str()),
            ("Tvdb", providers.tvdb.as_str()),
        ];
        for (ptype, pid) in attempts {
            if pid.is_empty() {
                continue;
            }
            if let Some(hit) = self.find_item_one_provider(user_id, ptype, pid).await? {
                return Ok(Some(hit));
            }
        }
        Ok(None)
    }

    async fn find_item_one_provider(
        &self,
        user_id: &str,
        provider_type: &str,
        provider_id: &str,
    ) -> Result<Option<(String, ProviderIds)>> {
        let uid = utf8_percent_encode(user_id, PATH_SEGMENT);
        let path = format!(
            "/Users/{}/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes={}&ProviderIds={}",
            uid,
            provider_type,
            utf8_percent_encode(provider_id, PATH_SEGMENT)
        );
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
                    let found = ProviderIds::from_json(item.get("ProviderIds"));
                    return Ok(Some((id.to_string(), found)));
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
}
