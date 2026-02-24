use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{db::{models::Role, queries}, AppState};

#[derive(Deserialize)]
pub struct UpsertRoleRequest {
    pub name: String,
    pub max_memory_mb: i64,
    pub can_pull_models: bool,
    pub trust_level: i64,
}

/// GET /api/permissions/roles
pub async fn list_roles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match queries::list_roles(&state.pool).await {
        Ok(roles) => Json(serde_json::json!({ "roles": roles })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/permissions/roles
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertRoleRequest>,
) -> impl IntoResponse {
    let role = Role {
        id: format!("role-{}", Uuid::new_v4()),
        name: req.name,
        max_memory_mb: req.max_memory_mb,
        can_pull_models: req.can_pull_models,
        trust_level: req.trust_level,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    match queries::upsert_role(&state.pool, &role).await {
        Ok(()) => (StatusCode::CREATED, Json(role)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// PUT /api/permissions/roles/:id
pub async fn update_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpsertRoleRequest>,
) -> impl IntoResponse {
    let role = Role {
        id: id.clone(),
        name: req.name,
        max_memory_mb: req.max_memory_mb,
        can_pull_models: req.can_pull_models,
        trust_level: req.trust_level,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    match queries::upsert_role(&state.pool, &role).await {
        Ok(()) => {
            // Re-fetch from DB so created_at reflects the actual stored value
            match queries::get_role(&state.pool, &id).await {
                Ok(Some(stored)) => Json(stored).into_response(),
                Ok(None) => Json(role).into_response(), // fallback (should not happen)
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/permissions/roles/:id
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Prevent deleting built-in roles
    if ["role-admin", "role-user", "role-guest"].contains(&id.as_str()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Cannot delete built-in roles" })),
        )
            .into_response();
    }

    match queries::delete_role(&state.pool, &id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
