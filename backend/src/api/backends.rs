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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendConfig {
    pub backend_type: String, // "llamacpp" | "ollama" | "lmstudio" | "vllm" | "openai" | "custom"
    pub url: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: "llamacpp".to_string(),
            url: String::new(),
            model: String::new(),
            api_key: None,
        }
    }
}

#[derive(Deserialize)]
pub struct ModelsQuery {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub url: String,
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

    let api_key = queries::get_setting(&state.pool, "backend_api_key")
        .await
        .unwrap_or(None)
        .filter(|s| !s.is_empty());

    Json(BackendConfig {
        backend_type,
        url,
        model,
        api_key,
    })
}

// ─── POST /api/backends/config ────────────────────────────────────────────────

pub async fn set_backend_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<BackendConfig>,
) -> impl IntoResponse {
    let pool = &state.pool;

    if let Err(e) = queries::set_setting(pool, "backend_type", &cfg.backend_type).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    if let Err(e) = queries::set_setting(pool, "backend_url", &cfg.url).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    if let Err(e) = queries::set_setting(pool, "backend_model", &cfg.model).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    let api_key_val = cfg.api_key.as_deref().unwrap_or("");
    if let Err(e) = queries::set_setting(pool, "backend_api_key", api_key_val).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    Json(serde_json::json!({ "ok": true })).into_response()
}

// ─── GET /api/backends/models ─────────────────────────────────────────────────

pub async fn list_backend_models(
    Query(q): Query<ModelsQuery>,
) -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let base = q.url.trim_end_matches('/');

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
                        Err(e) => (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({ "error": format!("Parse error: {}", e) })),
                        )
                            .into_response(),
                    }
                }
                Ok(resp) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Ollama returned HTTP {}", resp.status()) })),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Cannot reach Ollama: {}", e) })),
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
                        Err(e) => (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({ "error": format!("Parse error: {}", e) })),
                        )
                            .into_response(),
                    }
                }
                Ok(resp) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Backend returned HTTP {}", resp.status()) })),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Cannot reach backend: {}", e) })),
                )
                    .into_response(),
            }
        }
    }
}
