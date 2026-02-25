use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use futures::future::join_all;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    db::queries,
    llama_cpp::validate_model_path,
    AppState,
};

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StartInferenceRequest {
    pub model_path: String,
    /// Device IDs from the DB whose RPC servers should be included
    pub device_ids: Vec<String>,
    /// Number of layers to put on GPU. -1 = all (default), 0 = CPU only.
    pub n_gpu_layers: Option<i32>,
    /// Context window size in tokens (default 4096).
    pub ctx_size: Option<u32>,
}

/// Query params for GET /api/cluster/model-check
#[derive(Deserialize)]
pub struct ModelCheckParams {
    pub path: String,
    /// Comma-separated device IDs to include in the memory pool.
    pub device_ids: Option<String>,
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

    // Probe all approved devices in parallel (each with a 2-second timeout)
    let probe_data: Vec<_> = approved
        .iter()
        .map(|d| {
            (
                d.id.clone(),
                d.name.clone(),
                d.ip.clone(),
                d.rpc_port,
                d.rpc_status.clone(),
                d.memory_total_mb,
                d.memory_free_mb,
            )
        })
        .collect();

    let llama_cpp = state.llama_cpp.clone();
    let pool = state.pool.clone();
    let http_client = state.llama_cpp.client.clone();

    let probe_futs = probe_data.into_iter().map(
        move |(id, name, ip, rpc_port, rpc_status, memory_total_mb, memory_free_mb)| {
            let mgr = llama_cpp.clone();
            let pool = pool.clone();
            let ip_clone = ip.clone();
            let id_clone = id.clone();
            let client = http_client.clone();
            async move {
                let reachable = mgr.probe_rpc_device(&ip_clone, rpc_port as u16).await;
                let live_status: String = if reachable {
                    "ready".to_string()
                } else {
                    rpc_status.clone()
                };
                // Persist live probe result to DB so other pages see consistent status
                let _ = queries::update_device_rpc_status(&pool, &id_clone, &live_status).await;

                // When reachable, fetch real memory stats from the remote device
                let (mem_total, mem_free) = if reachable {
                    match fetch_remote_memory(&client, &ip_clone).await {
                        Some((t, f)) => {
                            let _ = queries::update_device_memory_stats(&pool, &id_clone, t, f)
                                .await;
                            (t, f)
                        }
                        None => (memory_total_mb, memory_free_mb),
                    }
                } else {
                    (memory_total_mb, memory_free_mb)
                };

                serde_json::json!({
                    "id": id,
                    "name": name,
                    "ip": ip,
                    "rpc_port": rpc_port,
                    "rpc_status": live_status,
                    "memory_total_mb": mem_total,
                    "memory_free_mb": mem_free,
                })
            }
        },
    );
    let device_statuses: Vec<_> = join_all(probe_futs).await;

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

/// Fetch total and free memory from a remote device's /api/gpu endpoint.
/// Returns `None` if the request fails or the device reports no memory.
async fn fetch_remote_memory(client: &reqwest::Client, ip: &str) -> Option<(i64, i64)> {
    let url = format!("http://{}:8080/api/gpu", ip);
    let data: serde_json::Value = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let providers = data["providers"].as_array()?;
    if providers.is_empty() {
        return None;
    }
    let total: i64 = providers
        .iter()
        .filter_map(|p| p["total_mb"].as_i64())
        .sum();
    let free: i64 = providers
        .iter()
        .filter_map(|p| p["free_mb"].as_i64())
        .sum();
    if total == 0 {
        return None;
    }
    Some((total, free))
}

// ─── POST /api/cluster/inference/start ───────────────────────────────────────

pub async fn start_inference(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartInferenceRequest>,
) -> impl IntoResponse {
    // Validate model path before doing anything else (VULN-02)
    if let Err(e) = validate_model_path(&req.model_path) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    // Limit device_ids to prevent DoS via excessive DB queries (VULN-12)
    if req.device_ids.len() > 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Too many device IDs (max 20)" })),
        )
            .into_response();
    }

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
        .start_inference(
            &req.model_path,
            rpc_addresses,
            req.n_gpu_layers.unwrap_or(-1),
            req.ctx_size.unwrap_or(4096),
        )
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

// ─── GET /api/cluster/model-check ────────────────────────────────────────────

