//! Item lookup helpers on MediaClient.
use super::MediaClient;
use super::ProviderIds;
use super::request::send_with_retry;
use anyhow::{Context, Result};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::collections::HashMap;

impl MediaClient {
    pub async fn get_library_items(&self) -> Result<HashMap<String, ProviderIds>> {
        let mut all_items: HashMap<String, ProviderIds> = HashMap::new();
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
                        let p = ProviderIds::from_json(item.get("ProviderIds"));
                        all_items.insert(id.to_string(), p);
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
    ) -> Result<ProviderIds> {
        let path = format!(
            "/Users/{}/Items/{}",
            utf8_percent_encode(user_id, NON_ALPHANUMERIC),
            utf8_percent_encode(item_id, NON_ALPHANUMERIC)
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_item_providers",
        )
        .await
        .context("Failed to get item details")?;

        let data: serde_json::Value = resp.json().await.context("Failed to parse item response")?;
        Ok(ProviderIds::from_json(data.get("ProviderIds")))
    }

    pub async fn get_item_name(&self, user_id: &str, item_id: &str) -> Result<String> {
        let path = format!(
            "/Users/{}/Items/{}",
            utf8_percent_encode(user_id, NON_ALPHANUMERIC),
            utf8_percent_encode(item_id, NON_ALPHANUMERIC)
        );
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_item_name",
        )
        .await?;
        let data: serde_json::Value = resp.json().await?;
        let name = data
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Item")
            .to_string();
        Ok(name)
    }
}
