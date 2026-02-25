use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{db::queries, AppState};

// ─── Types ────────────────────────────────────────────────────────────────────

/// The `api_key` field is intentionally masked in GET responses.
/// The real key is stored in the database and never sent back to clients.
/// On POST, send `null` / omit the field to keep the existing key,
/// or send a new non-empty string to replace it.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendConfig {
    pub backend_type: String, // "llamacpp" | "ollama" | "lmstudio" | "vllm" | "openai" | "custom"
    pub url: String,
    pub model: String,
    /// GET: `true` if a key is stored, `false` otherwise (the actual value is never returned).
    /// POST: send the new key value to replace, or omit / send null to keep existing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Indicates whether an API key is currently stored (returned on GET only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_set: Option<bool>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: "llamacpp".to_string(),
            url: String::new(),
            model: String::new(),
            api_key: None,
            api_key_set: Some(false),
        }
    }
}

#[derive(Deserialize)]
pub struct ModelsQuery {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub url: String,
    /// API key is accepted via query param for backwards compatibility,
    /// but should be sent via the X-Backend-Api-Key header when possible.
    pub api_key: Option<String>,
}

// ─── GET /api/backends/config ─────────────────────────────────────────────────

pub async fn get_backend_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let backend_type = queries::get_setting(&state.pool, "backend_type")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "llamacpp".to_string());

    let url = queries::get_setting(&state.pool, "backend_url")
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let model = queries::get_setting(&state.pool, "backend_model")
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    // SECURITY: Never return the actual API key — only signal whether one is set.
    let api_key_set = queries::get_setting(&state.pool, "backend_api_key")
        .await
        .unwrap_or(None)
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Json(BackendConfig {
        backend_type,
        url,
        model,
        api_key: None,           // never echoed back
        api_key_set: Some(api_key_set),
    })
}

// ─── POST /api/backends/config ────────────────────────────────────────────────

pub async fn set_backend_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<BackendConfig>,
) -> impl IntoResponse {
    let pool = &state.pool;

    if let Err(e) = queries::set_setting(pool, "backend_type", &cfg.backend_type).await {
        tracing::error!("Failed to save backend_type: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to save configuration" })),
        )
            .into_response();
    }
    if let Err(e) = queries::set_setting(pool, "backend_url", &cfg.url).await {
        tracing::error!("Failed to save backend_url: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to save configuration" })),
        )
            .into_response();
    }
    if let Err(e) = queries::set_setting(pool, "backend_model", &cfg.model).await {
        tracing::error!("Failed to save backend_model: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to save configuration" })),
        )
            .into_response();
    }

    // SECURITY: Only update the stored API key if the client sent a non-empty,
    // non-placeholder value. This prevents accidentally wiping or overwriting
    // the key when the frontend sends back a masked placeholder.
    if let Some(key) = &cfg.api_key {
        if !key.is_empty() && key != "****" {
            if let Err(e) = queries::set_setting(pool, "backend_api_key", key).await {
                tracing::error!("Failed to save backend_api_key: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Failed to save configuration" })),
                )
                    .into_response();
            }
        }
    }

    Json(serde_json::json!({ "ok": true })).into_response()
}

// ─── GET /api/backends/models ─────────────────────────────────────────────────

pub async fn list_backend_models(
    Query(q): Query<ModelsQuery>,
) -> impl IntoResponse {
    // Basic URL validation — reject empty or obviously malformed URLs
    let base_url = q.url.trim();
    if base_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "url parameter is required" })),
        )
            .into_response();
    }
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "url must start with http:// or https://" })),
        )
            .into_response();
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let base = base_url.trim_end_matches('/');

    match q.backend_type.as_str() {
        "ollama" => {
            // Ollama: GET {url}/api/tags → { "models": [{ "name": "..." }] }
            let url = format!("{}/api/tags", base);
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let models: Vec<String> = json["models"]
                                .as_array()
                                .unwrap_or(&vec![])
                                .iter()
                                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                                .collect();
                            Json(models).into_response()
                        }
                        Err(_) => (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({ "error": "Failed to parse Ollama response" })),
                        )
                            .into_response(),
                    }
                }
                Ok(resp) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Ollama returned HTTP {}", resp.status())
                    })),
                )
                    .into_response(),
                Err(_) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "Cannot reach Ollama at the provided URL" })),
                )
                    .into_response(),
            }
        }
        _ => {
            // OpenAI-compatible: GET {url}/v1/models → { "data": [{ "id": "..." }] }
            let url = format!("{}/v1/models", base);
            let mut req = client.get(&url);
            if let Some(key) = &q.api_key {
                if !key.is_empty() {
                    req = req.header(header::AUTHORIZATION, format!("Bearer {}", key));
                }
            }
            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let models: Vec<String> = json["data"]
                                .as_array()
                                .unwrap_or(&vec![])
                                .iter()
                                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                                .collect();
                            Json(models).into_response()
                        }
                        Err(_) => (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({ "error": "Failed to parse backend response" })),
                        )
                            .into_response(),
                    }
                }
                Ok(resp) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Backend returned HTTP {}", resp.status())
                    })),
                )
                    .into_response(),
                Err(_) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "Cannot reach the backend at the provided URL" })),
                )
                    .into_response(),
            }
        }
    }
}
