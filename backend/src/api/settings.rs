use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{db::queries, AppState};

#[derive(Deserialize)]
pub struct UpdateSettingRequest {
    pub value: String,
}

/// GET /api/settings
pub async fn list_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match queries::list_settings(&state.pool).await {
        Ok(settings) => {
            let map: std::collections::HashMap<String, String> =
                settings.into_iter().map(|s| (s.key, s.value)).collect();
            Json(map).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// PUT /api/settings/:key
pub async fn update_setting(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Json(req): Json<UpdateSettingRequest>,
) -> impl IntoResponse {
    match queries::set_setting(&state.pool, &key, &req.value).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "key": key, "value": req.value }))
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
