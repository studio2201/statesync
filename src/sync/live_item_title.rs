use crate::client::MediaClient;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Resolve a human title for live-sync log lines.
pub async fn resolve_live_item_title(
    item_name: Option<String>,
    source_item_id: &str,
    user_lower: &str,
    source_index: usize,
    state_lock: &Arc<Mutex<AppState>>,
    source_client: &Arc<MediaClient>,
) -> String {
    match item_name {
        Some(ref name) if !name.is_empty() => name.clone(),
        _ => {
            let mut found_name = None;
            {
                let st = state_lock.lock().await;
                for (_, name, _, _, id) in st.active_sessions.values() {
                    if id == source_item_id {
                        found_name = Some(name.clone());
                        break;
                    }
                }
            }
            if let Some(name) = found_name {
                name
            } else {
                let src_user_id = {
                    let st = state_lock.lock().await;
                    st.caches[source_index].users.get(user_lower).cloned()
                };
                if let Some(uid) = src_user_id {
                    match source_client.get_item_name(&uid, source_item_id).await {
                        Ok(name) => name,
                        Err(_) => format!("item ID '{}'", source_item_id),
                    }
                } else {
                    format!("item ID '{}'", source_item_id)
                }
            }
        }
    }
}
