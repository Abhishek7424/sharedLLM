use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Device ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub platform: Option<String>,
    pub role_id: Option<String>,
    pub status: String, // pending | approved | denied | suspended | offline
    pub discovery_method: String, // mdns | manual
    pub allocated_memory_mb: i64,
    pub last_seen: Option<String>,
    pub first_seen: String,
    pub created_at: String,
    // RPC / distributed inference fields (added in migration 0003)
    pub rpc_port: i64,
    pub rpc_status: String, // offline | connecting | ready | error
    pub memory_total_mb: i64,
    pub memory_free_mb: i64,
}

impl Device {
    pub fn new(name: String, ip: String, mac: Option<String>, discovery_method: &str) -> Self {
        let now = Utc::now().to_rfc3339();
        Device {
            id: Uuid::new_v4().to_string(),
            name,
            ip,
            mac,
            hostname: None,
            platform: None,
            role_id: None,
            status: "pending".into(),
            discovery_method: discovery_method.into(),
            allocated_memory_mb: 0,
            last_seen: Some(now.clone()),
            first_seen: now.clone(),
            created_at: now,
            rpc_port: 8181,
            rpc_status: "offline".into(),
            memory_total_mb: 0,
            memory_free_mb: 0,
        }
    }
}

// ─── Role ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub max_memory_mb: i64,
    pub can_pull_models: bool, // sqlx maps SQLite INTEGER 0/1 → bool automatically
    pub trust_level: i64,
    pub created_at: String,
}

// ─── Allocation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Allocation {
    pub id: String,
    pub device_id: String,
    pub memory_mb: i64,
    pub provider: String,
    pub granted_at: String,
    pub revoked_at: Option<String>,
}

// ─── Setting ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

