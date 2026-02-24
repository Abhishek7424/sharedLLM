use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct PullModelRequest {
    pub name: String,
}

/// GET /api/models
pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.ollama.list_models().await {
        Ok(models) => Json(serde_json::json!({ "models": models })).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/models/pull
/// Streams the Ollama pull response so the client gets progress lines in real time.
pub async fn pull_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PullModelRequest>,
) -> impl IntoResponse {
    match state.ollama.pull_model_stream(&req.name).await {
        Ok(response) => {
            let status = response.status();
            // Convert the reqwest byte stream into an axum Body so we stream
            // progress NDJSON lines to the client without buffering the whole body.
            let stream = response.bytes_stream();
            Response::builder()
                .status(status)
                .header("Content-Type", "application/x-ndjson")
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": e.to_string() }).to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())
                    .unwrap()
            }),
    }
}

/// DELETE /api/models/:name
pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.ollama.delete_model(&name).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/ollama/status
pub async fn ollama_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let running = state.ollama.is_healthy().await;
    Json(serde_json::json!({
        "running": running,
        "host": state.ollama.host,
    }))
}
