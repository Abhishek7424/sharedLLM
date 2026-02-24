use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    db::queries,
    permissions::PermissionService,
    AppState,
};

#[derive(Deserialize)]
pub struct AddDeviceRequest {
    pub name: String,
    pub ip: String,
    pub mac: Option<String>,
}

#[derive(Deserialize)]
pub struct ApproveDeviceRequest {
    pub role_id: Option<String>,
}

#[derive(Deserialize)]
pub struct AllocateMemoryRequest {
    pub memory_mb: i64,
}

/// GET /api/devices
pub async fn list_devices(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match queries::list_devices(&state.pool).await {
        Ok(devices) => Json(serde_json::json!({ "devices": devices })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/devices/:id
pub async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match queries::get_device(&state.pool, &id).await {
        Ok(Some(device)) => Json(device).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Device not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/devices  (manual add)
pub async fn add_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddDeviceRequest>,
) -> impl IntoResponse {
    let svc = PermissionService::new(state.pool.clone(), state.event_tx.clone());
    match svc
        .register_device(req.name, req.ip, req.mac, "manual")
        .await
    {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/devices/:id/approve
pub async fn approve_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ApproveDeviceRequest>,
) -> impl IntoResponse {
    let svc = PermissionService::new(state.pool.clone(), state.event_tx.clone());
    match svc.approve_device(&id, req.role_id.as_deref()).await {
        Ok(device) => Json(device).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/devices/:id/deny
pub async fn deny_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let svc = PermissionService::new(state.pool.clone(), state.event_tx.clone());
    match svc.deny_device(&id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// PATCH /api/devices/:id/memory
pub async fn allocate_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<AllocateMemoryRequest>,
) -> impl IntoResponse {
    let svc = PermissionService::new(state.pool.clone(), state.event_tx.clone());
    match svc.allocate_memory(&id, req.memory_mb).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "memory_mb": req.memory_mb })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/devices/:id
pub async fn delete_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match queries::delete_device(&state.pool, &id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