pub async fn model_check(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ModelCheckParams>,
) -> impl IntoResponse {
    // Validate model path (VULN-02 defense in depth)
    if let Err(e) = validate_model_path(&params.path) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    // Get local free memory across all providers
    let snapshots = crate::memory::aggregate_snapshot_async(&state.providers).await;
    let local_free_mb: u64 = snapshots.iter().map(|s| s.free_mb).sum();

    // Collect free memory from selected (or all approved) cluster devices
    let device_free_mbs: Vec<u64> = if let Some(ids_str) = &params.device_ids {
        let ids: Vec<&str> = ids_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .take(20)  // VULN-12: cap at 20 to prevent DoS
            .collect();
        let mut mbs = Vec::new();
        for id in ids {
            if let Ok(Some(device)) = queries::get_device(&state.pool, id).await {
                if device.memory_free_mb > 0 {
                    mbs.push(device.memory_free_mb as u64);
                }
            }
        }
        mbs
    } else {
        vec![]
    };

    match crate::llama_cpp::LlamaCppManager::analyze_model(
        &params.path,
        local_free_mb,
        device_free_mbs,
    ) {
        Ok(analysis) => Json(serde_json::to_value(analysis).unwrap_or_default()).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
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

// ─── POST /v1/chat/completions (proxy to active backend) ─────────────────────

pub async fn chat_completions_proxy(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Response {
    // Read active backend config from DB
    let backend_type = queries::get_setting(&state.pool, "backend_type")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "llamacpp".to_string());

    // ── llama.cpp path (existing behaviour) ──────────────────────────────────
    if backend_type == "llamacpp" {
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

        return proxy_request(&state.llama_cpp.client, &url, None, body).await;
    }

    // ── External backend path ─────────────────────────────────────────────────
    let backend_url = queries::get_setting(&state.pool, "backend_url")
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let api_key = queries::get_setting(&state.pool, "backend_api_key")
        .await
        .unwrap_or(None)
        .filter(|s| !s.is_empty());

    if backend_url.is_empty() {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": "No backend URL configured. Set a backend in the Inference page." })
                    .to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::empty())
                    .unwrap()
            });
    }

    let chat_url = if backend_type == "ollama" {
        // Ollama supports OpenAI-compat endpoint too; use /v1/chat/completions
        format!("{}/v1/chat/completions", backend_url.trim_end_matches('/'))
    } else {
        format!("{}/v1/chat/completions", backend_url.trim_end_matches('/'))
    };

    proxy_request(&state.llama_cpp.client, &chat_url, api_key.as_deref(), body).await
}

// ─── GET /v1/models ──────────────────────────────────────────────────────────
/// OpenAI-compatible model list. Proxies to the active backend when inference
/// is running; returns an empty list otherwise so Open WebUI stays connected.
pub async fn models_proxy(
    State(state): State<Arc<AppState>>,
) -> Response {
    let backend_type = queries::get_setting(&state.pool, "backend_type")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "llamacpp".to_string());

    // Helper: build an empty OpenAI models response
    let empty = || {
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "object": "list", "data": [] }).to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder().status(200).body(Body::empty()).unwrap()
            })
    };

    // ── llama.cpp path ────────────────────────────────────────────────────────
    if backend_type == "llamacpp" {
        if !state.llama_cpp.is_inference_running().await {
            return empty();
        }
        let url = format!("{}/v1/models", state.llama_cpp.inference_base_url());
        return proxy_get(&state.llama_cpp.client, &url, None).await;
    }

    // ── External backend path ─────────────────────────────────────────────────
    let backend_url = queries::get_setting(&state.pool, "backend_url")
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    if backend_url.is_empty() {
        return empty();
    }

    let api_key = queries::get_setting(&state.pool, "backend_api_key")
        .await
        .unwrap_or(None)
        .filter(|s| !s.is_empty());

    let url = format!("{}/v1/models", backend_url.trim_end_matches('/'));
    proxy_get(&state.llama_cpp.client, &url, api_key.as_deref()).await
}

// ─── shared proxy helper ──────────────────────────────────────────────────────

async fn proxy_get(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
) -> Response {
    let mut req = client.get(url);
    if let Some(key) = api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "application/json".parse().unwrap());
            let bytes = resp.bytes().await.unwrap_or_default();
            Response::builder()
                .status(status)
                .header("content-type", ct)
                .body(Body::from(bytes))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
        }
        Err(_e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": "Backend unreachable" })
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

async fn proxy_request(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
    body: axum::body::Bytes,
) -> Response {
    let mut req = client
        .post(url)
        .header("Content-Type", "application/json");

    if let Some(key) = api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    match req.body(body).send().await {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "application/json".parse().unwrap());
            let stream = resp.bytes_stream();
            Response::builder()
                .status(status)
                .header("content-type", ct)
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
        }
        Err(_e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": "Backend unreachable" }).to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())
                    .unwrap()
            }),
    }
}
