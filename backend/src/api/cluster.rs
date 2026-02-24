use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    db::queries,
    AppState,
};

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StartInferenceRequest {
    pub model_path: String,
    /// Device IDs from the DB whose RPC servers should be included
    pub device_ids: Vec<String>,
}

// ─── GET /api/cluster/status ──────────────────────────────────────────────────

pub async fn cluster_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let devices = match queries::list_devices(&state.pool).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let approved: Vec<_> = devices
        .iter()
        .filter(|d| d.status == "approved")
        .collect();

    // Probe each approved device's RPC port to get live status
    let mut device_statuses = Vec::new();
    for device in &approved {
        let reachable = state
            .llama_cpp
            .probe_rpc_device(&device.ip, device.rpc_port as u16)
            .await;
        device_statuses.push(serde_json::json!({
            "id": device.id,
            "name": device.name,
            "ip": device.ip,
            "rpc_port": device.rpc_port,
            "rpc_status": if reachable { "ready" } else { &device.rpc_status },
            "memory_total_mb": device.memory_total_mb,
            "memory_free_mb": device.memory_free_mb,
        }));
    }

    let llama_status = state.llama_cpp.get_status().await;

    Json(serde_json::json!({
        "devices": device_statuses,
        "llama_cpp": {
            "rpc_server_running": llama_status.rpc_server_running,
            "inference_running": llama_status.inference_running,
            "rpc_server_bin": llama_status.rpc_server_bin,
            "inference_server_bin": llama_status.inference_server_bin,
            "rpc_port": llama_status.rpc_port,
            "inference_port": llama_status.inference_port,
        },
        "current_session": llama_status.current_session,
    }))
    .into_response()
}

// ─── POST /api/cluster/inference/start ───────────────────────────────────────

pub async fn start_inference(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartInferenceRequest>,
) -> impl IntoResponse {
    // Build the list of "ip:port" strings for the selected devices
    let mut rpc_addresses = Vec::new();

    for device_id in &req.device_ids {
        match queries::get_device(&state.pool, device_id).await {
            Ok(Some(device)) => {
                rpc_addresses.push(format!("{}:{}", device.ip, device.rpc_port));
            }
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": format!("Device not found: {}", device_id) })),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        }
    }

    match state
        .llama_cpp
        .start_inference(&req.model_path, rpc_addresses)
        .await
    {
        Ok(()) => {
            let session = state.llama_cpp.get_current_session().await;
            Json(serde_json::json!({
                "ok": true,
                "session": session,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─── POST /api/cluster/inference/stop ────────────────────────────────────────

pub async fn stop_inference(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.llama_cpp.stop_inference().await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─── GET /api/cluster/inference/status ───────────────────────────────────────

pub async fn inference_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.llama_cpp.get_status().await;
    Json(serde_json::json!({
        "running": status.inference_running,
        "healthy": state.llama_cpp.inference_is_healthy().await,
        "session": status.current_session,
        "inference_port": status.inference_port,
    }))
    .into_response()
}

// ─── POST /api/cluster/rpc/start ─────────────────────────────────────────────

pub async fn start_rpc_server(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.llama_cpp.start_rpc_server().await {
        Ok(()) => Json(serde_json::json!({
            "ok": true,
            "port": state.llama_cpp.rpc_port,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─── POST /api/cluster/rpc/stop ──────────────────────────────────────────────

pub async fn stop_rpc_server(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.llama_cpp.stop_rpc_server().await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─── POST /v1/chat/completions (proxy to llama-server) ───────────────────────

pub async fn chat_completions_proxy(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Response {
    if !state.llama_cpp.is_inference_running().await {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "error": "Inference server is not running. Start it from the Inference page first."
                })
                .to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::empty())
                    .unwrap()
            });
    }

    let url = format!(
        "{}/v1/chat/completions",
        state.llama_cpp.inference_base_url()
    );

    match state
        .llama_cpp
        .client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| {
                    "application/json".parse().unwrap()
                });
            let stream = resp.bytes_stream();
            Response::builder()
                .status(status)
                .header("content-type", ct)
                .header("Access-Control-Allow-Origin", "*")
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
                serde_json::json!({ "error": format!("llama-server unreachable: {}", e) })
                    .to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())
                    .unwrap()
            }),
    }
}
