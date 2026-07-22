//! Per-user ignore (skip live + mesh force for this person).

use axum::{Extension, Json, body::Body, http::StatusCode, response::Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::config::Config;
use crate::web::WebServerState;

#[derive(Debug, Deserialize)]
pub struct IgnoreUserRequest {
    /// Display username (any linked alias works).
    pub name: String,
    /// true = add to ignore list; false = remove.
    pub ignore: bool,
}

/// POST /api/users/ignore — persist ignore / un-ignore for one person.
pub async fn post_user_ignore(
    Extension(state): Extension<Arc<WebServerState>>,
    Json(req): Json<IgnoreUserRequest>,
) -> Response {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return json_err(StatusCode::BAD_REQUEST, "name is required");
    }

    let mut config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            return json_err(StatusCode::INTERNAL_SERVER_ERROR, &format!("config: {}", e));
        }
    };

    let key = name.to_lowercase();
    let list = &mut config.sync.user_ignorelist;
    let already = list.iter().any(|n| n.trim().eq_ignore_ascii_case(&key));

    if req.ignore {
        if !already {
            list.push(name.clone());
        }
    } else {
        list.retain(|n| !n.trim().eq_ignore_ascii_case(&key));
        // Also drop linked aliases that were stored as separate ignore entries.
        for group in &config.user_mappings {
            let members: Vec<String> = group
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if members.iter().any(|m| m == &key) {
                list.retain(|n| {
                    let ln = n.trim().to_lowercase();
                    !members.iter().any(|m| m == &ln)
                });
            }
        }
    }

    if let Err(e) = config.save() {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("save failed: {}", e),
        );
    }

    let _ = state.reload_tx.send(()).await;

    let msg = if req.ignore {
        format!(
            "Ignoring '{}' — live and force sync will skip this person",
            name
        )
    } else {
        format!("'{}' will sync again", name)
    };

    {
        let mut st = state.app_state.lock().await;
        st.log_event("info", &msg);
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(
            json!({
                "status": "ok",
                "message": msg,
                "ignore": req.ignore,
                "user_ignorelist": config.sync.user_ignorelist,
            })
            .to_string(),
        ))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}

fn json_err(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(
            json!({ "status": "error", "message": message }).to_string(),
        ))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}
